use async_trait::async_trait;
use futures::prelude::*;
use serde::{Serialize,de::DeserializeOwned};
use tokio_util::codec::Framed;
use tokio::io::{AsyncRead,AsyncWrite};

use super::codec::BincodeCodec;
use super::message::{Message,Error};
use super::transport::Transport;


/// Generic Service trait that handling requests and call corresponding RPC method.
#[async_trait]
pub trait Service: Send+Sync+Unpin {
    type Request: Sized+Send+Sync+Unpin+Serialize+DeserializeOwned;
    type Response: Sized+Send+Sync+Unpin+Serialize+DeserializeOwned;

    /// Return True if service should be kept alive
    fn is_alive(&self) -> bool;

    /// Dispatch request
    async fn dispatch(&mut self, request: Self::Request) -> Option<Self::Response>;

    /// Serve using provided transport
    async fn serve<T,E>(&mut self, mut transport: T)
        where T: Stream<Item=ServiceMessage<Self>>+Sink<ServiceMessage<Self>,Error=E>+Unpin+Send,
              E: Unpin+Send
    {
        while let (Some(msg), true) = (transport.next().await, self.is_alive()) {
            match msg {
                Message::Request(req) => match self.dispatch(req).await {
                    Some(resp) => { transport.send(Message::Response(resp)).await; },
                    _ => (),
                }
                _ => (),
            }
        }
    }

    /// Serve from asyncwrite and asyncread streams using Bincode de-serializers.
    async fn serve_bincode<S,R>(&mut self, send_stream: S, recv_stream: R)
        where S: AsyncWrite+Unpin+Send, R: AsyncRead+Unpin+Send,
    {
        let codec = BincodeCodec::<ServiceMessage<Self>>::new();
        let mut transport = Framed::new(Transport::new(send_stream, recv_stream), codec);
        while let (Some(msg), true) = (transport.next().await, self.is_alive()) {
            match msg {
                Ok(Message::Request(req)) => match self.dispatch(req).await {
                    Some(resp) => { transport.send(Message::Response(resp)).await; },
                    _ => (),
                }
                Err(_) => { transport.send(Message::Error(Error::Format)).await; break; },
                _ => (),
            }
        }
    }
}

/// Message type for a provided Service.
type ServiceMessage<S> = Message<<S as Service>::Request, <S as Service>::Response>;


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
            let mut service = SimpleService::new();
            service.serve(server_transport).await;
        };

        LocalPool::new().run_until(join(client_fut, server_fut));
    }
}


