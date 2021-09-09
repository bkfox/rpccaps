use std::{
    fs,  io::ErrorKind as IoErrorKind,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
};


use futures::prelude::*;
use tokio::runtime::Runtime;
use serde::{Deserialize,Serialize};

use rcgen::{self, generate_simple_self_signed};
use quinn::{self, Endpoint, ServerConfigBuilder, ClientConfigBuilder};

use super::{ErrorKind, Result};
use super::codec::BincodeCodec;
use super::dispatch::Dispatch;


pub struct ServerConfig {
    /// Max concurrent dispatch
    pub dispatch_max: Option<u32>,
    /// Certificate and private key's file path
    pub cert: Option<(PathBuf, PathBuf)>,
    /// Certificate's subjects' names
    pub cert_subjects: Vec<String>,
}


pub struct ServerContext
{
    pub connection: quinn::Connection,
    pub endpoint: quinn::Endpoint,
}



pub type IncomingStreamItem = (quinn::SendStream, quinn::RecvStream, Arc<ServerContext>);

/// Server class handling dispatching to services from incoming transport stream.
/// It uses Bincode for messages de-serialization and Quic for communication.
struct Server<Id>
    where Id: std::cmp::Ord,
{
    pub dispatch: Arc<Dispatch<Id,IncomingStreamItem>>,
    pub endpoint: Option<quinn::Endpoint>,
    config: ServerConfig,
}



impl ServerConfig {
    pub fn new() -> Self {
		Self {
    		dispatch_max: None,
    		cert: None,
			cert_subjects: vec![String::from("localhost")],
		}
    }

	/// Read from files (see `self.cert`) or generate certificate and private key and return it.
	/// If `self.cert` is provided, save generated certificate and private key.
    pub fn get_cert(&self) -> Result<(quinn::CertificateChain, quinn::PrivateKey)> {
        if let Some((ref cert_path, ref key_path)) = self.cert {
            match self.load_cert(cert_path, key_path) {
				Err(err) if err.kind() == ErrorKind::NotFound => {},
				x => return x
            }
        }
        self.generate_cert()
    }

    fn load_cert(&self, cert_path: &PathBuf, key_path: &PathBuf)
        -> Result<(quinn::CertificateChain, quinn::PrivateKey)>
    {
        match (fs::read(cert_path), fs::read(key_path)) {
			(Ok(cert), Ok(key)) => {
				let cert_err = ErrorKind::InvalidData.err("invalid cert data");
				let cert_chain_err = ErrorKind::InvalidData.err("invalid cert chain data");
				let key_err = ErrorKind::InvalidData.err("invalid key data");
				let key = match key_path.extension() {
					Some(x) if x == "der" => quinn::PrivateKey::from_der(&key).or(key_err)?,
					_ => quinn::PrivateKey::from_pem(&key).or(key_err)?,
				};
				let cert = match cert_path.extension() {
					Some(x) if x == "der" => quinn::CertificateChain::from_certs(
    					Some(quinn::Certificate::from_der(&cert).or(cert_err)?),
					),
					_ => quinn::CertificateChain::from_pem(&cert).or(cert_chain_err)?,
				};
				Ok((cert, key))
			},
			(Err(err), _) if err.kind() == IoErrorKind::NotFound =>
    			ErrorKind::NotFound.err("cert file not found"),
			(_, Err(err)) if err.kind() == IoErrorKind::NotFound =>
    			ErrorKind::NotFound.err("private key file not found"),
			(Err(err), _) => return ErrorKind::File.err(err.to_string()),
			(_, Err(err)) => return ErrorKind::File.err(err.to_string()),
        }
    }

    fn generate_cert(&self) -> Result<(quinn::CertificateChain, quinn::PrivateKey)>
    {
		// generate new certificate
		let cert = generate_simple_self_signed(self.cert_subjects.clone())
			.or_else(|_| ErrorKind::Certificate.err("can not generate certificate"))?;
        let (cert, key) = match cert.serialize_der() {
        	Ok(cert_) => (cert_, cert.serialize_private_key_der()),
        	_ => return ErrorKind::Certificate.err("can not serialize generated certificate"),
        };
        if let Some((ref cert_path, ref key_path)) = self.cert {
			// TODO: write cert
        }

		let cert = quinn::CertificateChain::from_certs(vec![
    		quinn::Certificate::from_der(&cert).unwrap()
        ]);
		let key = quinn::PrivateKey::from_der(&key).unwrap();
		Ok((cert, key))
    }}


impl<Id> Server<Id>
    where for<'de> Id: 'static+std::cmp::Ord+Send+Sync+Deserialize<'de>+Unpin
{
    /// Create new server.
    pub fn new(config: ServerConfig) -> Self {
        Self {
            dispatch: Arc::new(Dispatch::new(config.dispatch_max)),
            endpoint: None,
            config: config,
        }
    }

    /// Create endpoint binding to provided address.
    pub fn endpoint(&mut self, address: &SocketAddr)
            -> Result<(quinn::Endpoint, quinn::Incoming)>
    {
        let mut endpoint = Endpoint::builder();
        let mut server_config = ServerConfigBuilder::default();
		let (cert, key) = self.config.get_cert()?;
        server_config.certificate(cert, key).unwrap();
        endpoint.listen(server_config.build());

        let mut client_config = ClientConfigBuilder::default();
        client_config.add_certificate_authority(cert).unwrap();
        endpoint.default_client_config(client_config.build());

        endpoint.bind(address)
            	.or_else(|err| ErrorKind::IO.err(err.to_string()))
    }

    async fn run(&mut self, runtime: Arc<Runtime>, address: &SocketAddr) -> Result<()> {
        let (endpoint, mut incoming) = self.endpoint(address)?;
		self.endpoint = Some(endpoint.clone());
        
        while let Some(conn) = incoming.next().await {
            let quinn::NewConnection { connection, mut bi_streams, .. } = conn.await.unwrap();
            let (dispatch, runtime_) = (self.dispatch.clone(), runtime.clone());
            let context = Arc::new(ServerContext{
                connection: connection,
                endpoint: endpoint.clone()
            });

            runtime.spawn(async move {
                while let Some(stream) = bi_streams.next().await {
                    let (dispatch_, context) = (dispatch.clone(), context.clone()) ;
                    runtime_.spawn(async move {
                        let stream = stream.unwrap();
                        let data = (stream.0, stream.1, context);
                        dispatch_.dispatch_stream::<BincodeCodec<Id>>(data).await
                    });
                }
            });
        }
        Ok(())
    }

    async fn close(&mut self, error_code: u32, reason: &[u8]) {
		if let Some(ref endpoint) = self.endpoint {
    		endpoint.close(error_code.into(), reason);
    		self.endpoint = None;
		}
    }
}


#[cfg(test)]
pub mod tests {
	use super::*;
	use std::{
		net::{Ipv4Addr}
	};

    #[test]
    fn test_server() {
        let server = Server::new(ServerConfig::new());
		
    }

}


