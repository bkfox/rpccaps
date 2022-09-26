use std::{
    net::SocketAddr,
    sync::Arc,
};


use futures::prelude::*;
use tokio::{
    self,
    runtime::Runtime,
};
use serde::{Deserialize,Serialize};

use crate::{ErrorKind, Result};
use super::codec::BincodeCodec;
use super::context::{Context, DefaultContext};
use super::dispatch::Dispatch;
use super::config::ServerConfig;


pub type IncomingStream<C> = (quinn::SendStream, quinn::RecvStream, Arc<C>);


/// Server dispatching incoming requests to services, and using Bincode
/// for messages' de-serialization, and QUIC for communication.
/// 
pub struct Server<Id=u64, C=DefaultContext>
    where Id: std::cmp::Ord,
          C: Context
{
    /// Services dispatch.
    pub dispatch: Arc<Dispatch<Id,IncomingStream<C>>>,
    /// Server configuration
    pub config: ServerConfig,
}


impl<Id, C> Server<Id, C>
    where for<'de> Id: 'static+std::cmp::Ord+Send+Sync+Deserialize<'de>+Unpin,
                   C: 'static+Context+Send+Sync
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
    pub async fn listen(&mut self, address: SocketAddr)
        -> Result<()>
    {
        let (endpoint, incoming) = self.get_endpoint(address)?;
        self.dispatch_incoming(endpoint, incoming).await
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
    pub async fn dispatch_incoming(&mut self, endpoint: quinn::Endpoint,
                                   mut incoming: quinn::Incoming)
        -> Result<()>
    {
        while let Some(conn) = incoming.next().await {
            let quinn::NewConnection {connection, bi_streams, .. } = conn.await.unwrap();
            let context = C::from_connection(endpoint.clone(), connection);
            self.dispatch_streams(context, bi_streams);
        }
        Ok(())
    }

    /// Dispatch incoming bi_streams through the services.
    fn dispatch_streams(&self, context: C, mut bi_streams: quinn::IncomingBiStreams)
    {
        let dispatch = self.dispatch.clone();
        let context = Arc::new(context);

        tokio::spawn(async move {
            while let Some(stream) = bi_streams.next().await {
                let (dispatch_, context) = (dispatch.clone(), context.clone()) ;
                tokio::spawn(async move {
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
        net::SocketAddr,
        str::FromStr,
    };
    use super::super::service::tests::{simple_service,simple_service_2};


    fn get_server() -> Server::<u32, DefaultContext> {
        let mut server = Server::new(ServerConfig::default());
        server.dispatch.add_builder(0, Box::new(move |context| {
            simple_service::Service::new()
        }), false).unwrap();
        server.dispatch.add_builder(1, Box::new(move |context| {
            simple_service_2::Service::new()
        }), false).unwrap();
        server
    }

    #[test]
    fn test_server() {
        let runtime = Runtime::new().unwrap();

        let mut server = get_server();
        let server_fut = async move {
            server.listen(SocketAddr::from_str("127.0.0.1:4433").unwrap()).await;
        };
    }
}


