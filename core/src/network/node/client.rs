use emailmessage::{header, Message, SinglePart};

use chrono::Utc;
use libp2p::{Multiaddr, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::{
    network::address::Address,
    repositories::sqlite::models::{self, MessageStatus},
};

use super::worker::{Folder, WorkerCommand};

#[derive(Clone)]
pub struct NodeClient {
    worker_sender: mpsc::Sender<WorkerCommand>,
}

impl NodeClient {
    pub fn new(sender: mpsc::Sender<WorkerCommand>) -> Self {
        Self {
            worker_sender: sender,
        }
    }

    pub async fn start_listening(&self, multiaddr: Multiaddr) -> anyhow::Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.worker_sender
            .send(WorkerCommand::StartListening { multiaddr, sender })
            .await
            .expect("Command receiver not to be dropped");
        receiver.await.expect("Sender not to be dropped")
    }

    pub async fn get_listeners(&self) -> Multiaddr {
        let (sender, receiver) = oneshot::channel();
        self.worker_sender
            .send(WorkerCommand::GetListenerAddress { sender })
            .await
            .expect("Command receiver not to be dropped");
        receiver.await.expect("Sender not to be dropped")
    }

    pub async fn get_peer_id(&self) -> PeerId {
        let (sender, receiver) = oneshot::channel();
        self.worker_sender
            .send(WorkerCommand::GetPeerID { sender })
            .await
            .expect("Command receiver not to be dropped");
        receiver.await.expect("Sender not to be dropped")
    }

    pub async fn shutdown(&self) {
        self.worker_sender
            .send(WorkerCommand::Shutdown)
            .await
            .expect("Command receiver not to be dropped");
    }

    pub async fn get_own_identities(&self) -> Vec<Address> {
        let (tx, rx) = oneshot::channel();
        self.worker_sender
            .send(WorkerCommand::GetOwnIdentities { sender: tx })
            .await
            .expect("Receiver not to be dropped");
        rx.await.expect("Sender not to be dropped").unwrap()
    }

    pub async fn generate_new_identity(&self, label: String) -> String {
        let (sender, receiver) = oneshot::channel();
        self.worker_sender
            .send(WorkerCommand::GenerateIdentity { label, sender })
            .await
            .expect("Receiver not to be dropped");
        receiver
            .await
            .expect("Sender not to be dropped")
            .expect("repo not to fail")
    }

    pub async fn delete_identity(&self, address: String) {
        let (sender, receiver) = oneshot::channel();
        self.worker_sender
            .send(WorkerCommand::DeleteIdentity { address, sender })
            .await
            .expect("Receiver not to be dropped");
        receiver
            .await
            .expect("Sender not to be dropped")
            .expect("repo not to fail")
    }

    pub async fn rename_identity(&self, address: String, new_label: String) {
        let (sender, receiver) = oneshot::channel();
        self.worker_sender
            .send(WorkerCommand::RenameIdentity {
                new_label,
                address,
                sender,
            })
            .await
            .expect("Receiver not to be dropped");
        receiver
            .await
            .expect("Sender not to be dropped")
            .expect("repo not to fail")
    }

    pub async fn get_messages(&self, address: String, folder: Folder) -> Vec<models::Message> {
        let (sender, receiver) = oneshot::channel();
        self.worker_sender
            .send(WorkerCommand::GetMessages {
                address,
                folder,
                sender,
            })
            .await
            .expect("Receiver not to be dropped");
        receiver
            .await
            .expect("Sender not to be dropped")
            .expect("repo not to fail")
    }

    pub async fn send_message(
        &self,
        from: String,
        to: String,
        title: String,
        body: String,
    ) -> anyhow::Result<()> {
        let (sender, receiver) = oneshot::channel();
        let m: Message<SinglePart<&str>> = Message::builder().subject(title).mime_body(
            SinglePart::builder()
                .header(header::ContentType(
                    "text/plain; charset=utf8".parse().unwrap(),
                ))
                .header(header::ContentTransferEncoding::QuotedPrintable)
                .body(&body),
        );
        let data = m.to_string().into_bytes();
        let msg = models::Message {
            hash: "".to_string(),
            sender: from.clone(),
            recipient: to,
            created_at: Utc::now(),
            status: MessageStatus::Unknown.to_string(),
            signature: Vec::new(),
            data,
        };

        self.worker_sender
            .send(WorkerCommand::SendMessage { msg, from, sender })
            .await
            .expect("Receiver not to be dropped");
        receiver.await.expect("Sender not to be dropped")?;
        Ok(())
    }
}
