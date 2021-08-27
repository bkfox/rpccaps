use std::fmt;
use std::marker::PhantomData;

use bincode;
use serde::{Serialize,Deserialize};
use signature::{Signer,Verifier};

use super::bytes::{self as bytes};
use super::validate::Validate;
use super::capability::Capability;
use super::signature as sign;


#[derive(Debug)]
pub enum Error {
    Empty, Capability, Issuer, Subject, MaxShare,
    Serialize(bincode::Error),
    Signature(sign::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        f.write_str(&format!("{}", self))
    }
}



/// A Reference is the combination of an object reference (as id) and authorizations chain.
///
/// It implements various utilities to sign and validate it.
#[derive(Serialize,Deserialize,PartialEq,Clone)]
pub struct Reference<Id,Sign>
    where Id: Clone, Sign: sign::SignMethod
{
    id: Id,
    #[serde(with="bytes")]
    issuer: Sign::Verifier,
    max_share: u32,
    certs: Vec<Certificate<Sign>>,
    phantom: PhantomData<Sign>,
}


#[derive(Serialize,Deserialize,PartialEq,Clone)]
pub struct Certificate<Sign>
    where Sign: sign::SignMethod
{
    #[serde(bound="Sign: sign::SignMethod")]
    pub auth: Authorization<Sign>,
    #[serde(with="bytes")]
    pub signature: sign::Signature,
}


#[derive(Serialize,Deserialize,PartialEq,Clone)]
pub struct Authorization<Sign>
    where Sign: sign::SignMethod
{
    pub capability: Capability,
    #[serde(with="bytes")]
    pub subject: Sign::Verifier,
}


