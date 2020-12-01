use signature as sig;
use signatory::public_key;
pub use signature::{Signer,Verifier,Error};

use super::bytes;


pub trait PrivateKey: bytes::Bytes {}
pub trait PublicKey: bytes::Bytes+public_key::PublicKey {}
pub trait Signature: sig::Signature+bytes::Bytes {}


pub trait Method {
    type PrivateKey: PrivateKey;
    type PublicKey: PublicKey;
    type Signature: Signature;
    type Signer: Signer<Self::Signature>;
    type Verifier: Verifier<Self::Signature>;

    fn private_key() -> Self::PrivateKey;
    fn signer(key: &Self::PrivateKey) -> Self::Signer;
    fn public_key(signer: &Self::Signer) -> Option<Self::PublicKey>;
    fn verifier(key: &Self::PublicKey) -> Self::Verifier;
}


pub mod ed25519 {
    use super::bytes;
    use std::convert::TryFrom;

    pub use signatory::ed25519::{Seed as PrivateKey, PublicKey, Signature};

    impl super::PrivateKey for PrivateKey { }
    impl bytes::Bytes for PrivateKey {
        fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
            PrivateKey::from_bytes(b)
        }

        fn as_bytes(&self) -> &[u8] {
            self.as_secret_slice()
        }
    }

    impl super::PublicKey for PublicKey {}
    impl bytes::Bytes for PublicKey {
        fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
            PublicKey::from_bytes(b)
        }

        fn as_bytes(&self) -> &[u8] {
            self.as_ref()
        }
    }

    impl super::Signature for Signature {}
    impl bytes::Bytes for Signature {
        fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
            <Self as TryFrom<&[u8]>>::try_from(b.as_ref()).ok()
        }

        fn as_bytes(&self) -> &[u8] {
            self.as_ref()
        }
    }
}


mod sodium {
    pub use signatory_sodiumoxide::{Ed25519Signer, Ed25519Verifier};
    use signatory::{ed25519,public_key::PublicKeyed};

    pub struct Sodium;

    impl super::Method for Sodium {
        type PrivateKey = ed25519::Seed;
        type PublicKey = ed25519::PublicKey;
        type Signature = ed25519::Signature;
        type Signer = Ed25519Signer;
        type Verifier = Ed25519Verifier;

        fn private_key() -> Self::PrivateKey {
            Self::PrivateKey::generate()
        }

        fn signer(key: &Self::PrivateKey) -> Self::Signer {
            Self::Signer::from(key)
        }

        fn verifier(key: &Self::PublicKey) -> Self::Verifier {
            Self::Verifier::from(key)
        }

        fn public_key(signer: &Self::Signer) -> Option<Self::PublicKey> {
            signer.public_key().ok()
        }
    }
}

pub use sodium::Sodium;


