use std::path::PathBuf;

use libp2p::Multiaddr;
use tokio::sync::mpsc;

use crate::network::node::worker::WorkerCommand;

use self::node::{client::NodeClient, worker::NodeWorker};

pub mod address;
pub(crate) mod behaviour;
pub(crate) mod messages;
pub mod node;

pub async fn new(
    bootstrap_nodes: Option<Vec<Multiaddr>>,
    data_dir: PathBuf,
) -> (NodeClient, NodeWorker) {
    let (worker, sender) = NodeWorker::new(bootstrap_nodes, data_dir).await;
    let client = NodeClient::new(sender);
    (client, worker)
}

pub async fn new_worker(
    bootstrap_nodes: Option<Vec<Multiaddr>>,
    data_dir: PathBuf,
) -> (NodeWorker, mpsc::Sender<WorkerCommand>) {
    let (worker, sender) = NodeWorker::new(bootstrap_nodes, data_dir).await;
    (worker, sender)
}
