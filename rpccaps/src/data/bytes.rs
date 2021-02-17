//! Provide serialize/deserialize methods for types containing bytes array.
//! This module is used for cryptographic serialization.
use std::{mem,fmt};
use std::marker::PhantomData;
use serde::{Serializer,Deserializer,de};

pub trait Bytes: Clone+Sized {
    fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self>;
    fn as_bytes(&self) -> &[u8];
}

/// Serialize provided value as bytes
pub fn serialize<S,T>(value: &T, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer, T: Bytes
{
    ser.serialize_bytes(value.as_bytes())
}

/// Deserialize provided value from bytes
pub fn deserialize<'de,D,T>(de: D) -> Result<T, D::Error>
    where D: Deserializer<'de>, T: Bytes
{
    struct BytesVisitor<T: Bytes>(::std::marker::PhantomData<T>);

    impl<'de,T: Bytes> de::Visitor<'de> for BytesVisitor<T> {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a bytes array containing at least {} bytes",
                   mem::size_of::<T>())
        }

        fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
            where E: de::Error
        {
            T::from_bytes(v).ok_or(de::Error::custom("invalid size"))
        }
    }

    de.deserialize_bytes(BytesVisitor::<T>(PhantomData))
}


/// Implement Bytes for Box<Bytes>
impl<T: Bytes> Bytes for Box<T> {
    fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
        T::from_bytes(b).and_then(|t| Some(Box::new(t)))
    }

    fn as_bytes(&self) -> &[u8] {
        self.as_ref().as_bytes()
    }
}

/// Implement Bytes for Option<Bytes>
impl<T: Bytes> Bytes for Option<T> {
    fn from_bytes<B: AsRef<[u8]>>(b: B) -> Option<Self> {
        T::from_bytes(b).and_then(|t| Some(Some(t)))
    }

    fn as_bytes(&self) -> &[u8] {
        match self {
            Some(t) => t.as_bytes(),
            None => <&[u8]>::default(),
        }
    }
}


