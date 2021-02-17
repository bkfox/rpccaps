use std::collections::BTreeMap;

use futures::prelude::*;
use serde::{Serialize,de::DeserializeOwned};
use tokio::io::{AsyncRead,AsyncWrite};

use super::codec::BincodeCodec;
use super::transport::Transport;


pub struct Handler<S,R> {
    pub func: Box<dyn Fn(S, R) -> Box<dyn Unpin+Future<Output=()>>>,
    pub once: bool,
    // TODO timeout
}


/// Low-level api to dispatch sender+receiver to handlers by id.
pub struct Multiplex<Id,S,R> {
    pub handlers: BTreeMap<Id, Handler<S,R>>,
}

impl<Id,S,R> Multiplex<Id,S,R>
    where Id: std::cmp::Ord,
{
    pub fn new() -> Self {
        Self { handlers: BTreeMap::new() }
    }

    pub fn register<F>(&mut self, id: Id, func: Box<F>, once: bool) -> Result<(), Handler<S,R>>
        where F: 'static+Fn(S,R) -> Box<dyn Unpin+Future<Output=()>>
    {
        let handler = Handler { func, once };
        match self.handlers.insert(id, handler) {
            None => Ok(()),
            Some(h) => Err(h),
        }
    }

    pub fn unregister(&mut self, id: &Id) {
        self.handlers.remove(id);
    }

    pub async fn dispatch(&mut self, id: Id, sender: S, receiver: R) -> Option<(Id,S,R)>
    {
        let handler = match self.handlers.get(&id) {
            None => return Some((id, sender, receiver)),
            Some(handler) => handler
        };

        let ref func = handler.func;
        func(sender, receiver).await;

        if handler.once {
            self.unregister(&id);
        }
        None
    }
}


use super::service::Service;

impl<Id,S,R> Service for Multiplex<Id,S,R>
    where Id: std::cmp::Ord+Send+Sync+Unpin,
          S: AsyncWrite+Unpin, R: AsyncRead+Unpin
{
    type Request = (Id,S,R);
    type Response = (Id,S,R);

    pub async fn serve(&mut self, sender: S, receiver: R) -> Result<Option<(Id,S,R)>,(S,R)> {
        let codec = BincodeCodec::<Id>::new();
        let mut transport = Transport::framed(sender, receiver, codec);
        let id = match transport.next().await {
            Some(Ok(id)) => id,
            _ => {
                let (sender, receiver) = transport.into_inner().into_inner();
                return Err((sender,receiver))
            }
        };

        let (sender, receiver) = transport.into_inner().into_inner();
        Ok(self.dispatch(id, sender, receiver).await)
    }
}



/*
        match self.transport.poll_next() {
            Poll::Ready(Some((id, sender, receiver))) => {
                self.get_mut().dispatch(id, sender, receiver).await;
                Poll::Pending,
            },
            poll => poll,
        }

        Pin::new(&mut self.get_mut().receiver).poll_next(cx)
    }
}
*/


