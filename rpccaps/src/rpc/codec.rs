use std::{
	marker::PhantomData,
    pin::Pin,
};

use bytes::BytesMut;
use futures::io::{AsyncRead,AsyncWrite};
use futures::prelude::*;
use futures::task::{Context,Poll};

use bincode;
use serde::{Deserialize,Serialize};
pub use tokio_util::codec::{Decoder,Encoder};

use crate::{ErrorKind,Error};


/// FramedRead/Write compatible with futures::io's AsyncRead/Write
pub struct Framed<T,C>
{
    inner: T,
    codec: C,
    chunk_size: usize,
    buffer: BytesMut,
}


impl<T,C> Framed<T,C>
{
    pub fn new(inner: T, codec: C) -> Self {
        Self::with_capacity(inner, codec, 128)
    }

    pub fn with_capacity(inner: T, codec: C, capacity: usize) -> Self {
        let mut buffer = BytesMut::with_capacity(capacity);
        Self { inner, codec, chunk_size: capacity, buffer: BytesMut::new() }
    }

    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T,C> Stream for Framed<T,C>
    where T: AsyncRead+Unpin,
          C: Decoder+Unpin,
{
    type Item = C::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>>
    {
        let mut this = self.as_mut();
        let buffer_size = this.buffer.len();

        if this.buffer.len() + this.chunk_size < this.buffer.capacity() {
            let len = this.buffer.len() + this.chunk_size;
            this.buffer.resize(len, 0);
        }

        let mut buffer = BytesMut::new();
        std::mem::swap(&mut buffer, &mut this.buffer);

        let poll = Pin::new(&mut this.inner)
                        .poll_read(cx, &mut buffer[buffer_size..]);
        let r = match poll {
            Poll::Ready(Ok(size)) => {
                buffer.resize(buffer_size+size, 0);
                match this.codec.decode(&mut buffer) {
                    Ok(Some(item)) => Poll::Ready(Some(item)),
                    Ok(None) => Poll::Pending,
                    Err(_) => Poll::Ready(None),
                }
            },
            Poll::Ready(Err(_)) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        };

        std::mem::swap(&mut buffer, &mut this.buffer);
        r
    }
}

impl<T,C,I> Sink<I> for Framed<T,C>
    where T: AsyncWrite+Unpin,
          C: Encoder<I>+Unpin,
{
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Result<(), Self::Error>>
    {
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, item: I)
        -> Result<(), Self::Error>
    {
        let mut this = self.as_mut();
        let mut buffer = BytesMut::new();
        std::mem::swap(&mut buffer, &mut this.buffer);

        let r = this.codec.encode(item, &mut buffer)
            		.or_else(|_| ErrorKind::Codec.err("encoding error"));
        std::mem::swap(&mut buffer, &mut this.buffer);
        r
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Result<(), Self::Error>>
    {
        let mut this = self.as_mut();
        let mut buffer = BytesMut::new();
        std::mem::swap(&mut buffer, &mut this.buffer);

        let r = match Pin::new(&mut this.inner).poll_write(cx, &mut buffer) {
            Poll::Ready(Ok(size)) => match this.buffer.split_at(size).0.len() {
                x if x > 0 => Poll::Pending,
                _ => Poll::Ready(Ok(())),
            },
            Poll::Ready(Err(err)) => Poll::Ready(ErrorKind::IO.err(err.to_string())),
            Poll::Pending => Poll::Pending,
        };

        std::mem::swap(&mut buffer, &mut this.buffer);
        r
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Result<(), Self::Error>>
    {
        let mut this = self.as_mut();
        match Pin::new(&mut this.inner).poll_close(cx) {
            Poll::Ready(Err(err)) => Poll::Ready(ErrorKind::IO.err(err.to_string())),
            Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
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
    pub fn framed_read<R: AsyncRead>(inner: R) -> Framed<R,Self> {
        Framed::new(inner, Self::new())
    }
}

impl<T> BincodeCodec<T>
    where T: Serialize
{
    pub fn framed_write<R: AsyncWrite>(inner: R) -> Framed<R,Self> {
        Framed::new(inner, Self::new())
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

