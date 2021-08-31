use std::{
    fs::File,
    io::Read,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    sync::{Arc,RwLock},
    pin::Pin
};


use futures::prelude::*;
use tokio::{
    io::{AsyncRead,AsyncWrite},
    runtime::Runtime
};
use serde::{Deserialize,Serialize};

use rcgen::{self, generate_simple_self_signed};
use quinn::{self, Endpoint, ServerConfigBuilder, ClientConfigBuilder};

use super::codec::BincodeCodec;
use super::dispatch::{Dispatch,Error,HandlerFn};



#[derive(Serialize,Deserialize)]
struct ServerConfig {
    dispatch_max: Option<u32>,
    cert: Option<rcgen::Certificate>,
    cert_file: String,
    cert_subject_names: Vec<String>,
}



pub type IncomingStreamItem = (quinn::SendStream, quinn::RecvStream);

/// Server class handling dispatching to services from incoming transport stream.
/// It uses Bincode for messages de-serialization and Quic for communication.
struct Server<Id>
    where Id: std::cmp::Ord,
{
    // FIXME: RecvStream/SendStream
    pub dispatch: Arc<Dispatch<Id,IncomingStreamItem>>,
    pub connection: Option<quinn::Connection>,
    config: ServerConfig,
}



impl ServerConfig {
    /*fn load_cert(&mut self, cert_file: &str) -> Result<Vec<u8>,()> {
        let mut cert_data = Vec::new();
        let mut file = File::open(cert_file).or(Err(()))?;
        file.read_to_end(&mut cert_data).or(Err(()))?;
        Ok(cert_data)
    }*/

    pub fn load(&mut self) -> Result<(), ()> {
        if self.cert.is_none() {
            let cert = generate_simple_self_signed(self.cert_subject_names.clone())
                            .or(Err(()))?;
            self.cert = cert.ok();
        }

        Ok(())
    }
}


impl<Id> Server<Id>
    where for<'de> Id: 'static+std::cmp::Ord+Send+Sync+Clone+Deserialize<'de>,
{
    /// Create new server.
    pub fn new(config: ServerConfig) -> Self {
        Self {
            dispatch: Arc::new(Dispatch::new(config.dispatch_max)),
            connection: None,
            config: config,
        }
    }

    /// Create endpoint binding to provided address.
    pub fn endpoint(&mut self, address: &SocketAddr)
            -> Result<(quinn::Endpoint, quinn::Incoming), ()>
    {
        self.config.load()?;

        let mut endpoint = Endpoint::builder();
        let mut server_config = ServerConfigBuilder::default();
        let cert = self.config.cert.unwrap();
        let key = quinn::PrivateKey::from_der(&cert.serialize_private_key_der()).unwrap();
        let cert = quinn::Certificate::from_der(&cert.serialize_der().unwrap()).unwrap();
        let cert_chain = quinn::CertificateChain::from_certs(vec![cert.clone()]);

        server_config.certificate(cert_chain, key).unwrap();
        endpoint.listen(server_config.build());

        let mut client_config = ClientConfigBuilder::default();
        client_config.add_certificate_authority(cert).unwrap();
        endpoint.default_client_config(client_config.build());

        endpoint.bind(address).or(Err(()))
    }

    async fn run(&mut self, runtime: &Arc<Runtime>, address: &SocketAddr) -> Result<(),()> {
        let (mut endpoint, mut incoming) = self.endpoint(address)?;

        while let Some(connecting) = incoming.next().await {
            let new_conn = connecting.await.unwrap();
            let (dispatch, runtime) = (self.dispatch.clone(), runtime.clone());
            let connection = new_conn.connection;

            let handle = new_conn.bi_streams
                .take_while(|x| future::ready(x.is_ok()))
                .for_each(async move |stream| {
                    runtime.spawn(async {
                        let stream = stream.unwrap();
                        let data = (stream.0, stream.1, connection.clone());
                        dispatch.dispatch_stream::<BincodeCodec<Id>>(data).await
                                .or(Err(()))
                    });
                });
            runtime.spawn(handle);
        }
        Ok(())
    }

    // TODO: run(), stop()
}


/*#[cfg(feature="network")]
pub mod quic {
    use quinn::{self, Certificate, Connection, ConnectionError, ReadError, WriteError};
    use rand::{self, RngCore};
    use tokio::runtime::Builder;

    pub struct QuicServer<Id> {
        pub connection: Connection,
        cert_secret: Vec<u8>,
    }

    impl<Id> QuicServer<Id> {
        fn configure_listener(&self) -> (quinn::ServerConfig, Vec<u8>) {
            
        }
    }
}*/


#[cfg(test)]
pub mod tests {


    #[test]
    fn test_server() {
    }

}


