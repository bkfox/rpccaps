
pub mod codec;
pub mod dispatch;
pub mod server;
pub mod service;
pub mod transport;


// #[cfg(test)]
// pub mod tests;


pub use codec::BincodeCodec;
pub use service::Service;
pub use transport::Transport;


#[derive(PartialEq,Debug)]
pub enum Error {
    Internal,
    KeyError,
    TooManyTasks,
    Codec,
    File,
    Io,
}


