use async_trait::async_trait;
use futures::prelude::*;
use tokio::io::{AsyncRead,AsyncWrite};
use tokio_util::codec::{Decoder,Encoder,FramedRead,FramedWrite};

use super::transport::Transport;


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

    /// Service metadata
    fn metas() -> &'static [(&'static str, &'static str)] {
        static metas : [(&'static str, &'static str);0] = [];
        &metas
    }

    /// Dispatch request
    async fn dispatch(&mut self, request: Self::Request) -> Option<Self::Response>;

    /// Serve provided request-response transport
    async fn serve<T,E>(&mut self, mut transport: T)
        where T: Stream<Item=Self::Request>+Sink<Self::Response,Error=E>+Send+Unpin,
              E: Send+Unpin
    {
        while let (true, Some(req)) = (self.is_alive(), transport.next().await) {
            match self.dispatch(req).await {
                Some(resp) => match transport.send(resp).await {
                    Ok(_) => (),
                    Err(_) => break,
                }
                _ => (),
            }
        }
    }

    /// Run service for provided sender/receiver using bincode format.
    async fn serve_stream<S,R,E,D>(mut self, (sender, receiver): (S,R),
                                   encoder: E, decoder: D)
        where Self: Sized,
              S: AsyncWrite+Send+Unpin,
              R: AsyncRead+Send+Unpin,
              E: Encoder<Self::Response>+Send+Unpin,
              E::Error: Send+Unpin,
              D: Decoder<Item=Self::Request>+Send+Unpin,
    {
        let stream = FramedRead::new(receiver, decoder)
            .filter_map(|req| { future::ready(req.ok()) });
        let sink = FramedWrite::new(sender, encoder);
        self.serve(Transport::new(sink,stream)).await
    }
}



#[cfg(test)]
pub mod tests {
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
        pub fn new() -> Self {
            Self { a: 0 }
        }
    }

    #[service]
    impl SimpleService {
        pub fn clear(&mut self) {
            self.a = 0;
        }

        pub fn add(&mut self, a: u32) -> u32 {
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
        let (server_transport, client_transport) = MPSCTransport::<service::Response, service::Request>::bi(8);

        let client_fut = async move {
            let mut client = service::Client::new(client_transport);
            assert_eq!(client.add(13).await, Ok(13));
            assert_eq!(client.sub(1).await, Ok(12));
            client.clear().await;
            assert_eq!(client.get().await, Ok(0));
        };

        let server_fut = async move {
            let (s,r) = server_transport.split();
            let transport = Transport::new(s, r);
            let mut service = SimpleService::new();
            service.serve(transport).await;
        };

        LocalPool::new().run_until(join(client_fut, server_fut));
    }
}


