use std::{mem,fmt};

use bincode;
use serde::{Serialize,Deserialize};
use signature::{Signer,Verifier};

use super::bytes;
use super::validate::Validate;
use super::capability::Capability;
use super::signature as sign;


#[derive(Debug)]
pub enum Error {
    Data, Issuer, Subject,
    Signature(usize, sign::Error),
    Capability(usize),
    MissingSignature(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        f.write_str(&format!("{}", self))
    }
}



/// A reference to an object with capabilities.
#[derive(PartialEq)]
pub struct Reference<Id,M>
    where Id: Clone+Serialize, M: sign::Method
{
    header: Header<Id,M::PublicKey>,
    auths: Vec<Authorization<M::PublicKey,M::Signature>>,
}

/// Reference header
#[derive(Serialize,Deserialize,PartialEq,Clone)]
pub struct Header<Id,Pub>
    where Id: Clone+Serialize, Pub: sign::PublicKey
{
    pub id: Id,
    #[serde(with="bytes")]
    pub issuer: Pub,
}

#[derive(Serialize,Deserialize,PartialEq,Clone)]
pub struct Authorization<Pub,Sig>
    where Pub: sign::PublicKey, Sig: sign::Signature
{
    pub capability: Capability,
    #[serde(with="bytes")]
    pub subject: Pub,
    #[serde(with="bytes")]
    pub signature: Option<Sig>,
}


impl<Id,M> Reference<Id,M>
    where Id: Clone+Serialize, M: sign::Method
{
    /// Create a new object reference, signing it with the provided keys.
    pub fn new(signer: &M::Signer, id: Id, auth: Authorization<M::PublicKey,M::Signature>) -> Result<Self,Error>
    {
        match M::public_key(signer) {
            Some(issuer) => {
                let mut reference = Self { header: Header { id, issuer },
                                           auths: Vec::with_capacity(1) };
                reference.sign(signer, auth).and(Ok(reference))
            },
            _ => Err(Error::Issuer),
        }
    }

    /// Add a new signature to the reference.
    pub fn sign(&mut self, signer: &M::Signer, mut auth: Authorization<M::PublicKey,M::Signature>) -> Result<(), Error> {
        match self.auths.last() {
            Some(last) if M::public_key(&signer).unwrap() != last.subject
                => return Err(Error::Issuer),
            Some(last) if !auth.capability.is_subset(&last.capability)
                => return Err(Error::Capability(self.auths.len())),
            // None => return Err(Error::Data),
            _ => (),
        };

        let mut buf = self.sign_buf();
        for auth in self.auths.iter() {
            bincode::serialize_into(&mut buf, &auth);
        }

        auth.serialize_data(&mut buf);
        signer.try_sign(&buf).and_then(|signature| {
            auth.signature = Some(signature);
            self.auths.push(auth);
            Ok(())
        }).or_else(|err| {
            Err(Error::Signature(self.auths.len(), err))
        })
    }

    // New buffer for signing, already including reference's header.
    fn sign_buf(&self) -> Vec<u8> {
        let size = mem::size_of_val(&self.header) +
                   mem::size_of::<Authorization<M::PublicKey,M::Signature>>()
                     * (self.auths.len()+1);
        let mut buf = Vec::with_capacity(size);
        bincode::serialize_into(&mut buf, &self.header);
        buf
    }

    /// Return reference header.
    pub fn header(&self) -> &Header<Id,M::PublicKey> {
        &self.header
    }

    /// Return signed authorizations of the reference.
    pub fn auths(&self) -> &Vec<Authorization<M::PublicKey,M::Signature>> {
        &self.auths
    }

    /// Create a subset from this reference, including authorizations until provided
    /// subject.
    pub fn subset(&self, subject: &M::PublicKey) -> Option<Self> {
        self.auths.iter().enumerate().find(|(_i,a)| subject == &a.subject)
            .and_then(|(i, _auth)| Some(Self {
                header: self.header.clone(),
                auths: self.auths[0..i+1].to_vec(),
            }))
    }

    /// Shorten the authorization chains for the provided subject, signing it in
    /// a new reference.
    pub fn shrink(&self, signer: &M::Signer, subject: &M::PublicKey) -> Option<Self> {
        match self.auths.iter().find(|a| subject == &a.subject) {
            Some(auth) => M::public_key(signer)
                .and_then(|k| self.subset(&k))
                .and_then(|mut reference| {
                    reference.sign(signer, auth.clone()).ok().and(Some(reference))
                }),
            _ => None,
        }
    }
}


