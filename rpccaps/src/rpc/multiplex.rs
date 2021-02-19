use std::collections::BTreeMap;

use async_trait::async_trait;
use futures::prelude::*;
use tokio::io::{AsyncRead,AsyncWrite};


pub struct Handler<'a,S,R> {
    pub func: Box<dyn 'a+Unpin+Send+Sync+Fn(S, R) -> Box<dyn Send+Unpin+Future<Output=()>>>,
    pub once: bool,
    // TODO timeout
}


/// Low-level api to dispatch sender+receiver to handlers by id.
pub struct Multiplex<'a,Id,S,R> {
    pub handlers: BTreeMap<Id, Handler<'a,S,R>>,
}

impl<'a,Id,S,R> Multiplex<'a,Id,S,R>
    where Id: std::cmp::Ord,
{
    pub fn new() -> Self {
        Self { handlers: BTreeMap::new() }
    }

    pub fn register<F>(&mut self, id: Id, func: Box<F>, once: bool) -> Result<(), Handler<S,R>>
        where F: 'a+Send+Sync+Unpin+Fn(S,R) -> Box<dyn Send+Unpin+Future<Output=()>>
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
}


use super::service::Service;

#[async_trait]
impl<'a,Id,S,R> Service for Multiplex<'a,Id,S,R>
    where Id: std::cmp::Ord+Send+Sync+Unpin,
          S: AsyncWrite+Send+Sync+Unpin, R: AsyncRead+Send+Sync+Unpin
{
    type Request = (Id,S,R);
    type Response = (Id,S,R);

    fn is_alive(&self) -> bool {
        true
    }

    async fn dispatch(&mut self, (id, sender, receiver): Self::Request) -> Option<Self::Response>
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


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_multiplex_call() {
    }
}


