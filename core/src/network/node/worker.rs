use std::{
    borrow::Cow, collections::HashMap, error::Error, fs, iter, path::PathBuf, str::FromStr,
    time::Duration,
};

use anyhow::anyhow;
use chrono::Utc;
use futures::{future, StreamExt};
use libp2p::{
    core::upgrade::Version,
    gossipsub::{self, MessageId, PublishError, Sha256Topic},
    identify, identity,
    kad::{store::MemoryStore, Kademlia, KademliaConfig},
    mdns, noise,
    request_response::{self, ProtocolSupport},
    swarm::{keep_alive, SwarmBuilder, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use log::{debug, info};
use rand::distributions::{Alphanumeric, DistString};
use serde::Serialize;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};
use tokio::{
    select,
    sync::{mpsc, oneshot},
    task,
};

use crate::{
    network::{
        address::Address,
        behaviour::{
            BitmessageBehaviourEvent, BitmessageNetBehaviour, BitmessageProtocol,
            BitmessageProtocolCodec, BitmessageRequest, BitmessageResponse,
        },
        messages::{
            MessageCommand, MessagePayload, MsgEncoding, NetworkMessage, Object, ObjectKind,
            UnencryptedMsg,
        },
    },
    repositories::{
        address::AddressRepositorySync,
        inventory::InventoryRepositorySync,
        message::MessageRepositorySync,
        sqlite::{
            address::SqliteAddressRepository,
            inventory::SqliteInventoryRepository,
            message::SqliteMessageRepository,
            models::{self, MessageStatus},
        },
    },
};

use super::{
    handler::Handler,
    pow_worker::{ProofOfWorkWorker, ProofOfWorkWorkerCommand},
};

const IDENTIFY_PROTO_NAME: &str = "/bitmessage/id/1.0.0";
const KADEMLIA_PROTO_NAME: &[u8] = b"/bitmessage/kad/1.0.0";

const MIGRATIONS: Migrator = sqlx::migrate!("src/repositories/sqlite/migrations");
const COMMON_PUBSUB_TOPIC: &'static str = "common";
const POOL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum Folder {
    Inbox,
    Sent,
}

#[derive(Debug)]
pub enum WorkerCommand {
    StartListening {
        multiaddr: Multiaddr,
        sender: oneshot::Sender<anyhow::Result<()>>,
    },
    Dial {
        peer: Multiaddr,
        sender: oneshot::Sender<anyhow::Result<()>>,
    },
    GetListenerAddress {
        sender: oneshot::Sender<Multiaddr>,
    },
    GetPeerID {
        sender: oneshot::Sender<PeerId>,
    },
    BroadcastMsgByPubSub {
        sender: oneshot::Sender<anyhow::Result<()>>,
        msg: NetworkMessage,
    },
    NonceCalculated {
        obj: Object,
    },
    GetOwnIdentities {
        sender: oneshot::Sender<anyhow::Result<Vec<Address>>>,
    },
    GenerateIdentity {
        label: String,
        sender: oneshot::Sender<anyhow::Result<String>>,
    },
    RenameIdentity {
        new_label: String,
        address: String,
        sender: oneshot::Sender<anyhow::Result<()>>,
    },
    DeleteIdentity {
        address: String,
        sender: oneshot::Sender<anyhow::Result<()>>,
    },
    GetMessages {
        address: String,
        folder: Folder,
        sender: oneshot::Sender<anyhow::Result<Vec<models::Message>>>,
    },
    SendMessage {
        msg: models::Message,
        from: String,
        sender: oneshot::Sender<anyhow::Result<()>>,
    },
    Shutdown,
}

pub struct NodeWorker {
    local_peer_id: PeerId,
    swarm: Swarm<BitmessageNetBehaviour>,
    handler: Handler,
    _command_sender: mpsc::Sender<WorkerCommand>,
    command_receiver: mpsc::Receiver<WorkerCommand>,

    pubkey_notifier: mpsc::Receiver<String>,
    tracked_pubkeys: HashMap<String, bool>,

    pending_commands: Vec<WorkerCommand>,
    _sqlite_connection_pool: SqlitePool,
    common_topic: Sha256Topic,

    inventory_repo: Box<InventoryRepositorySync>,
    address_repo: Box<AddressRepositorySync>,
    messages_repo: Box<MessageRepositorySync>,

    pow_worker_command_sink: mpsc::Sender<ProofOfWorkWorkerCommand>,
    pow_worker: Option<ProofOfWorkWorker>,
}

impl NodeWorker {
    pub async fn new(
        bootstrap_nodes: Option<Vec<Multiaddr>>,
        data_dir: PathBuf,
    ) -> (NodeWorker, mpsc::Sender<WorkerCommand>) {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer id: {:?}", local_peer_id);

        let transport = tcp::tokio::Transport::default()
            .upgrade(Version::V1Lazy)
            .authenticate(noise::Config::new(&local_key).unwrap())
            .multiplex(yamux::Config::default())
            .boxed();

        let mut swarm = SwarmBuilder::with_tokio_executor(
            transport,
            BitmessageNetBehaviour {
                gossipsub: gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(local_key.clone()),
                    Default::default(),
                )
                .unwrap(),
                rpc: request_response::Behaviour::new(
                    BitmessageProtocolCodec(),
                    iter::once((BitmessageProtocol(), ProtocolSupport::Full)),
                    Default::default(),
                ),
                kademlia: Kademlia::with_config(
                    local_peer_id,
                    MemoryStore::new(local_peer_id),
                    KademliaConfig::default()
                        .set_protocol_names(
                            iter::once(Cow::Borrowed(KADEMLIA_PROTO_NAME)).collect(),
                        )
                        .to_owned(),
                ),
                identify: identify::Behaviour::new(identify::Config::new(
                    IDENTIFY_PROTO_NAME.to_string(),
                    local_key.public(),
                )),
                mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap(),
                keep_alive: keep_alive::Behaviour::default(),
            },
            local_peer_id,
        )
        .build();

        if let Some(bootstrap_peers) = bootstrap_nodes {
            // First, we add the addresses of the bootstrap nodes to our view of the DHT
            for peer_address in &bootstrap_peers {
                let peer_id = extract_peer_id_from_multiaddr(peer_address).unwrap(); // FIXME
                swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, peer_address.clone());
            }

            // Next, we add our own info to the DHT. This will then automatically be shared
            // with the other peers on the DHT. This operation will fail if we are a bootstrap peer.
            swarm
                .behaviour_mut()
                .kademlia
                .bootstrap()
                .map_err(|err| err)
                .unwrap();
        }

        let data_dir_buf = data_dir.join("db");
        fs::create_dir_all(&data_dir_buf).expect("db folder is created");
        let db_url = data_dir_buf.join("database.db");

        debug!("{:?}", db_url.to_str().unwrap());

        let topic = Sha256Topic::new(COMMON_PUBSUB_TOPIC);
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .expect("subscription not to fail");

        let (sender, receiver) = mpsc::channel(3);
        let (pubkey_notifier_sink, pubkey_notifier) = mpsc::channel(3);

        let connect_options =
            SqliteConnectOptions::from_str(&format!("sqlite://{}", db_url.to_string_lossy()))
                .unwrap()
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .foreign_keys(true)
                .synchronous(SqliteSynchronous::Normal)
                .busy_timeout(POOL_TIMEOUT);

        let pool = SqlitePoolOptions::new()
            .connect_with(connect_options)
            .await
            .expect("pool open");

        MIGRATIONS.run(&pool).await.expect("migrations not to fail");

        let inventory_repo = Box::new(SqliteInventoryRepository::new(pool.clone()));
        let address_repo = Box::new(SqliteAddressRepository::new(pool.clone()));
        let message_repo = Box::new(SqliteMessageRepository::new(pool.clone()));

        let (pow_worker, pow_worker_command_sink) = ProofOfWorkWorker::new(
            inventory_repo.clone(),
            message_repo.clone(),
            address_repo.clone(),
            sender.clone(),
        );

        (
            Self {
                local_peer_id,
                swarm,
                handler: Handler::new(
                    address_repo.clone(),
                    inventory_repo.clone(),
                    message_repo.clone(),
                    sender.clone(),
                    pubkey_notifier_sink,
                    pow_worker_command_sink.clone(),
                ),
                _command_sender: sender.clone(),
                pubkey_notifier,
                tracked_pubkeys: HashMap::new(),
                command_receiver: receiver,
                pending_commands: Vec::new(),
                _sqlite_connection_pool: pool,
                common_topic: topic,

                address_repo: address_repo.clone(),
                inventory_repo: inventory_repo.clone(),
                messages_repo: message_repo.clone(),

                pow_worker_command_sink,
                pow_worker: Some(pow_worker),
            },
            sender,
        )
    }

    async fn handle_event<E>(&mut self, event: SwarmEvent<BitmessageBehaviourEvent, E>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {:?}", address);
                let indexes: Vec<usize> = self
                    .pending_commands
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| match v {
                        WorkerCommand::GetListenerAddress { sender: _ } => Some(i.clone()),
                        _ => None,
                    })
                    .collect();
                for i in indexes {
                    if let WorkerCommand::GetListenerAddress { sender } =
                        self.pending_commands.remove(i)
                    {
                        sender
                            .send(address.clone())
                            .expect("Receiver not to be dropped");
                    }
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint: _endpoint,
                num_established,
                cause: _cause,
            } => {
                if num_established == 0 {
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .remove_explicit_peer(&peer_id);
                    self.swarm.behaviour_mut().kademlia.remove_peer(&peer_id);
                }
            }
            SwarmEvent::Behaviour(BitmessageBehaviourEvent::RequestResponse(
                request_response::Event::Message { message, peer, .. },
            )) => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    debug!("received request {}: {:?}", request_id, request);
                    let msg = self.handler.handle_message(request.0).await.unwrap();
                    self.swarm
                        .behaviour_mut()
                        .rpc
                        .send_response(channel, BitmessageResponse(msg))
                        .unwrap()
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    debug!("received response on {}: {:?}", request_id, response);
                    let another_request = self.handler.handle_message(response.0).await;
                    if let Some(m) = another_request {
                        self.swarm
                            .behaviour_mut()
                            .rpc
                            .send_request(&peer, BitmessageRequest(m));
                    }
                }
            },
            SwarmEvent::Behaviour(BitmessageBehaviourEvent::Identify(e)) => {
                self.handle_identify_event(e)
            }
            SwarmEvent::Behaviour(BitmessageBehaviourEvent::Mdns(mdns::Event::Discovered(
                list,
            ))) => {
                for (peer_id, multiaddr) in list {
                    debug!("Found new peer via mDNS: {:?}/{:?}", multiaddr, peer_id);
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, multiaddr);
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer_id);
                    self.on_new_peer(peer_id.clone());
                }
            }
            SwarmEvent::Behaviour(BitmessageBehaviourEvent::Gossipsub(
                gossipsub::Event::Message {
                    propagation_source: _,
                    message_id: _,
                    message,
                },
            )) => {
                if message.topic != self.common_topic.hash() {
                    return;
                }
                let msg: NetworkMessage = serde_cbor::from_slice(&message.data).unwrap();
                let reply = self.handler.handle_message(msg).await;
                if let Some(m) = reply {
                    self.swarm
                        .behaviour_mut()
                        .rpc
                        .send_request(&message.source.unwrap(), BitmessageRequest(m));
                }
            }
            _ => {}
        }
    }

    async fn handle_command(&mut self, command: WorkerCommand) {
        match command {
            WorkerCommand::StartListening { multiaddr, sender } => {
                debug!("Starting listening to the network...");
                match self.swarm.listen_on(multiaddr.clone()) {
                    Ok(_) => sender.send(Ok(())).expect("Receiver not to be dropped"),
                    Err(e) => sender
                        .send(Err(anyhow!(e.to_string())))
                        .expect("Receiver not to be dropped"),
                };
            }
            WorkerCommand::Dial {
                peer: _peer,
                sender: _sender,
            } => todo!(),
            WorkerCommand::GetListenerAddress { sender } => match self.swarm.listeners().next() {
                Some(v) => {
                    sender.send(v.clone()).expect("Receiver not to be dropped");
                }
                None => {
                    self.pending_commands
                        .push(WorkerCommand::GetListenerAddress { sender });
                }
            },
            WorkerCommand::GetPeerID { sender } => sender
                .send(self.local_peer_id)
                .expect("Receiver not to be dropped"),
            WorkerCommand::BroadcastMsgByPubSub { sender, msg } => match self.publish_pubsub(msg) {
                Ok(_) => sender.send(Ok(())).expect("receiver not to be dropped"),
                Err(e) => sender
                    .send(Err(anyhow!(e.to_string())))
                    .expect("receiver not to be dropped"),
            },
            WorkerCommand::NonceCalculated { obj } => {
                match &obj.kind {
                    ObjectKind::Msg { encrypted: _ } => self
                        .messages_repo
                        .update_message_status(
                            bs58::encode(&obj.hash).into_string(),
                            MessageStatus::Sent,
                        )
                        .await
                        .unwrap(),
                    _ => {}
                }

                let inventory = self.inventory_repo.get().await.expect("repo not to fail");
                let msg = NetworkMessage {
                    command: MessageCommand::Inv,
                    payload: MessagePayload::Inv { inventory },
                };
                let result = self.publish_pubsub(msg);
                match result {
                    Err(e) => {
                        log::error!("Pubsub failed to publish the message: {}", e);
                    }
                    _ => {}
                }
            }
            WorkerCommand::GetOwnIdentities { sender } => {
                let result = self.address_repo.get_identities().await;
                match result {
                    Ok(a) => {
                        log::debug!(
                            "GetOwnIdentities: fetched {} identities, sending reply",
                            a.len()
                        );
                        if let Err(_err) = sender.send(Ok(a)) {
                            log::error!("GetOwnIdentities: oneshot receiver dropped before reply could be sent");
                        } else {
                            log::debug!("GetOwnIdentities: reply sent");
                        }
                    }
                    Err(e) => {
                        sender
                            .send(Err(anyhow!(e.to_string())))
                            .expect("receiver not to be dropped");
                        return;
                    }
                }
            }
            WorkerCommand::GenerateIdentity { label, sender } => {
                let mut address = Address::generate();
                address.label = label;
                let res = self.address_repo.store(address.clone()).await;
                match res {
                    Ok(_) => {
                        sender
                            .send(Ok(address.string_repr))
                            .expect("receiver not to be dropped");
                    }
                    Err(e) => sender
                        .send(Err(anyhow!(e.to_string())))
                        .expect("receiver not to be dropped"),
                }
            }
            WorkerCommand::RenameIdentity {
                new_label,
                address,
                sender,
            } => match self.address_repo.update_label(address, new_label).await {
                Ok(_) => {
                    sender.send(Ok(())).expect("receiver not to be dropped");
                }
                Err(e) => sender
                    .send(Err(anyhow!(e.to_string())))
                    .expect("receiver not to be dropped"),
            },
            WorkerCommand::DeleteIdentity { address, sender } => {
                match self.address_repo.delete_address(address).await {
                    Ok(_) => {
                        sender.send(Ok(())).expect("receiver not to be dropped");
                    }
                    Err(e) => sender
                        .send(Err(anyhow!(e.to_string())))
                        .expect("receiver not to be dropped"),
                }
            }
            WorkerCommand::GetMessages {
                address,
                folder,
                sender,
            } => match folder {
                Folder::Inbox => {
                    match self.messages_repo.get_messages_by_recipient(address).await {
                        Ok(v) => sender.send(Ok(v)).expect("receiver not to be dropped"),
                        Err(e) => sender
                            .send(Err(anyhow!(e.to_string())))
                            .expect("receiver not to be dropped"),
                    }
                }
                Folder::Sent => match self.messages_repo.get_messages_by_sender(address).await {
                    Ok(v) => sender.send(Ok(v)).expect("receiver not to be dropped"),
                    Err(e) => sender
                        .send(Err(anyhow!(e.to_string())))
                        .expect("receiver not to be dropped"),
                },
            },
            WorkerCommand::SendMessage {
                mut msg,
                from,
                sender,
            } => {
                let identity = self
                    .address_repo
                    .get_by_ripe_or_tag(from)
                    .await
                    .unwrap()
                    .unwrap();
                let recipient: Option<Address> = self
                    .address_repo
                    .get_by_ripe_or_tag(msg.recipient.clone())
                    .await
                    .unwrap();
                match recipient {
                    Some(v) => {
                        msg.status = MessageStatus::WaitingForPOW.to_string();
                        let object = create_object_from_msg(&identity, &v, msg.clone());
                        msg.hash = bs58::encode(&object.hash).into_string();
                        self.messages_repo.save_model(msg).await.unwrap();
                        enqueue_pow(self.pow_worker_command_sink.clone(), object).await;
                    }
                    None => {
                        let recipient_address = Address::with_string_repr(msg.recipient.clone());
                        match recipient_address {
                            Ok(addr) => {
                                self.address_repo.store(addr.clone()).await.unwrap();
                                msg.status = MessageStatus::WaitingForPubkey.to_string();
                                // we generate random hash value, cuz we don't really know real hash value of the message at the moment, and it's not that important
                                msg.hash = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
                                self.messages_repo.save_model(msg.clone()).await.unwrap();
                                self.tracked_pubkeys
                                    .insert(bs58::encode(addr.tag).into_string(), true);
                                // send getpubkey request
                                let obj = Object::with_signing(
                                    &identity,
                                    ObjectKind::Getpubkey {
                                        tag: Address::new(
                                            bs58::decode(msg.recipient).into_vec().unwrap(),
                                        )
                                        .tag,
                                    },
                                    Utc::now() + chrono::Duration::days(7),
                                );
                                enqueue_pow(self.pow_worker_command_sink.clone(), obj).await;
                            }
                            Err(e) => {
                                sender
                                    .send(Err(anyhow!(
                                        "Recipient address is invalid: {}",
                                        e.to_string()
                                    )))
                                    .expect("receiver not to be dropped");
                                return;
                            }
                        };
                    }
                }
                sender.send(Ok(())).unwrap();
            }
            _ => {}
        };
    }

    fn publish_pubsub(&mut self, msg: NetworkMessage) -> Result<MessageId, PublishError> {
        let serialized_msg = serde_cbor::to_vec(&msg).unwrap();
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.common_topic.clone(), serialized_msg)
    }

    pub async fn run(mut self) {
        task::spawn(self.pow_worker.take().unwrap().run());

        // populate tracked_pubkeys map
        let msgs_waiting_for_pubkey = self
            .messages_repo
            .get_messages_by_status(MessageStatus::WaitingForPubkey)
            .await
            .unwrap();
        for m in msgs_waiting_for_pubkey {
            if self
                .address_repo
                .get_by_ripe_or_tag(m.recipient.clone())
                .await
                .unwrap()
                .unwrap()
                .public_encryption_key
                .is_some()
            {
                self.messages_repo
                    .update_message_status(m.hash, MessageStatus::WaitingForPOW)
                    .await
                    .expect("db won't to fail");
            } else {
                let tag = bs58::encode(
                    self.address_repo
                        .get_by_ripe_or_tag(m.recipient)
                        .await
                        .unwrap()
                        .unwrap()
                        .tag,
                )
                .into_string();
                self.tracked_pubkeys.insert(tag, true);
            }
        }

        // cleanup expired objects from the storage
        self.inventory_repo.cleanup().await.unwrap();

        debug!("node worker event loop started");
        loop {
            select! {
                event = self.swarm.select_next_some() => self.handle_event(event).await,
                command = self.command_receiver.recv() => match command {
                    Some(c) => {
                        match c {
                            WorkerCommand::Shutdown => {
                                log::debug!("Shutting down network event loop...");
                                return;
                            },
                            _ => self.handle_command(c).await,
                        }
                    }
                    // Command channel closed, thus shutting down the network event loop.
                    None => {
                        log::debug!("Shutting down network event loop...");
                        return;
                    },
                },
                pubkey_notification = self.pubkey_notifier.recv() => self.handle_pubkey_notification(pubkey_notification.unwrap()).await,
                else => {
                    log::debug!("Shutting down network event loop...");
                    return;
                }
            }
        }
    }

    async fn handle_pubkey_notification(&mut self, tag: String) {
        if let Some(_) = self.tracked_pubkeys.get(&tag) {
            let addr = self
                .address_repo
                .get_by_ripe_or_tag(tag.clone())
                .await
                .unwrap()
                .expect("Address entity exists in db");
            let msgs = self
                .messages_repo
                .get_messages_by_recipient(addr.string_repr.clone())
                .await
                .unwrap();
            futures::stream::iter(msgs.into_iter())
                .filter(|x| future::ready(x.status == MessageStatus::WaitingForPubkey.to_string()))
                .for_each(|x| {
                    let address_repo = self.address_repo.clone();
                    let mut messages_repo = self.messages_repo.clone();
                    let addr = addr.clone();
                    let pow_worker_command_sink = self.pow_worker_command_sink.clone();
                    async move {
                        let identity = address_repo
                            .get_by_ripe_or_tag(x.sender.clone())
                            .await
                            .unwrap()
                            .expect("identity exists in address repo");
                        let object = create_object_from_msg(&identity, &addr, x.clone());
                        let old_hash = x.hash.clone();
                        let new_hash = bs58::encode(&object.hash).into_string();
                        messages_repo
                            .update_hash(old_hash, new_hash.clone())
                            .await
                            .unwrap();

                        messages_repo
                            .update_message_status(new_hash, MessageStatus::WaitingForPOW)
                            .await
                            .unwrap();
                        enqueue_pow(pow_worker_command_sink, object).await;
                    }
                })
                .await;

            self.tracked_pubkeys.remove(&tag);
        }
    }

    /// When we receive IdentityInfo, if the peer supports our Kademlia protocol, we add
    /// their listen addresses to the DHT, so they will be propagated to other peers.
    fn handle_identify_event(&mut self, identify_event: identify::Event) {
        debug!("Received identify::Event: {:?}", identify_event);

        if let identify::Event::Received {
            peer_id,
            info:
                identify::Info {
                    listen_addrs,
                    protocols,
                    ..
                },
        } = identify_event
        {
            if protocols
                .iter()
                .any(|p| p.as_bytes() == KADEMLIA_PROTO_NAME)
            {
                for addr in listen_addrs {
                    debug!("Adding received IdentifyInfo matching protocol '{}' to the DHT. Peer: {}, addr: {}", String::from_utf8_lossy(KADEMLIA_PROTO_NAME), peer_id, addr);
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr);
                }

                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .add_explicit_peer(&peer_id);
            }
        }
    }

    pub fn serialize_and_encrypt_payload<T>(
        object: T,
        secret_key: &libsecp256k1::SecretKey,
    ) -> Vec<u8>
    where
        T: Serialize,
    {
        let encrypted = ecies::encrypt(
            &ecies::PublicKey::from_secret_key(secret_key).serialize(),
            serde_cbor::to_vec(&object).unwrap().as_ref(),
        )
        .unwrap();
        encrypted
    }

    fn on_new_peer(&mut self, peer_id: PeerId) {
        self.swarm.behaviour_mut().rpc.send_request(
            &peer_id,
            BitmessageRequest(NetworkMessage {
                command: MessageCommand::ReqInv,
                payload: MessagePayload::None,
            }),
        );
    }
}