impl<Id,M> Validate for Reference<Id,M>
    where Id: Clone+Serialize, M: sign::Method
{
    type Error = Error;
    type Context = M::PublicKey;

    fn validate(&self, subject: &Self::Context) -> Result<(),Self::Error> {
        // test agains't provided subject
        match self.auths.last() {
            Some(auth) if subject != &auth.subject => return Err(Error::Subject),
            None if subject != &self.header.issuer => return Err(Error::Issuer),
            _ => ()
        };

        // tests authorizations
        let (mut issuer, mut cap): (_, Option<&Capability>) = (&self.header.issuer, None);
        let mut buf = self.sign_buf();

        for (index, auth) in self.auths.iter().enumerate() {
            // test capability
            if let Some(cap) = cap {
                if !auth.capability.is_subset(cap) {
                    return Err(Error::Capability(index))
                }
            }

            // test signature
            if auth.signature.is_none() {
                return Err(Error::MissingSignature(index));
            }

            auth.serialize_data(&mut buf);

            let verifier = M::verifier(issuer);
            if let Err(err) = verifier.verify(&buf, &auth.signature.as_ref().unwrap()) {
                return Err(Error::Signature(index, err))
            }

            auth.serialize_sig(&mut buf);

            cap = Some(&auth.capability);
            issuer = &auth.subject;
        }
        Ok(())
    }
}


impl<Pub,Sig> Authorization<Pub,Sig>
    where Pub: sign::PublicKey, Sig: sign::Signature
{
    pub fn new(capability: Capability, subject: Pub) -> Self {
        Self { capability, subject, signature: None }
    }

    fn serialize_data(&self, mut buf: &mut Vec<u8>) {
        bincode::serialize_into(&mut buf, &self.capability);
        bincode::serialize_into(&mut buf, self.subject.as_bytes());
    }

    fn serialize_sig(&self, buf: &mut Vec<u8>) {
        let sig = self.signature.as_ref().unwrap();
        bincode::serialize_into(buf, sig.as_ref());
    }
}


#[cfg(test)]
mod tests {
    use std::ops::{Deref,DerefMut};
    use crate::expect;
    use super::super::signature::{Sodium,Method};
    use super::*;

    struct TestReference<M: Method> {
        signers: Vec<M::Signer>,
        public_keys: Vec<M::PublicKey>,
        reference: Reference<u64,M>,
    }

    impl<M: Method> Deref for TestReference<M> {
        type Target = Reference<u64,M>;

        fn deref(&self) -> &Self::Target {
            &self.reference
        }
    }

