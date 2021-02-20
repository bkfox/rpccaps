pub mod bytes;
pub mod capability;
pub mod reference;
pub mod signature;
pub mod validate;

pub use capability::Capability;
pub use reference::{Authorization,Reference};
pub use self::signature::SignMethod;


// TODO: read/write file as objects

