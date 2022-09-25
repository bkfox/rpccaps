use std::{
    net::SocketAddr,
    sync::Arc,
};


use futures::prelude::*;
use tokio::runtime::Runtime;
use serde::{Deserialize,Serialize};

use crate::{ErrorKind, Result};
use super::codec::BincodeCodec;
use super::dispatch::Dispatch;
use super::config::ServerConfig;



/// Trait defining server context passed down to services at dispatch.
pub trait ServerContext {
    /// Create new server context with provided endpoint and connection
    fn server_context(endpoint: quinn::Endpoint, connection: quinn::Connection) -> Self;
}

/// Default server context passed down to server.
pub struct DefaultServerContext
{
    pub endpoint: quinn::Endpoint,
    pub connection: quinn::Connection,
}


pub type IncomingStream<C> = (quinn::SendStream, quinn::RecvStream, Arc<C>);


/// Server dispatching incoming requests to services, and using Bincode
/// for messages' de-serialization, and QUIC for communication.
///
/// ```
/// let config = ServerConfig::new()
/// let mut server = Server::<u64, DefaultServerContext>::new(config);
/// // init handlers
/// let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 6677);
/// let (endpoint, mut incomings) = server.endpoint(&address).unwrap();
/// let fut = server.dispatch_incoming(runtime, endpoint, incoming);
/// // spawn fut
/// ```
/// 
struct Server<Id, C>
    where Id: std::cmp::Ord,
          C: ServerContext
{
    /// Services dispatch.
    pub dispatch: Arc<Dispatch<Id,IncomingStream<C>>>,
    /// Server configuration
    pub config: ServerConfig,
}


impl ServerContext for DefaultServerContext {
    fn server_context(endpoint: quinn::Endpoint, connection: quinn::Connection) -> Self {
        Self { endpoint, connection }
    }
}


impl<Id, C> Server<Id, C>
    where for<'de> Id: 'static+std::cmp::Ord+Send+Sync+Deserialize<'de>+Unpin,
                   C: 'static+ServerContext+Send+Sync
{
    /// Create new server.
    pub fn new(config: ServerConfig) -> Self {
        Self {
            // max dispatch is handled by ServerConfig::concurrent_streams
            dispatch: Arc::new(Dispatch::new(None)),
            config: config,
        }
    }

    /// Listen at provided address, dispatching services on provided runtime.
    pub async fn listen(&mut self, address: SocketAddr, runtime: Arc<Runtime>)
        -> Result<()>
    {
        let (endpoint, incoming) = self.get_endpoint(address)?;
        self.dispatch_incoming(runtime, endpoint, incoming).await
    }

    /// Return new endpoint binding to provided address.
    pub fn get_endpoint(&mut self, address: SocketAddr)
        -> Result<(quinn::Endpoint, quinn::Incoming)>
    {
        let server_config = self.config.get_server_config()?;
        quinn::Endpoint::server(server_config, address)
                .or(ErrorKind::Endpoint.err("can't init endpoint"))
    }

    /// Listen to incoming connections and dispatch them to services
    pub async fn dispatch_incoming(&mut self, runtime: Arc<Runtime>,
                        endpoint: quinn::Endpoint, mut incoming: quinn::Incoming)
        -> Result<()>
    {
        while let Some(conn) = incoming.next().await {
            let quinn::NewConnection {connection, bi_streams, .. } = conn.await.unwrap();
            let context = C::server_context(endpoint.clone(), connection);
            self.dispatch_streams(runtime.clone(), context, bi_streams);
        }
        Ok(())
    }

    /// Dispatch incoming bi_streams through the services.
    fn dispatch_streams(&self, runtime: Arc<Runtime>, context: C,
                            mut bi_streams: quinn::IncomingBiStreams)
    {
        let dispatch = self.dispatch.clone();
        let context = Arc::new(context);
        let runtime_ = runtime.clone();

        runtime.spawn(async move {
            while let Some(stream) = bi_streams.next().await {
                let (dispatch_, context) = (dispatch.clone(), context.clone()) ;
                runtime_.spawn(async move {
                    let stream = stream.unwrap();
                    let data = (stream.0, stream.1, context);
                    dispatch_.dispatch_stream::<BincodeCodec<Id>>(data).await
                });
            }
        });
    }
}


#[cfg(test)]
pub mod tests {
    use super::*;
    use std::{
        net::{Ipv4Addr}
    };

    #[test]
    fn test_server() {
        let server = Server::new(ServerConfig::new());
        
    }
}


