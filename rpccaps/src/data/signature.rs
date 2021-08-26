use std::convert::TryFrom;

use signature::{Signer,Verifier};
use signatory::public_key::PublicKeyed;

use super::bytes;

pub use signatory::ed25519::{Seed as PrivateKey, PublicKey, Signature};
pub use signature::Error;


pub trait SignMethod {
    type Signer: Signer<Signature>;
    type Verifier: Verifier<Signature>;

    fn signer(key: &PrivateKey) -> Self::Signer;
    fn verifier(key: &PublicKey) -> Self::Verifier;
    fn public_key(signer: &Self::Signer) -> Option<PublicKey>;
}


impl bytes::Bytes for PrivateKey {
    fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
        PrivateKey::from_bytes(b)
    }

    fn as_bytes(&self) -> &[u8] {
        self.as_secret_slice()
    }
}

impl bytes::Bytes for PublicKey {
    fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
        PublicKey::from_bytes(b)
    }

    fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }
}

impl bytes::Bytes for Signature {
    fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
        <Self as TryFrom<&[u8]>>::try_from(b.as_ref()).ok()
    }

    fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }
}


mod sodium {
    use super::*;
    use signatory_sodiumoxide::{Ed25519Signer, Ed25519Verifier};

    pub struct Sodium;

    impl super::SignMethod for Sodium {
        type Signer = Ed25519Signer;
        type Verifier = Ed25519Verifier;

        fn signer(key: &PrivateKey) -> Self::Signer {
            Self::Signer::from(key)
        }

        fn verifier(key: &PublicKey) -> Self::Verifier {
            Self::Verifier::from(key)
        }

        fn public_key(signer: &Self::Signer) -> Option<PublicKey> {
            signer.public_key().ok()
        }
    }
}

pub use sodium::Sodium;


