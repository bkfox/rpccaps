use std::pin::Pin;
use futures::prelude::*;
use futures::task::{Context,Poll};


use crate::rpc::service::{Scope,Service};
use crate::data::signature::{PublicKey,SignMethod,Signature};

#[derive(Serialize,Deserialize)]
pub enum Error {
}

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


impl<S,Sign> Auth<S,Sign
    where S: Service, Sign: SignMethod
{
}

