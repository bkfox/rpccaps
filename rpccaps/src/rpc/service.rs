use async_trait::async_trait;
use futures::prelude::*;
use futures::io::{AsyncRead,AsyncWrite};
use tokio_util::codec::{Decoder,Encoder};

use super::codec::Framed;
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
        let stream = Framed::new(receiver, decoder);
        let sink = Framed::new(sender, encoder);
        self.serve(Transport::new(sink,stream)).await
    }

    /// Run service for provided sender/receiver using bincode format.
    fn client_transport<S,R,E,D>((sender, receiver): (S,R),
                                 encoder: E, decoder: D)
        -> Transport<Framed<S,E>, Framed<R,D>>
        where Self: Sized,
              S: AsyncWrite+Send+Unpin,
              R: AsyncRead+Send+Unpin,
              E: Encoder<Self::Response>+Send+Unpin,
              E::Error: Send+Unpin,
              D: Decoder<Item=Self::Request>+Send+Unpin
    {
        let stream = Framed::new(receiver, decoder);
        let sink = Framed::new(sender, encoder);
        Transport::new(sink,stream)
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

    pub mod simple_service {
        use super::*;
        
        pub struct Service {
            a: u32,
        }

        impl Service {
            pub fn new() -> Self {
                Self { a: 0 }
            }
        }

        #[service]
        impl Service {
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
    }

    pub mod simple_service_2 {
        use super::*;
        
        pub struct Service {
            a: f32,
        }

        impl Service {
            pub fn new() -> Self {
                Self { a: 1.0 }
            }
        }

        #[service]
        impl Service {
            pub fn clear(&mut self) {
                self.a = 1.0;
            }

            pub fn mul(&mut self, a: f32) -> f32 {
                self.a *= a;
                self.a
            }

            async fn div(&mut self, a: f32) -> Result<f32, ()> {
                match a {
                    0.0 => Err(()),
                    x => {
                        self.a /= x;
                        Ok(self.a)
                    }
                }
            }

            async fn get(&mut self) -> f32 {
                self.a
            }
        }
    }

    use super::*;
    use rpccaps::rpc::Transport;
    use futures::stream::StreamExt;

    #[test]
    fn test_request_response() {
        let (server_transport, client_transport) = MPSCTransport::<simple_service::Response, simple_service::Request>::bi(8);

        let client_fut = async move {
            let mut client = simple_service::Client::new(client_transport);
            assert_eq!(client.add(13).await, Ok(13));
            assert_eq!(client.sub(1).await, Ok(12));
            client.clear().await;
            assert_eq!(client.get().await, Ok(0));
        };

        let server_fut = async move {
            let (s,r) = server_transport.split();
            let transport = Transport::new(s, r);
            let mut service = simple_service::Service::new();
            service.serve(transport).await;
        };

        LocalPool::new().run_until(join(client_fut, server_fut));
    }
}


