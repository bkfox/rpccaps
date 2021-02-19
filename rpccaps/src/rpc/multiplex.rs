use std::collections::BTreeMap;

use async_trait::async_trait;
use futures::prelude::*;
use futures::future::BoxFuture;
use tokio::io::{AsyncRead,AsyncWrite};


pub type HandlerFn<S,R> = Box<dyn Unpin+Fn(S,R) -> Pin<Box<dyn Future<Output=()>>>>;

pub struct Handler<S,R> {
    pub func: HandlerFn<S,R>,
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

    pub fn register(&mut self, id: Id, func: HandlerFn<S,R>, once: bool) -> Result<(), HandlerFn<S,R>>
    {
        let handler = Handler { func, once };
        match self.handlers.insert(id, handler) {
            None => Ok(()),
            Some(h) => Err(h.func),
        }
    }

    pub fn unregister(&mut self, id: &Id) {
        self.handlers.remove(id);
    }

    async fn dispatch(&mut self, (id, sender, receiver): (Id, S, R)) -> Result<(), ()>
    {
        let handler = match self.handlers.get(&id) {
            None => return Err(()),
            Some(handler) => handler
        };

        let ref func = handler.func;
        let fut = func(sender, receiver);
        fut.await;

        if handler.once {
            self.unregister(&id);
        }
        Ok(())
    }
}

use std::pin::Pin;

#[cfg(test)]
mod test {
    use futures::executor::LocalPool;
    use super::*;

    #[test]
    fn test_multiplex_call() {
        LocalPool::new().run_until(async {
            let mut multiplex = Multiplex::<&str,i64,i64>::new();
            multiplex.register("add", Box::new(|s,r| Box::pin(async move { println!("----- {}", s+r) })), false);
            multiplex.register("sub", Box::new(|s,r| Box::pin(async move { println!("----- {}", s-r) })), false);

            multiplex.dispatch(("add",2,3)).await;
            multiplex.dispatch(("sub",3,1)).await;
        })
    }
}


