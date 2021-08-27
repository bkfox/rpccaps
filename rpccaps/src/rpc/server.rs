use std::sync::{Arc,RwLock};
use std::pin::Pin;
use futures::prelude::*;
use tokio::io::{AsyncRead,AsyncWrite};
use tokio_util::codec::{Decoder,Encoder,FramedRead,FramedWrite};

use serde::{Deserialize,Serialize};

use super::codec::BincodeCodec;
use super::dispatch::{Dispatch,Error,HandlerFn};
use super::service::Service;


type ServiceBuilder<S> = Box<dyn Send+Sync+Unpin+Fn() -> S>;

struct ServerConfig {
    dispatch_max: Option<u32>,
    key_file: String,
}


/// Server class handling dispatching to services from incoming transport stream.
/// It uses BincodeCodec for messages de-serialization and Quic for communication.
struct Server<Id,Context,T,S,R>
    where Id: std::cmp::Ord,
          T: Stream<Item=(S,R)>,
          S: AsyncWrite+Send+Unpin,
          R: AsyncRead+Send+Unpin,
{
    incoming: T,
    pub dispatch: Arc<Dispatch<Id,(S,R)>>,
    pub context: Arc<RwLock<Context>>,
    config: ServerConfig,
}

impl<Id,Context,T,S,R> Server<Id,Context,T,S,R>
    where for<'de> Id: std::cmp::Ord+Send+Sync+Clone+Deserialize<'de>,
          T: Stream<Item=(S,R)>,
          S: 'static+AsyncWrite+Sync+Send+Unpin,
          R: 'static+AsyncRead+Sync+Send+Unpin,
{
    /// Create new server.
    pub fn new(incoming: T, context: Context, config: ServerConfig) -> Self {
        Self {
            incoming: incoming,
            dispatch: Arc::new(Dispatch::new(config.dispatch_max)),
            context: Arc::new(RwLock::new(context)),
            config: config,
        }
    }

    // TODO: stop()

    /// Register a service using factory function.
    pub fn register<Sv>(&self, id: Id, builder: ServiceBuilder<Sv>, once: bool)
            -> Result<(), Error>
        where Sv: 'static+Service,
              for <'de> Sv::Request: Deserialize<'de>, Sv::Response: Serialize
    {
        let handler: HandlerFn<(S,R)> = Box::new(move |stream| {
            let encoder = BincodeCodec::new();
            let decoder = BincodeCodec::new();
            builder().serve_stream(stream, encoder, decoder)
        });
        self.dispatch.register(id, handler, once)
    }

    pub fn unregister(&self, id: &Id) {
        self.dispatch.unregister(id)
    }

    /// Dispatch stream to service
    pub async fn dispatch_stream(&self, stream: (S,R)) -> Result<(), Error> {
        self.dispatch.dispatch_stream::<BincodeCodec<Id>>(stream).await
    }
}

// #[cfg(feature="network")]



#[cfg(test)]
pub mod tests {


    #[test]
    fn test_server() {
    }

}


