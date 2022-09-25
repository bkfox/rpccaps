// TODO:
// - auth a single identity
// - auth flow
//   - nonce & key exchange
//     - certificate validation
//   - auth signature exchange
//   - expiration and renewal
// - auth multiple identities -> stream per identity
//      - use of channel id or Dispatch
// - reference:
//   - expiration timeout
//

use futures::prelude::*;
use serde::{Serialize,Deserialize};


use crate::data::bytes;
use crate::data::signature::*;
use crate::data::reference::Reference;
use crate::rpc::service::Service;

#[derive(Serialize,Deserialize)]
pub enum Error {
}

pub type IdentityRef<Sign> = Reference<bytes::AsBytes<PublicKey>, Sign>;
pub type Nonce = [u8;32];

#[derive(Serialize,Deserialize)]
pub enum Message<Sign>
    where Sign: SignMethod
{
	AuthRequest(Nonce, IdentityRef),
	AuthResponse(Nonce, #[serde(with="bytes")] Signature),
    Message(Vec<u8>, #[serde(with="bytes")] Signature),
}


pub enum IdentityState {
    /// Unauthenticated
    Unauthenticated,
    /// Authentication requested, provided Nonce is 
    Requested,
    /// Authenticated
    Authenticated,
}


pub struct Identity<Sign>
    where Sign: SignMethod
{
    pub state: IdentityState,
    /// Signer instance
    pub signer: Sign::Verifier,
    /// A reference issued by identity owner, proving sign_key is allowed
    /// to sign as the owner.
    pub identity: Reference<bytes::AsBytes<PublicKey>,Sign>,
    pub nonce: [u8;32],
    // nonce_timeout, nonce_next_timeout
}


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

#[service]
impl<S,Sign> Auth<S,Sign>
    where S: Service, Sign: SignMethod
{
	pub fn request_auth(&mut self, nonce: Nonce, identity: IdentityRef)
		-> Result<(Nonce, IdentityRef, Signature)>
	{
	}
}

