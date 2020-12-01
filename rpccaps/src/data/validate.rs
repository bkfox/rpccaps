use std::fmt::Display;
use std::ops::{Deref,DerefMut};

use serde::{Serialize,Deserialize,Serializer,Deserializer};


/// Add data validation after deserialization for a struct.
pub trait Validate: Sized {
    type Error: Display;
    type Context;

    fn validate(&self, context: &Self::Context) -> Result<(),Self::Error>;
}


/// Wrapper around struct used to add validation at deserialization
pub struct Unsafe<T: Validate>(T);

impl<T: Validate> Unsafe<T> {
    pub fn validate(self, context: &T::Context) -> Result<T,T::Error> {
        self.0.validate(context).and_then(|_| Ok(self.0))
    }
}


impl<T: Validate> Deref for Unsafe<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Validate> DerefMut for Unsafe<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}


impl<T: Validate+Serialize> Serialize for Unsafe<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Validate+Deserialize<'de>> Deserialize<'de> for Unsafe<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>,
    {
        <T as Deserialize>::deserialize(deserializer).and_then(|d| Ok(Self(d)))
    }
}



