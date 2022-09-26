use std::{
    convert::TryInto,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize,Serialize};
use crate::{
    ErrorKind, Result,
    data::tls,
};


/// Connection configuration
pub struct ConnectionConfig {
    /// Endpoint's certificate data
    pub cert_data: Option<(Vec<rustls::Certificate>, rustls::PrivateKey)>,
    /// Endpoint's certificate and private key's file path
    pub cert_path: Option<(PathBuf, PathBuf)>,
    /// Endpoint's certificate subjects' names
    pub cert_subjects: Vec<String>,
    /// If true, create cert when missing
    pub create_cert: bool,
    /// Maximum concurrent bidirectional streams per peer
    pub concurrent_streams: u32,
    /// Maximum connection idle timeout
    pub idle_timeout: Duration,
    /// Wether client must authenticate
    pub with_no_client_auth: bool,
}


/// Server configuration
pub struct ServerConfig {
    /// Connection configuration
    pub connection_config: ConnectionConfig,
    /// Maximum concurrent connections
    pub concurrent_connections: u32,
    /// Allow client onnection migration
    pub migration: bool,
    /// Enable stateless retries
    pub stateless_retry: bool,
}


/// Client configuration
pub struct ClientConfig {
    /// Connection configuration
    pub connection_config: ConnectionConfig,
    /// Use system's trusted root certificates
    pub system_certs: bool,
    /// Provide certificate authorities from provided files
    pub root_certs: Vec<PathBuf>,
}



impl ConnectionConfig {
    /// Initialize ``quinn::Transport`` based on self's parameters.
    pub fn set_transport_config(&self, transport: &mut quinn::TransportConfig) {
        transport.max_concurrent_uni_streams(0_u8.into())
                 .max_concurrent_bidi_streams(self.concurrent_streams.into())
                 .max_idle_timeout(Some(self.idle_timeout.try_into().unwrap()));
    }

    /// Get certificate and private key based on self's parameters.
    pub fn get_cert(&self, create_cert: bool)
        -> Result<Option<(Vec<rustls::Certificate>, rustls::PrivateKey)>>
    {
        match self.cert_data {
            Some((ref cert, ref key)) => Ok(Some((cert.clone(), key.clone()))),
            None => match self.cert_path {
                Some((ref cert_path, ref key_path)) => {
                    let cert = tls::cert_from_file(cert_path)?;
                    let key = tls::private_key_from_file(key_path)?;

                    // TODO: write cert
                    Ok(Some((cert, key)))
                },
                None if create_cert => tls::new_cert(self.cert_subjects.clone())
                                            .and_then(|v| Ok(Some(v))),
                None => Ok(None),
            }
        }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            cert_data: None,
            cert_path: None,
            cert_subjects: vec![String::from("localhost")],
            create_cert: true,
            concurrent_streams: 32,
            idle_timeout: Duration::from_secs(10),
            with_no_client_auth: true,
        }
    }
}


impl ServerConfig {
    /// Return quinn server configuration.
    pub fn get_server_config(&self) -> Result<quinn::ServerConfig>
    {
        let crypto = self.get_tls_config()?;
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        server_config.concurrent_connections(self.concurrent_connections)
                     .use_retry(self.stateless_retry)
                     .migration(self.migration);
        let ref mut transport = Arc::get_mut(&mut server_config.transport).unwrap();
        self.connection_config.set_transport_config(transport);
        Ok(server_config)
    }

    /// Initialize ``rustls::ConfigBuilder`` based on self's parameters.
    pub fn get_tls_config(&self) -> Result<rustls::ServerConfig>
    {
        let certs_key = match self.connection_config.get_cert(self.connection_config.create_cert)? {
            Some(certs_key) => certs_key,
            None => return ErrorKind::ValueError.err("no certificate specified"),
        };
        let builder = rustls::ServerConfig::builder().with_safe_defaults();
        /*match self.connection_config.with_no_client_auth {
            true => */  /*,
            false => Ok(builder.with_single_cert(certs_key.0, certs_key.1)),
        }*/
        builder.with_no_client_auth()
               .with_single_cert(certs_key.0, certs_key.1)
               .or(ErrorKind::Certificate.err("invalid certificate at init client config"))
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            connection_config: ConnectionConfig::default(),
            concurrent_connections: 32,
            stateless_retry: false,
            migration: false,
        }
    }
}


impl ClientConfig {
    /// Return quinn client configuration.
    pub fn get_client_config(&self) -> Result<quinn::ClientConfig>
    {
        let crypto = self.get_tls_config()?;
        let mut client_config = quinn::ClientConfig::new(Arc::new(crypto));
        let ref mut transport = Arc::get_mut(&mut client_config.transport).unwrap();
        self.connection_config.set_transport_config(transport);
        Ok(client_config)
    }

    /// Initialize ``rustls::ConfigBuilder`` based on self's parameters.
    pub fn get_tls_config(&self) -> Result<rustls::ClientConfig>
    {
        let certs_key = self.connection_config.get_cert(self.connection_config.create_cert)?;
        let mut roots = rustls::RootCertStore::empty();

        for cert_path in self.root_certs.iter() {
            for ref mut cert in tls::cert_from_file(cert_path)? {
                roots.add(cert)
                     .or(ErrorKind::Certificate.err("invalid authority certificate"));
            }
        }

        let builder = rustls::ClientConfig::builder()
                                .with_safe_defaults()
                                .with_root_certificates(roots);
        // TODO: errors handling
        match (self.connection_config.with_no_client_auth, certs_key) {
            (true, Some((certs, key))) => Ok(builder.with_single_cert(certs, key).unwrap()),
            (true, None) => ErrorKind::ValueError.err(
                "missing certificate while specifying `with_no_client_auth`"),
            (false, _) => Ok(builder.with_no_client_auth()),
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            connection_config: ConnectionConfig::default(),
            system_certs: false,
            root_certs: Vec::new(),
        }
    }
}


#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_default_server_config() {
        let mut config = ServerConfig::default();
        // TODO: save cert, load cert

        let quinn_config = config.get_server_config().unwrap();
    }

    #[test]
    fn test_default_client_config() {
        let config = ClientConfig::default();
        let quinn_config = config.get_client_config().unwrap();
    }
}


