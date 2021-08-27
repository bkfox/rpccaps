use std::convert::TryFrom;

use signature;
use serde::{Serialize,Deserialize};

use super::bytes;

pub use signature::Error;
pub use ed25519::Signature;


pub trait Verifier : signature::Verifier<Signature>+PartialEq+Clone+bytes::Bytes {

}
pub trait Signer : signature::Signer<Signature> {}


pub trait SignMethod : Clone {
    type Signer: Signer;
    type Verifier: Verifier;

    fn generate() -> Result<Self::Signer,Error>;
    fn signer(secret: &[u8]) -> Result<Self::Signer, Error>;
    fn verifier(signer: &Self::Signer) -> Result<&Self::Verifier, Error>;
}



impl bytes::Bytes for Signature {
    fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
        <Self as TryFrom<&[u8]>>::try_from(b.as_ref()).ok()
    }

    fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }
}


pub mod dalek {
    pub use ed25519_dalek::{Keypair,PublicKey};
    use rand_core::{OsRng};
    use super::*;

    #[derive(Serialize,Deserialize,Clone)]
    pub struct Dalek;

    impl super::Signer for Keypair {}
    impl super::Verifier for PublicKey {}

    impl super::SignMethod for Dalek {
        type Signer = Keypair;
        type Verifier = PublicKey;

        fn generate() -> Result<Self::Signer, Error> {
            Ok(Keypair::generate(&mut OsRng))
        }

        fn signer(secret: &[u8]) -> Result<Self::Signer, Error> {
            Keypair::from_bytes(secret)
        }

        fn verifier(signer: &Self::Signer) -> Result<&Self::Verifier, Error> {
            Ok(&signer.public)
        }
    }

    impl bytes::Bytes for PublicKey {
        fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
            PublicKey::from_bytes(b.as_ref()).ok()
        }

        fn as_bytes(&self) -> &[u8] {
            (self as &PublicKey).as_bytes()
        }
    }
}

pub use dalek::Dalek;


