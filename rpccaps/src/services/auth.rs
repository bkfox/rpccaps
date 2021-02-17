use std::pin::Pin;
use futures::prelude::*;
use futures::task::{Context,Poll};


use crate::rpc::service::{Scope,Service};
use crate::data::signature::{PublicKey,SignMethod,Signature};


#[derive(Serialize,Deserialize)]
pub enum Message {
    Request(Vec<u8>, Signature),
    // Auth(),
}

pub struct Auth<S,Sign>
    where S: Service, Sign: SignMethod
{
    service: S,
    scope: Scope<S::Id>,
    signer: Sign::Signer,
}


impl<S,Sign> Auth<S,Sign>
    where S: Service, Sign: SignMethod
{
    fn new(signer: Sign::Signer, service: S) -> Self {
        Self { signer, service }
    }
}

impl<S,Sign> Service for Auth<S,Sign>
    where S: Service, Sign: SignMethod,
{
    type Request = Message;
    type Response = Message;
    type Context = S::Context;

    /*
    fn poll_next(self: Pin<&mut Self>, cx: Context) -> Poll<Self::Item> {
        match Pin::new(&mut self.get_mut().transport).poll(cx) {
            Poll::Ready(AuthRequest::Request(req, sign)) => match bincode::deserialize::<Self::Item>(req) {
                Ok(req) => Poll::Ready(Some(req)),
                _ => Poll::Ready(None),
            },
            _ => Poll::NotReady,
        }
    }*/
}



