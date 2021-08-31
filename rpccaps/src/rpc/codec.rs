use std::marker::PhantomData;
use std::pin::Pin;

use bytes::BytesMut;
use futures::io::{AsyncRead,AsyncWrite};
use futures::prelude::*;
use futures::task::{Context,Poll};

use bincode;
use serde::{Deserialize,Serialize};
pub use tokio_util::codec::{Decoder,Encoder,FramedWrite};



/// FramedRead compatible with futures::io's AsyncRead/Write
pub struct FramedRead<R,C>
{
    reader: R,
    decoder: C,
    buffer: BytesMut,
}

impl<R,C> FramedRead<R,C>
{
    pub fn new(reader: R, decoder: C) -> Self {
        Self::with_capacity(reader, decoder, 128)
    }

    pub fn with_capacity(&self, reader: R, decoder: C, capacity: usize) -> Self {
        let mut buffer = BytesMut::with_capacity(capacity);
        Self { reader, decoder, buffer: BytesMut::new() }
    }

    pub fn capacity(&self) -> usize {
        self.buffer.capacity
    }

    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R,C> Stream for FramedRead<R,C>
    where R: AsyncRead,
          C: Decoder+Unpin,
{
    type Item = C::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>>
    {
        let mut this = self.as_mut();
        this.buffer.resize(this.buffer.capacity);

        match Pin::new(&mut this.reader).poll_read(cx, &this.buffer) {
            Poll::Ready(Ok(size)) => match this.codec.decode(&this.buffer[..size]) {
                Ok(Some(item)) => Poll::Ready(Some(item)),
                Ok(None) => Poll::Pending,
                Err(_) => Poll::Ready(None),
            },
            Poll::Ready(Err(_)) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}




/// Implement tokio codec for Bincode.
pub struct BincodeCodec<T>(PhantomData<T>);

impl<T> BincodeCodec<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> BincodeCodec<T>
    where for <'de> T: Deserialize<'de>
{
    pub fn framed_read<R: AsyncRead>(inner: R) -> FramedRead<R,Self> {
        FramedRead::new(inner, Self::new())
    }
}

impl<T> BincodeCodec<T>
    where T: Serialize
{
    pub fn framed_write<R: AsyncWrite>(inner: R) -> FramedWrite<R,Self> {
        FramedWrite::new(inner, Self::new())
    }
}

impl<T> Default for BincodeCodec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Encoder<T> for BincodeCodec<T>
    where T: Serialize
{
    type Error = bincode::Error;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let size = bincode::serialized_size(&item)? as u64;
        let header_size = bincode::serialized_size(&size)? as usize;

        let index = dst.len();
        dst.resize(index + header_size + size as usize, 0);
        let mut buf = &mut dst.as_mut()[index..];
        bincode::serialize_into(&mut buf, &size)?;
        bincode::serialize_into(&mut buf, &item)
    }
}

impl<T> Decoder for BincodeCodec<T>
    where for<'de> T: Deserialize<'de>
{
    type Item = T;
    type Error = bincode::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error>
    {
        let size = 0u64;
        let header_size = bincode::serialized_size(&size)? as usize;
        if src.len() < header_size {
            return Ok(None);
        }

        let buf = src.split_to(header_size);
        match bincode::deserialize(buf.as_ref()) {
            Err(err) => return Err(err),
            Ok(size) if src.len() < size => return Ok(None),
            Ok(size) => {
                let buf = src.split_to(size);
                bincode::deserialize::<Self::Item>(buf.as_ref())
                    .and_then(|item| Ok(Some(item)))
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    struct TestCase<T> {
        pub buffer: BytesMut,
        pub codec: BincodeCodec<T>,
        pub value: T
    }

    impl<T> TestCase<T>
        where T: Clone+Serialize
    {
        fn new(value: T) -> Self {
            Self {
                buffer: BytesMut::new(),
                codec: BincodeCodec::new(),
                value,
            }
        }

        fn encode(&mut self) {
            self.codec.encode(self.value.clone(), &mut self.buffer).unwrap();
        }
    }

    #[test]
    fn test_encode_decode_complete() {
        let mut case = TestCase::new(String::from("nothing flight like a bird"));
        case.encode();

        // decode complete message
        let decoded = case.codec.decode(&mut case.buffer)
            .unwrap_or_else(|err| panic!("decoding error: {}", err))
            .expect("decoding complete result is Ok(None)");

        if decoded != case.value {
            panic!("decoded is not encoded value ('{}' != '{}')", decoded, case.value);
        }
    }

    #[test]
    fn test_encode_decode_incomplete() {
        let mut case = TestCase::new(String::from("nothing flight like a bird"));
        case.encode();

        // test decoding incomplete
        let mut buffer = case.buffer.split_off(case.buffer.len() / 2);
        match case.codec.decode(&mut buffer) {
            Ok(None) => (),
            Err(err) => panic!("decoding error: {}", err),
            Ok(Some(_)) => panic!("got frame while it should return None"),
        }
    }
}

