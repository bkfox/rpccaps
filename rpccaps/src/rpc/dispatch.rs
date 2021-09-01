use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::{RwLock, atomic::{AtomicU32, Ordering}};
use std::pin::Pin;

use futures::prelude::*;
use serde::{Deserialize,Serialize};
use futures::io::{AsyncRead,AsyncWrite};

use super::Error;
use super::codec::{BincodeCodec,Decoder,Framed};
use super::service::Service;


pub type HandlerFn<D> = Box<dyn Send+Sync+Unpin+Fn(D) -> Pin<Box<dyn Future<Output=()>+Send>>>;

/// Dispatch handler information
pub struct Handler<D> {
    /// Function to call returning pin-boxed future.
    pub func: HandlerFn<D>,
    /// If true, remove handler after call.
    pub once: bool,
}


/// Data dispatch to handler by Id, able to spawn tasks.
pub struct Dispatch<Id,D>
    where Id: std::cmp::Ord
{
    pub handlers: RwLock<BTreeMap<Id, Handler<D>>>,
    pub count: AtomicU32,
    pub max_count: Option<u32>,
    phantom: PhantomData<()>,
}

impl<Id,D> Dispatch<Id,D>
    where Id: std::cmp::Ord+Send+Sync,
          D: Send+Sync
{
    pub fn new(max_count: Option<u32>) -> Self {
        Self { handlers: RwLock::new(BTreeMap::new()),
               count: AtomicU32::new(0),
               max_count, phantom: PhantomData }
    }

    pub fn add(&self, id: Id, func: HandlerFn<D>, once: bool) -> Result<(), Error>
    {
        let handler = Handler { func, once };
        match self.handlers.write() {
            Ok(mut handlers) => match handlers.insert(id, handler) {
                None => Ok(()),
                Some(_) => Err(Error::KeyError),
            },
            _ => Err(Error::Internal),
        }
    }

    pub fn remove(&self, id: &Id) {
        self.handlers.write().unwrap().remove(&id);
    }

    pub async fn dispatch(&self, id: Id, data: D) -> Result<(), Error> {
        if let Some(max_count) = self.max_count {
            if self.count.load(Ordering::Relaxed) >= max_count {
                return Err(Error::TooManyTasks);
            }
        }
        self.count.fetch_add(1, Ordering::Relaxed);

        // we need to keep handlers reading out of future awaiting in order
        // to avoid deadlock/latency among dispatch tasks (e.g. when
        // unregistering once handlers.
        let (fut, once) = {
            match self.handlers.read() {
                Ok(handlers) => match handlers.get(&id) {
                    None => return Err(Error::KeyError),
                    Some(handler) => ((handler.func)(data), handler.once)
                },
                Err(_) => return Err(Error::Internal),
            }
        };

        fut.await;

        if once {
            self.remove(&id);
        }

        // FIXME: handling task cancelation, count may not be substracted
        self.count.fetch_sub(1, Ordering::Relaxed);
        Ok(())
    }
}


/// Implement Dispatch with ``(AsyncWrite, AsyncRead)`` as ``Data``.
impl<Id,S,R,D> Dispatch<Id,(S,R,D)>
    where for<'de> Id: std::cmp::Ord+Send+Sync+Deserialize<'de>,
          S: 'static+AsyncWrite+Unpin+Sync+Send,
          R: 'static+AsyncRead+Unpin+Sync+Send,
          D: 'static+Sync+Send,
{
    /// Register a service using factory function.
    /// FIXME: generic codec
    pub fn add_builder<F,Sv>(&self, id: Id, builder: Box<F>, once: bool)
            -> Result<(), Error>
        where F: 'static+Send+Sync+Unpin+Fn(D)->Sv,
              Sv: 'static+Send+Sync+Service,
              for <'de> Sv::Request: Deserialize<'de>, Sv::Response: Serialize
    {
        let handler = Box::new(move |(sender, receiver, data)| {
            let (encoder, decoder) = (BincodeCodec::new(), BincodeCodec::new());
            builder(data).serve_stream((sender, receiver), encoder, decoder)
        });
        self.add(id, handler, once)
    }

    /// Dispatch ``(sender, receiver)`` to service. Uses provided
    /// codec ``C`` to decode handler's Id.
    pub async fn dispatch_stream<C>(&self, (sender, receiver, data): (S,R,D))
            -> Result<(), Error>
        where C: Default+Decoder<Item=Id>+Unpin
    {
        let mut codec = Framed::new(receiver, C::default());
        let id = match codec.next().await {
            Some(id) => id,
            _ => return Err(Error::Codec),
        };

        let receiver = codec.into_inner();
        self.dispatch(id, (sender, receiver, data)).await
    }

}


#[cfg(test)]
pub mod tests {
    use std::sync::{Arc,RwLock};
    use futures::executor::LocalPool;

    use super::*;

    pub struct TestDispatch {
        pub result: Arc<RwLock<i64>>,
        pub dispatch: Dispatch<&'static str,(i64,i64)>,
    }

    impl TestDispatch {
        pub fn new(max_count: Option<u32>) -> Self {
            let dispatch = Dispatch::new(max_count);
            let result = Arc::new(RwLock::new(0i64));

            let res = result.clone();
            dispatch.add("add", Box::new(move |(a,b)| {
                let res = res.clone();
                Box::pin(async move {
                    let mut result = res.write().unwrap();
                    *result = a+b;
                })
            }), false).unwrap();

            let res = result.clone();
            dispatch.add("sub", Box::new(move |(a,b)| {
                let res = res.clone();
                Box::pin(async move {
                    let mut result = res.write().unwrap();
                    *result = a-b;
                })
            }), false).unwrap();

            let res = result.clone();
            dispatch.add("add_once", Box::new(move |(a,b)| {
                let res = res.clone();
                Box::pin(async move {
                    let mut result = res.write().unwrap();
                    *result = a+b;
                })
            }), true).unwrap();

            Self { result, dispatch }
        }

        pub fn result(&self) -> i64 {
            *self.result.read().unwrap()
        }
    }

    impl ::std::ops::Deref for TestDispatch {
        type Target = Dispatch<&'static str, (i64, i64)>;

        fn deref(&self) -> &Self::Target {
            &self.dispatch
        }
    }

    impl ::std::ops::DerefMut for TestDispatch {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.dispatch
        }
    }

    #[test]
    fn test_dispatch() {
        LocalPool::new().run_until(async {
            let test = TestDispatch::new(None);
            test.dispatch(&"add", (2,3)).await.unwrap();
            assert_eq!(test.result(), 5);

            test.dispatch(&"sub", (3,1)).await.unwrap();
            assert_eq!(test.result(), 2);
        })
    }

    #[test]
    fn test_dispatch_once() {
        LocalPool::new().run_until(async {
            let test = TestDispatch::new(None);
            test.dispatch(&"add_once",(2,3)).await.unwrap();
            assert_eq!(test.result(), 5);
            assert_eq!(test.dispatch(&"add_once",(2,3)).await.unwrap_err(),
                       Error::KeyError);
        })
    }

    // TODO:
    // - test max_count
    // - test dispatch_transport

    /*
    #[test]
    fn test_dispatch_max_count() {
        LocalPool::new().run_until(async {
            let test = TestDispatch::new(Some(2));
            let fut_0 = test.dispatch(&"add", (2,3));
            let fut_1 = test.dispatch(&"add", (5,7));
            let fut_2 = test.dispatch(&"sub", (13,12));

            assert_eq!(fut_2.await.unwrap_err(), Error::TooManyTasks);
        })
    }*/

}