    impl<M: Method> DerefMut for TestReference<M> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.reference
        }
    }

    impl<M: Method> TestReference<M> {
        fn new(cap: Capability) -> Self {
            let signers = (0..10).map(|_| M::signer(&M::private_key())).collect::<Vec<_>>();
            let public_keys = signers.iter()
                .map(|s| M::public_key(s).expect("error getting public key from signer"))
                .collect::<Vec<_>>();

            let auth = Authorization::new(cap, public_keys[1].clone());
            let reference = Reference::<u64,M>::new(&signers[0], 0u64, auth)
                                .expect("can not create reference");

            Self { signers, public_keys, reference }
        }

        fn sign(&mut self, signer: usize, capability: Capability) -> Result<(),Error>
        {
            if signer+1 >= self.signers.len() {
                panic!("signer invalid")
            }

            let auth = Authorization::new(capability, self.public_keys[signer+1].clone());
            self.reference.sign(&self.signers[signer], auth)
        }

        fn sign_n(&mut self, last: Option<usize>, mut capability: Capability) -> Result<(), (usize,Error)> {
            let last = last.unwrap_or_else(|| self.signers.len()-1);
            for i in 1..last {
                capability.ops >>= 1;
                if let Err(err) = self.sign(i, capability.clone()) {
                    return Err((i, err));
                }
            }
            Ok(())
        }

        fn validate(&self, subject: Option<usize>) -> Result<(), Error> {
            let subject = subject.unwrap_or_else(|| self.public_keys.len()-1);
            self.reference.validate(&self.public_keys[subject])
        }
    }

    #[test]
    fn test_sign_ok() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Sodium>::new(cap.clone());

        expect!(test.sign_n(None, cap), Ok(_));
        expect!(test.validate(None), Ok(_));
    }

    #[test]
    fn test_sign_err() {
        let cap = Capability::new(0b11111111, 0b00000000);
        let mut test = TestReference::<Sodium>::new(cap.clone());

        expect!(test.sign(1, cap.clone()), Err(Error::Capability(_)));
        expect!(test.sign(2, cap.clone()), Err(Error::Issuer));
    }

    #[test]
    fn test_validate_err_auth() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Sodium>::new(cap.clone());

        expect!(test.sign_n(None, cap), Ok(_));
        expect!(test.validate(None), Ok(_));

        let auth = test.auths.remove(5);
        expect!(test.validate(Some(test.auths.len())), Err(_));

        test.auths.push(auth);
        expect!(test.validate(None), Err(_));
    }

    #[test]
    fn test_validate_err_subject() {
        let cap = Capability::new(0b11111111, 0b00001111);
        let test = TestReference::<Sodium>::new(cap.clone());

        expect!(test.validate(Some(2)), Err(Error::Subject));
    }

    #[test]
    fn test_validate_err_cap() {
        let cap = Capability::new(0b11111111, 0b00001111);
        let mut test = TestReference::<Sodium>::new(cap.clone());

        test.sign(1, cap.subset(cap.ops >> 1, cap.share)).unwrap();
        test.reference.auths.get_mut(1).unwrap().capability.ops = cap.ops;
        expect!(test.validate(Some(2)), Err(Error::Capability(_)));
    }

    #[test]
    fn test_validate_err_sign() {
        let cap = Capability::new(0b11111111, 0b00001111);
        let mut test = TestReference::<Sodium>::new(cap.clone());

        test.sign(1, cap.subset(cap.ops >> 1, cap.share)).unwrap();

        // signature poisoning
        let sig = test.reference.auths.get(0).unwrap().signature.unwrap().clone();
        test.reference.auths.get_mut(1).unwrap().signature = Some(sig);

        expect!(test.validate(Some(2)), Err(Error::Signature(_,_)));
    }

    #[test]
    fn test_subset() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Sodium>::new(cap.clone());

        test.sign_n(None, cap).unwrap();

        let subject = test.public_keys[4];
        let subset = test.reference.subset(&subject).unwrap();
        if subject != subset.auths.last().unwrap().subject {
            panic!("subject in reference and its subset are different")
        }

        expect!(subset.validate(&subject), Ok(_));
    }

    #[test]
    fn test_shrink() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Sodium>::new(cap.clone());

        test.sign_n(None, cap).unwrap();

        let (signer, subject) = (&test.signers[2], &test.public_keys[6]);
        let subset = test.reference.shrink(signer, subject).unwrap();
        let last = subset.auths.last().unwrap();

        if &last.subject != subject {
            panic!("subject incorrect: \n{:?}\n{:?}", last.subject, subject)
        }

        expect!(subset.validate(&subject), Ok(_));
    }

}

