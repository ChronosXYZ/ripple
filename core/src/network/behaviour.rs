use std::io;

use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite, AsyncWriteExt};
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed},
    gossipsub, identify,
    kad::{record::store::MemoryStore, Kademlia, KademliaEvent},
    mdns,
    request_response::{self, Codec, ProtocolName},
    swarm::{keep_alive, NetworkBehaviour},
};
use log::error;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use void::Void;

#[derive(Debug, Clone)]
pub struct BitmessageProtocol();
#[derive(Clone)]
pub struct BitmessageProtocolCodec();

impl ProtocolName for BitmessageProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/bitmessage/1.0".as_bytes()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BitmessageRequest(pub super::messages::NetworkMessage);

#[derive(Serialize, Deserialize, Debug)]
pub struct BitmessageResponse(pub super::messages::NetworkMessage);

impl BitmessageProtocolCodec {
    async fn _read_data<T, B>(&self, io: &mut B) -> io::Result<T>
    where
        T: DeserializeOwned,
        B: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 10_000_000).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        let res: T = match serde_cbor::from_slice(vec.as_slice()) {
            Ok(v) => v,
            Err(e) => {
                error!("Deserialization error: {}", e);
                return io::Result::Err(io::ErrorKind::InvalidInput.into());
            }
        };

        Ok(res)
    }

    async fn _write_data<T, B>(&self, io: &mut B, data: T) -> io::Result<()>
    where
        T: Serialize,
        B: AsyncWrite + Unpin + Send,
    {
        let res = match serde_cbor::to_vec(&data) {
            Ok(vec) => vec,
            Err(e) => {
                error!("Serialization error: {}", e);
                return io::Result::Err(io::ErrorKind::InvalidInput.into());
            }
        };

        write_length_prefixed(io, res).await?;
        io.close().await?;

        Ok(())
    }
}

#[async_trait]
impl Codec for BitmessageProtocolCodec {
    type Protocol = BitmessageProtocol;
    type Request = BitmessageRequest;
    type Response = BitmessageResponse;

    async fn read_request<T>(
        &mut self,
        _: &BitmessageProtocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        self._read_data(io).await
    }

    async fn read_response<T>(
        &mut self,
        _: &BitmessageProtocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        self._read_data(io).await
    }

    async fn write_request<T>(
        &mut self,
        _: &BitmessageProtocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        self._write_data(io, req).await
    }

    async fn write_response<T>(
        &mut self,
        _: &BitmessageProtocol,
        io: &mut T,
        resp: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        self._write_data(io, resp).await
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BitmessageBehaviourEvent")]
pub struct BitmessageNetBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: Kademlia<MemoryStore>,
    pub rpc: request_response::Behaviour<BitmessageProtocolCodec>,
    pub mdns: mdns::async_io::Behaviour,
    pub keep_alive: keep_alive::Behaviour,
}

#[derive(Debug)]
pub enum BitmessageBehaviourEvent {
    RequestResponse(request_response::Event<BitmessageRequest, BitmessageResponse>),
    #[allow(dead_code)]
    Kademlia(KademliaEvent),
    Identify(identify::Event),
    Gossipsub(gossipsub::Event),
    Mdns(mdns::Event),
    Void,
}

impl From<request_response::Event<BitmessageRequest, BitmessageResponse>>
    for BitmessageBehaviourEvent
{
    fn from(event: request_response::Event<BitmessageRequest, BitmessageResponse>) -> Self {
        BitmessageBehaviourEvent::RequestResponse(event)
    }
}

impl From<KademliaEvent> for BitmessageBehaviourEvent {
    fn from(event: KademliaEvent) -> Self {
        BitmessageBehaviourEvent::Kademlia(event)
    }
}

impl From<identify::Event> for BitmessageBehaviourEvent {
    fn from(value: identify::Event) -> Self {
        BitmessageBehaviourEvent::Identify(value)
    }
}

impl From<gossipsub::Event> for BitmessageBehaviourEvent {
    fn from(value: gossipsub::Event) -> Self {
        BitmessageBehaviourEvent::Gossipsub(value)
    }
}

impl From<mdns::Event> for BitmessageBehaviourEvent {
    fn from(value: mdns::Event) -> Self {
        BitmessageBehaviourEvent::Mdns(value)
    }
}

impl From<Void> for BitmessageBehaviourEvent {
    fn from(_value: Void) -> Self {
        BitmessageBehaviourEvent::Void
    }
}
