use async_trait::async_trait;
use futures::prelude::*;
use serde::{Serialize,de::DeserializeOwned};
use tokio::io::{AsyncRead,AsyncWrite};
use quinn::Connection;

use crate::data::{Reference,signature::PublicKey};
use super::codec::BincodeCodec;
use super::message::{Message,Error};
use super::transport::Transport;

// TODO:
// - service_derive:
//      - #[rpc(is_alive)] => use this method as is_alive result
//      - request timeout
//      - impl context argument in all service methods calls

pub struct Context
{
    /// Service router id
    pub service_id: u64,
    /// Connection being used
    pub connection: Connection,
    /// Remote peer id key
    pub peer_key: Option<PublicKey>,
}


/// Generic Service trait that handling requests and call corresponding RPC method.
#[async_trait]
pub trait Service: Send+Sync+Unpin
{
    /// Request message
    type Request: Sized+Send+Sync+Unpin;
    /// Response message
    type Response: Sized+Send+Sync+Unpin;

    /// Return True if service should be kept alive
    fn is_alive(&self) -> bool;

    /// Dispatch request
    async fn dispatch(&mut self, request: Self::Request) -> Option<Self::Response>;

    /// Serve using provided transport
    async fn serve<T,E>(&mut self, mut transport: T)
        where T: Stream<Item=Self::Request>+Sink<Self::Response,Error=E>+Unpin+Send,
              E: Unpin+Send
    {
        while let (Some(req), true) = (transport.next().await, self.is_alive()) {
            match self.dispatch(req).await {
                Some(resp) => { transport.send(resp).await; },
                _ => (),
            }
        }
    }
}


/// Run service for provided sender/receiver using bincode format.
async fn serve_bincode<F,S,R,FR,FS>(service: &mut F, sender: S, receiver: R) -> Result<(),()>
    where F: Service<Request=FR, Response=FS>,
          FR: Serialize+DeserializeOwned+Send+Sync+Unpin,
          FS: Serialize+DeserializeOwned+Send+Sync+Unpin,
          S: AsyncWrite+Unpin+Send, R: AsyncRead+Unpin+Send,
{
    let codec = BincodeCodec::<ServiceMessage<F>>::new();
    let transport = Transport::framed(sender, receiver, codec);
    service.serve(transport).await.or(Err(()))
}


/// Message type for a provided Service.
pub type ServiceMessage<S> = Message<<S as Service>::Request, <S as Service>::Response>;


#[cfg(test)]
mod test {
    use futures::future::join;
    use futures::executor::LocalPool;

    use crate as rpccaps;
    use super::Service;
    use crate::rpc::transport::MPSCTransport;
    use rpccaps_derive::*;

    pub struct SimpleService {
        a: u32,
    }

    impl SimpleService {
        fn new() -> Self {
            Self { a: 0 }
        }
    }

    #[service]
    impl SimpleService {
        fn clear(&mut self) {
            self.a = 0;
        }

        fn add(&mut self, a: u32) -> u32 {
            self.a += a;
            self.a
        }

        async fn sub(&mut self, a: u32) -> u32 {
            self.a -= a;
            self.a
        }

        async fn get(&mut self) -> u32 {
            self.a
        }
    }

    use super::*;
    use rpccaps::rpc::Transport;
    use futures::stream::StreamExt;

    #[test]
    fn test_request_response() {
        let (client_transport, server_transport) = MPSCTransport::<service::Message, service::Message>::bi(8);

        let client_fut = async move {
            let mut client = service::Client::new(client_transport);
            assert_eq!(client.add(13).await, Ok(13));
            assert_eq!(client.sub(1).await, Ok(12));
            client.clear().await;
            assert_eq!(client.get().await, Ok(0));
        };

        let server_fut = async move {
            let (s,r) = server_transport.split();
            let transport = Transport::new(s, r.then(|x| async { Ok(x) }).boxed());
            let mut service = SimpleService::new();
            service.serve(transport).await.unwrap();
        };

        LocalPool::new().run_until(join(client_fut, server_fut));
    }
}


