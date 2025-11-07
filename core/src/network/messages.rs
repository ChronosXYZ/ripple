use std::sync::Arc;

use crate::pow::{self, async_pow::AsyncPoW};
use chrono::Utc;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sha2::Digest;
use tokio::{runtime::Runtime, sync::mpsc};

use super::{address::Address, node::pow_worker::ProofOfWorkWorkerCommand};

pub type InventoryVector = Vec<String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
pub enum ObjectKind {
    Msg { encrypted: Vec<u8> },
    Broadcast { tag: Vec<u8>, encrypted: Vec<u8> },
    Getpubkey { tag: Vec<u8> },
    Pubkey { tag: Vec<u8>, encrypted: Vec<u8> },
}

impl ObjectKind {
    pub fn object_type(&self) -> u8 {
        match self {
            ObjectKind::Msg { .. } => 0,
            ObjectKind::Broadcast { .. } => 1,
            ObjectKind::Getpubkey { .. } => 2,
            ObjectKind::Pubkey { .. } => 3,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Object {
    pub hash: Vec<u8>,
    pub nonce: Vec<u8>,
    pub expires: i64,
    pub signature: Vec<u8>,
    pub kind: ObjectKind,
    pub nonce_trials_per_byte: i32,
    pub extra_bytes: i32,
}

impl Object {
    pub fn new(expires: i64, signature: Vec<u8>, kind: ObjectKind) -> Self {
        let mut hash_data: Vec<u8> = Vec::new();
        hash_data.extend_from_slice(&expires.to_le_bytes()[..]);
        hash_data.extend_from_slice(&signature);
        hash_data.extend_from_slice(&serde_cbor::to_vec(&kind).unwrap()[..]);
        let result = sha2::Sha256::digest(&hash_data);
        let hash: &[u8] = result.as_ref();
        Self {
            hash: hash.to_vec(),
            nonce: Vec::new(),
            expires,
            signature,
            kind,
            nonce_trials_per_byte: pow::NETWORK_MIN_NONCE_TRIALS_PER_BYTE,
            extra_bytes: pow::NETWORK_MIN_EXTRA_BYTES,
        }
    }

    pub fn with_signing(
        identity: &Address,
        kind: ObjectKind,
        expires: chrono::DateTime<Utc>,
    ) -> Self {
        let mut object = Self::new(expires.timestamp(), Vec::new(), kind);

        let ppsk =
            libsecp256k1::SecretKey::parse(&identity.private_signing_key.unwrap().serialize())
                .unwrap();
        let (signature, _) = libsecp256k1::sign(
            &libsecp256k1::Message::parse_slice(&object.hash).unwrap(),
            &ppsk,
        );
        object.signature = signature.serialize().to_vec();
        object
    }

    pub fn do_proof_of_work(
        mut self,
        runtime: Arc<Runtime>,
        worker_sink: mpsc::Sender<ProofOfWorkWorkerCommand>,
    ) {
        let target = pow::get_pow_target(
            &self,
            pow::NETWORK_MIN_NONCE_TRIALS_PER_BYTE,
            pow::NETWORK_MIN_EXTRA_BYTES,
        );

        runtime.spawn(async move {
            AsyncPoW::do_pow(target, self.hash.clone())
                .then(move |res| async move {
                    let (_, nonce) = res.unwrap();
                    self.nonce = nonce.to_bytes_be();
                    worker_sink
                        .send(ProofOfWorkWorkerCommand::NonceCalculated { object: self })
                        .await
                        .expect("receiver not to be dropped");
                })
                .await;
        });
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum MessagePayload {
    GetData { inventory: InventoryVector },
    Inv { inventory: InventoryVector },
    Objects { objects: Vec<Object> },
    None,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MessageCommand {
    GetData,
    Inv,
    ReqInv,
    Objects,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkMessage {
    pub command: MessageCommand,
    pub payload: MessagePayload,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone)]
#[repr(u8)]
pub enum MsgEncoding {
    Ignore = 0,
    Trivial = 1,
    Simple = 2,
    Extended = 3,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnencryptedMsg {
    pub behavior_bitfield: u32, // TODO currently unused
    pub sender_ripe: String,
    pub destination_ripe: String,
    pub encoding: MsgEncoding,
    pub message: Vec<u8>,
    pub public_signing_key: Vec<u8>,
    pub public_encryption_key: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnencryptedPubkey {
    pub behaviour_bitfield: u32, // TODO currently unused
    pub public_signing_key: Vec<u8>,
    pub public_encryption_key: Vec<u8>,
}
