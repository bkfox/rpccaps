//! Provide bi-directionnal MPMC broadcast
use std::io;
use std::pin::Pin;

use futures::channel::{mpsc,oneshot};
use futures::prelude::*;
use futures::task::{Context,Poll};
use tokio::io::{AsyncRead,AsyncWrite,ReadBuf};


/// Transport implementing `Stream+Sink` or `AsyncRead+AsyncWrite` depending
/// of the provided sender and receiver.
///
/// It also implements mpsc & oneshot bidirectionnal channels instanciation.
pub struct Transport<S,R> {
    /// Sender
    pub sender: S,
    /// Receiver
    pub receiver: R,
}

/// Transport of mpsc sender and receiver.
pub type MPSCTransport<S,R> = Transport<mpsc::Sender<S>, mpsc::Receiver<R>>;
pub type OneshotTransport<S,R> = Transport<oneshot::Sender<S>, oneshot::Receiver<R>>;


impl<S,R> Transport<S,R>
{
    /// Return new transport instance.
    pub fn new(sender: S, receiver: R) -> Self {
        Self { sender, receiver }
    }

    /// Return inner sender and receiver
    pub fn into_inner(self) -> (S,R) {
        (self.sender, self.receiver)
    }
}

impl<S,R> Transport<mpsc::Sender<S>, mpsc::Receiver<R>>
{
    /// Return new bidirectionnal mpsc transport.
    pub fn bi(cap: usize) -> (Self, MPSCTransport<R,S>) {
        let (rs, rr) = mpsc::channel(cap);
        let (ss, sr) = mpsc::channel(cap);
        (MPSCTransport::new(rs, sr), MPSCTransport::new(ss, rr))
    }
}

impl<S,R> Transport<oneshot::Sender<S>, oneshot::Receiver<R>>
{
    /// Return new bidirectionnal oneshot transport.
    pub fn bi() -> (Self, OneshotTransport<R,S>) {
        let (rs, rr) = oneshot::channel();
        let (ss, sr) = oneshot::channel();
        (OneshotTransport::new(rs, sr), OneshotTransport::new(ss, rr))
    }
}


impl<I,S,R> Sink<I> for Transport<S,R>
    where S: Sink<I>+Unpin, R: Unpin
{
    type Error = S::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().sender).poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: I) -> Result<(), Self::Error> {
        Pin::new(&mut self.get_mut().sender).start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().sender).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {

        Pin::new(&mut self.get_mut().sender).poll_close(cx)
    }
}


impl<S,R> Stream for Transport<S,R>
    where R: Stream+Unpin, S: Unpin
{
    type Item = R::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().receiver).poll_next(cx)
    }
}


impl<S,R> AsyncRead for Transport<S,R>
    where S: AsyncWrite+Unpin, R: AsyncRead+Unpin
{
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>)
        -> Poll<io::Result<()>>
    {
        Pin::new(&mut self.get_mut().receiver).poll_read(cx, buf)
    }
}

impl<S,R> AsyncWrite for Transport<S,R>
    where S: AsyncWrite+Unpin, R: AsyncRead+Unpin
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8])
        -> Poll<io::Result<usize>>
    {
        Pin::new(&mut self.get_mut().sender).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<io::Result<()>>
    {
        Pin::new(&mut self.get_mut().sender).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<io::Result<()>>
    {
        Pin::new(&mut self.get_mut().sender).poll_shutdown(cx)
    }
}

