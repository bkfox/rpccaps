use std::{
    fs, io::ErrorKind as IoErrorKind,
    path::PathBuf,
};
use crate::{ErrorKind,Result};


/// Read private key from der or pem file.
pub fn private_key_from_file(key_path: &PathBuf)
    -> Result<rustls::PrivateKey>
{
    match fs::read(key_path) {
        Ok(key) => match key_path.extension() {
            Some(x) if x == "der" => Ok(rustls::PrivateKey(key)),
            _ => {
                let pkcs8 = rustls_pemfile::pkcs8_private_keys(&mut &*key)
                        .or(ErrorKind::InvalidData.err("invalid PKCS #8 key"))?;
                match pkcs8.into_iter().next() {
                    Some(x) => Ok(rustls::PrivateKey(x)),
                    None => {
                        let rsa = rustls_pemfile::rsa_private_keys(&mut &*key)
                            .or(ErrorKind::InvalidData.err("invalid PKCS #1 key"))?;
                        match rsa.into_iter().next() {
                            Some(x) => Ok(rustls::PrivateKey(x)),
                            None => ErrorKind::InvalidData.err("malformed PKCS #1 private key"),
                        }
                    }
                }
            }
        },
        Err(err) if err.kind() == IoErrorKind::NotFound
            => ErrorKind::NotFound.err("private key file not found"),
        Err(err) => ErrorKind::File.err(err.to_string()),
    }
}


/// Return certificates from der or pem file.
pub fn cert_from_file(cert_path: &PathBuf)
    -> Result<Vec<rustls::Certificate>>
{
    match fs::read(cert_path) {
        Ok(cert) => match cert_path.extension() {
            Some(x) if x == "der" => Ok(vec![rustls::Certificate(cert)]),
            _ => Ok(rustls_pemfile::certs(&mut &*cert)
                    .or(ErrorKind::Certificate.err("invalid certificate pem file"))?
                    .into_iter()
                    .map(rustls::Certificate)
                    .collect()),
        },
        Err(err) if err.kind() == IoErrorKind::NotFound
            => ErrorKind::NotFound.err("cert file not found"),
        Err(err) => return ErrorKind::File.err(err.to_string()),
    }
}


/// Generate a new certificate and private key
pub fn new_cert(subjects: Vec<String>)
    -> Result<(Vec<rustls::Certificate>, rustls::PrivateKey)>
{
    // generate new certificate
    let cert = rcgen::generate_simple_self_signed(subjects)
        .or(ErrorKind::Certificate.err("can not generate certificate"))?;
    let (cert, key) = match cert.serialize_der() {
        Ok(cert_) => (cert_, cert.serialize_private_key_der()),
        _ => return ErrorKind::Certificate.err("can not serialize generated certificate"),
    };
    Ok((vec![rustls::Certificate(cert)], rustls::PrivateKey(key)))
}

