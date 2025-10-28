use queues::{queue, IsQueue, Queue};
use tokio::sync::mpsc;

use crate::{
    network::{address::Address, messages::Object},
    repositories::{
        address::AddressRepositorySync, inventory::InventoryRepositorySync,
        message::MessageRepositorySync, sqlite::models::MessageStatus,
    },
};

use super::worker::{create_object_from_msg, WorkerCommand};

pub enum ProofOfWorkWorkerCommand {
    EnqueuePoW { object: Object },
    NonceCalculated { object: Object },
}

pub struct ProofOfWorkWorker {
    inventory: Box<InventoryRepositorySync>,
    message_repo: Box<MessageRepositorySync>,
    address_repo: Box<AddressRepositorySync>,
    node_worker_sink: mpsc::Sender<WorkerCommand>,
    command_sink: mpsc::Sender<ProofOfWorkWorkerCommand>,
    command_receiver: mpsc::Receiver<ProofOfWorkWorkerCommand>,
    is_pow_running: bool,
    waiting_objects: Queue<Object>,
}

impl ProofOfWorkWorker {
    pub fn new(
        inv: Box<InventoryRepositorySync>,
        msg_repo: Box<MessageRepositorySync>,
        addr_repo: Box<AddressRepositorySync>,
        worker_sink: mpsc::Sender<WorkerCommand>,
    ) -> (ProofOfWorkWorker, mpsc::Sender<ProofOfWorkWorkerCommand>) {
        let (cmd_sink, cmd_receiver) = mpsc::channel(3);

        return (
            ProofOfWorkWorker {
                inventory: inv,
                message_repo: msg_repo,
                address_repo: addr_repo,
                node_worker_sink: worker_sink,
                command_sink: cmd_sink.clone(),
                command_receiver: cmd_receiver,
                waiting_objects: queue![],
                is_pow_running: false,
            },
            cmd_sink,
        );
    }

    pub async fn run(mut self) {
        let objects = self
            .inventory
            .get_missing_pow_objects()
            .await
            .expect("db won't fail");
        let msgs = self
            .message_repo
            .get_messages_by_status(MessageStatus::WaitingForPOW)
            .await
            .expect("db won't fail");
        for o in objects {
            self.enqueue_pow(o);
        }
        for m in msgs {
            let identity = self
                .address_repo
                .get_by_ripe_or_tag(m.sender.clone())
                .await
                .expect("db won't fail")
                .expect("address exists in db");
            let recipient: Address = self
                .address_repo
                .get_by_ripe_or_tag(m.recipient.clone())
                .await
                .expect("db won't fail")
                .expect("address exists in db");

            let obj = create_object_from_msg(&identity, &recipient, m.clone());
            self.message_repo
                .update_hash(m.hash, bs58::encode(obj.hash.clone()).into_string())
                .await
                .expect("db won't fail");
            self.inventory
                .store_object(obj.clone())
                .await
                .expect("db won't fail");
            self.enqueue_pow(obj);
        }

        loop {
            tokio::select! {
                Some(command) = self.command_receiver.recv() => {
                    match command {
                        ProofOfWorkWorkerCommand::EnqueuePoW { object } => {
                            self.inventory.store_object(object.clone()).await.expect("db won't fail");
                            self.enqueue_pow(object);
                        },
                        ProofOfWorkWorkerCommand::NonceCalculated { object } => {
                            self.inventory.update_nonce(bs58::encode(object.hash.clone()).into_string(), object.nonce.clone())
                                .await
                                .expect("db won't fail");
                            self.node_worker_sink.send(WorkerCommand::NonceCalculated { obj: object }).await.expect("command successfully sent");
                            match self.waiting_objects.remove() {
                                Ok(o) => {
                                    o.do_proof_of_work(self.command_sink.clone())
                                },
                                Err(_) => {
                                    self.is_pow_running = false;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn enqueue_pow(&mut self, object: Object) {
        if self.is_pow_running {
            self.waiting_objects.add(object).unwrap();
        } else {
            object.do_proof_of_work(self.command_sink.clone());
            self.is_pow_running = true;
        }
    }
}