#[derive(Serialize,Deserialize,PartialEq,Clone)]
pub enum CertData<Id, Sign>
    where Sign: sign::SignMethod
{
    #[serde(bound(serialize="Sign: sign::SignMethod, Id: Serialize"))]
    Reference(Authorization<Sign>, Id, #[serde(with="bytes")] Sign::Verifier, u32),
    #[serde(bound(serialize="Sign: sign::SignMethod, Id: Serialize"))]
    Signature(Authorization<Sign>, #[serde(with="bytes")] sign::Signature),
}


impl<Id,Sign> Reference<Id,Sign>
    where Id: Clone+Serialize, Sign: sign::SignMethod
{
    /// Create a new reference, signing it with the provided keys.
    pub fn new(id: Id, issuer: &Sign::Signer, max_share: u32, auth: Authorization<Sign>)
        -> Result<Self,Error>
    {
        match Sign::verifier(&issuer) {
            Ok(verifier) => {
                let mut reference = Self {
                    id, issuer: verifier.clone(), max_share,
                    certs: Vec::with_capacity(1),
                    phantom: PhantomData
                };
                reference.sign(issuer, auth).and(Ok(reference))
            },
            _ => Err(Error::Issuer),
        }
    }

    /// Return id of the reference.
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Return issuer of the reference.
    pub fn issuer(&self) -> &Sign::Verifier {
        &self.issuer
    }

    /// Return authorizations of the reference.
    pub fn certs(&self) -> &Vec<Certificate<Sign>> {
        &self.certs
    }

    /// Return cert data for provided signer, authorization and last
    /// certificate. Return Error on data validation fails.
    fn cert_data(&self, issuer: &Sign::Verifier, auth: Authorization<Sign>,
                 last: Option<&Certificate<Sign>>)
        -> Result<CertData<Id,Sign>,Error>
    {
       match last {
            None => Ok(CertData::Reference(auth, self.id.clone(), self.issuer.clone(), self.max_share)),
            Some(last) => {
                // test: auth must be subset of last auth
                if !auth.capability.is_subset(&last.auth.capability) {
                    return Err(Error::Capability);
                }
                // test: issuer must be last certificate's subject
                if issuer != &last.auth.subject {
                    return Err(Error::Issuer);
                }
                Ok(CertData::Signature(auth, last.signature))
            }
        }
    }

    /// Add a new signature to the reference.
    pub fn sign(&mut self, issuer: &Sign::Signer, auth: Authorization<Sign>) -> Result<(), Error> {
        if self.certs.len() >= (self.max_share as usize)+1 {
            return Err(Error::MaxShare);
        }

        let cert_data = self.cert_data(&Sign::verifier(&issuer).unwrap(), auth.clone(),
                                       self.certs.last());
        match cert_data {
            Ok(cert_data) => bincode::serialize(&cert_data)
                .or_else(|err| Err(Error::Serialize(err)))
                .and_then(|buf| issuer.try_sign(&buf)
                                      .or_else(|err| Err(Error::Signature(err))))
                .and_then(|signature| {
                    self.certs.push(Certificate { auth, signature });
                    Ok(())
                }),
            Err(err) => Err(err),
        }
    }

    /// Create a new reference with authorizations' chain up to subject.
    pub fn subset(&self, subject: &Sign::Verifier) -> Option<Self> {
        self.certs.iter().enumerate().find(|(_i,c)| subject == &c.auth.subject)
            .and_then(|(i, _auth)| Some(Self {
                id: self.id.clone(),
                issuer: self.issuer.clone(),
                max_share: self.max_share,
                certs: self.certs[0..i+1].to_vec(),
                phantom: PhantomData,
            }))
    }

    /// Shorten the authorizations' chain for the provided subject, signing it in
    /// a new reference.
    pub fn shrink(&self, signer: &Sign::Signer, subject: &Sign::Verifier) -> Option<Self> {
        match self.certs.iter().find(|cert| subject == &cert.auth.subject) {
            Some(cert) => Sign::verifier(signer).ok()
                .and_then(|verifier| self.subset(verifier))
                .and_then(|mut reference| {
                    reference.sign(signer, cert.auth.clone()).ok().and(Some(reference))
                }),
            _ => None,
        }
    }
}

/// Validation is tested agains't last user's public-key
impl<Id,Sign> Validate for Reference<Id,Sign>
    where Id: Clone+Serialize, Sign: sign::SignMethod
{
    type Error = Error;
    type Context = Sign::Verifier;

    fn validate(&self, subject: &Self::Context) -> Result<(),Self::Error> {
        // Max share count
        if self.certs.len() > (self.max_share as usize)+1 {
            return Err(Error::MaxShare);
        }

        // Various checks around subject
        match self.certs.last() {
            // Subject must be last subject
            Some(cert) if subject != &cert.auth.subject => return Err(Error::Subject),
            // Certificate can not be empty
            None => return Err(Error::Empty),
            _ => ()
        };

        // Check certificates
        let mut buf = Vec::new();
        let mut issuer = &self.issuer;
        let mut last: Option<&Certificate<Sign>> = None;

        for cert in self.certs.iter() {
            match self.cert_data(issuer, cert.auth.clone(), last) {
                Ok(cert_data) => {
                    buf.clear();
                    if let Err(err) = bincode::serialize_into(&mut buf, &cert_data) {
                        return Err(Error::Serialize(err))
                    }

                    if let Err(err) = issuer.verify(&buf, &cert.signature) {
                        return Err(Error::Signature(err))
                    }

                    issuer = &cert.auth.subject;
                    last = Some(&cert);
                },
                Err(err) => return Err(err),
            };
        }
        Ok(())
    }
}



impl<Sign> Authorization<Sign>
    where Sign: sign::SignMethod
{
    pub fn new(capability: Capability, subject: Sign::Verifier) -> Self {
        Self { capability, subject }
    }
}


#[cfg(test)]
pub mod tests {
    use std::ops::{Deref,DerefMut};

    use crate::expect;
    use super::super::signature::{Dalek,SignMethod};
    use super::*;


    pub struct TestReference<Sign: SignMethod> {
        pub signers: Vec<Sign::Signer>,
        pub public_keys: Vec<Sign::Verifier>,
        pub reference: Reference<u64,Sign>,
    }

    impl<Sign: SignMethod> Deref for TestReference<Sign> {
        type Target = Reference<u64,Sign>;

        fn deref(&self) -> &Self::Target {
            &self.reference
        }
    }

    impl<Sign: SignMethod> DerefMut for TestReference<Sign> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.reference
        }
    }

    impl TestReference<Dalek> {
        pub fn new(max_share: u32, cap: Capability) -> Self {
            let signers = (0..10)
                .map(|_| Dalek::generate().unwrap())
                .collect::<Vec<_>>();
            let public_keys = signers.iter().map(|s| s.public)
                .collect::<Vec<_>>();

            let auth = Authorization::new(cap, public_keys[1].clone());
            let reference = Reference::<u64,Dalek>::new(0u64, &signers[0], max_share, auth)
                                .expect("can not create reference");

            Self { signers, public_keys, reference }
        }

        pub fn sign(&mut self, signer: usize, capability: Capability) -> Result<(),Error>
        {
            if signer+1 >= self.signers.len() {
                panic!("signer invalid")
            }

            let auth = Authorization::new(capability, self.public_keys[signer+1].clone());
            self.reference.sign(&self.signers[signer], auth)
        }

        pub fn sign_n(&mut self, last: Option<usize>, mut capability: Capability) -> Result<(), (usize,Error)> {
            let last = last.unwrap_or_else(|| self.signers.len()-1);
            for i in 1..last {
                capability.actions >>= 1;
                if let Err(err) = self.sign(i, capability.clone()) {
                    return Err((i, err));
                }
            }
            Ok(())
        }

        pub fn validate(&self, subject: Option<usize>) -> Result<(), Error> {
            let subject = subject.unwrap_or_else(|| self.public_keys.len()-1);
            self.reference.validate(&self.public_keys[subject])
        }
    }

    #[test]
    fn test_sign_ok() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Dalek>::new(64, cap.clone());

        expect!(test.sign_n(None, cap), Ok(_));
        expect!(test.validate(None), Ok(_));
    }

    #[test]
    fn test_sign_err() {
        let cap = Capability::new(0b11111111, 0b00000000);
        let mut test = TestReference::<Dalek>::new(64, cap.clone());

        expect!(test.sign(1, cap.clone()), Err(Error::Capability));
        expect!(test.sign(2, cap.clone()), Err(Error::Capability));
    }

    #[test]
    fn test_sign_max_share() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Dalek>::new(0, cap.clone());

        expect!(test.sign_n(None, cap), Err((_, Error::MaxShare)));
        // TODO expect!(test.validate(None), Ok(_));
    }

    #[test]
    fn test_validate_err_auth() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Dalek>::new(64, cap.clone());

        expect!(test.sign_n(None, cap), Ok(_));
        expect!(test.validate(None), Ok(_));

        let auth = test.certs.remove(5);
        expect!(test.validate(Some(test.certs.len())), Err(_));

        test.certs.push(auth);
        expect!(test.validate(None), Err(_));
    }

    #[test]
    fn test_validate_err_subject() {
        let cap = Capability::new(0b11111111, 0b00001111);
        let test = TestReference::<Dalek>::new(64, cap.clone());

        expect!(test.validate(Some(2)), Err(Error::Subject));
    }

    #[test]
    fn test_validate_err_cap() {
        let cap = Capability::new(0b11111111, 0b00001111);
        let mut test = TestReference::<Dalek>::new(64, cap.clone());

        test.sign(1, cap.subset(cap.actions >> 1, cap.share)).unwrap();
        test.reference.certs.get_mut(1).unwrap().auth.capability.actions = cap.actions;
        expect!(test.validate(Some(2)), Err(Error::Capability));
    }

    #[test]
    fn test_validate_err_sign() {
        let cap = Capability::new(0b11111111, 0b00001111);
        let mut test = TestReference::<Dalek>::new(64, cap.clone());

        test.sign(1, cap.subset(cap.actions >> 1, cap.share)).unwrap();

        // signature poisoning
        let sig = test.reference.certs.get(0).unwrap().signature.clone();
        test.reference.certs.get_mut(1).unwrap().signature = sig;

        expect!(test.validate(Some(2)), Err(Error::Signature(_)));
    }

    #[test]
    fn test_subset() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Dalek>::new(64, cap.clone());

        test.sign_n(None, cap).unwrap();

        let subject = test.public_keys[4];
        let subset = test.reference.subset(&subject).unwrap();
        if subject != subset.certs.last().unwrap().auth.subject {
            panic!("subject in reference and its subset are different")
        }

        expect!(subset.validate(&subject), Ok(_));
    }

    #[test]
    fn test_shrink() {
        let cap = Capability::new(0b11111111, 0b11111111);
        let mut test = TestReference::<Dalek>::new(64, cap.clone());

        test.sign_n(None, cap).unwrap();

        let (signer, subject) = (&test.signers[2], &test.public_keys[6]);
        let subset = test.reference.shrink(signer, subject).unwrap();
        let last = subset.certs.last().unwrap();

        if &last.auth.subject != subject {
            panic!("subject incorrect: \n{:?}\n{:?}", last.auth.subject, subject)
        }

        expect!(subset.validate(&subject), Ok(_));
    }
}

