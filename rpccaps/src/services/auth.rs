use futures::prelude::*;
use serde::{Serialize,Deserialize};


use crate::data::bytes;
use crate::data::signature::*;
use crate::data::reference::Reference;
use crate::rpc::service::Service;

#[derive(Serialize,Deserialize)]
pub enum Error {
}

#[derive(Serialize,Deserialize)]
pub enum Message {
    Request(Vec<u8>, #[serde(with="bytes")] Signature),
}

pub type Identity<Sign> = Reference<bytes::AsBytes<PublicKey>, Sign>;



pub struct Auth<S,Sign>
    where S: Service, Sign: SignMethod
{
    signer: Sign::Signer,
    service: S,
    peer: Option<Identity<Sign>>,
}


impl<S,Sign> Auth<S,Sign>
    where S: Service, Sign: SignMethod
{
    fn new(signer: Sign::Signer, service: S) -> Self {
        Self { signer, service, peer: None }
    }
}


impl<S,Sign> Auth<S,Sign>
    where S: Service, Sign: SignMethod
{
}

