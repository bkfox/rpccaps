use std::{ error, fmt, fmt::Display };

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



#[derive(PartialEq,Debug,Clone)]
pub enum ErrorKind {
	Internal,
	KeyError,
	NotFound,
	Codec,
	LimitReached,
	InvalidData,
	InvalidInput,
	IO,
	File,
	Certificate,
	Other,
}

#[derive(PartialEq,Debug,Clone)]
pub struct Error {
	kind: ErrorKind,
	description: String,
}

pub type Result<T> = std::result::Result<T, Error>;


impl ErrorKind {
    fn error(self, description: impl Into<String>) -> Error {
    	Error::new(self, description)
    }

	fn err<T>(self, description: impl Into<String>) -> Result<T> {
    	Err(self.error(description))
	}
}

impl Error {
	pub fn new(kind: ErrorKind, description: impl Into<String>) -> Self {
		Self { kind, description: description.into() }
	}

	pub fn kind(&self) -> ErrorKind {
		self.kind
	}
}

impl Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Error({:?}): {}", &self.kind, &self.description)
	}
}

impl error::Error for Error {
	fn description(&self) -> &str {
		&self.description
	}
}