async fn enqueue_pow(sink: mpsc::Sender<ProofOfWorkWorkerCommand>, object: Object) {
    let sink = sink.clone();
    sink.send(ProofOfWorkWorkerCommand::EnqueuePoW { object })
        .await
        .expect("command successfully sent");
}

fn extract_peer_id_from_multiaddr(
    address_with_peer_id: &Multiaddr,
) -> Result<PeerId, Box<dyn Error>> {
    match address_with_peer_id.iter().last() {
        Some(multiaddr::Protocol::P2p(hash)) => PeerId::from_multihash(hash).map_err(|multihash| {
            format!(
                "Invalid PeerId '{:?}' in Multiaddr '{}'",
                multihash, address_with_peer_id
            )
            .into()
        }),
        _ => Err("Multiaddr does not contain peer_id".into()),
    }
}

pub fn create_object_from_msg(
    identity: &Address,
    recipient: &Address,
    msg: models::Message,
) -> Object {
    let unenc_msg = UnencryptedMsg {
        behavior_bitfield: 0,
        sender_ripe: msg.sender.clone(),
        destination_ripe: msg.recipient.clone(),
        encoding: MsgEncoding::Simple,
        message: msg.data.clone(),
        public_encryption_key: recipient
            .public_encryption_key
            .unwrap()
            .serialize()
            .to_vec(),
        public_signing_key: identity.public_signing_key.unwrap().serialize().to_vec(),
    };
    let encrypted =
        serialize_and_encrypt_payload_pub(unenc_msg, &recipient.public_encryption_key.unwrap());
    Object::with_signing(
        &identity,
        ObjectKind::Msg { encrypted },
        Utc::now() + chrono::Duration::days(7), // FIXME
    )
}

pub fn serialize_and_encrypt_payload_pub<T>(
    object: T,
    public_key: &libsecp256k1::PublicKey,
) -> Vec<u8>
where
    T: Serialize,
{
    let encrypted = ecies::encrypt(
        &public_key.serialize(),
        serde_cbor::to_vec(&object).unwrap().as_ref(),
    )
    .unwrap();
    encrypted
}
