use serde::{Serialize,Deserialize};


/// Error
#[derive(Clone,Serialize,Deserialize)]
pub enum Error {
    ///! Invalid input data received
    Format,
    ///! Request cancelled
    Cancelled,
    ///! Internal error
    Internal,
    ///! Service not found
    ServiceNotFound,
    ///! Service action not found
    ActionNotFound,
}



#[derive(Serialize,Deserialize)]
pub enum Message<Req,Resp,Er=Error>
    where Req: Send+Sync+Unpin,
          Resp: Send+Sync+Unpin,
          Er: Send+Sync+Unpin
{
    Request(Req),
    Response(Resp),
    Error(Er),
}

impl<Req,Resp,Er> Message<Req,Resp,Er>
    where Req: Send+Sync+Unpin,
          Resp: Send+Sync+Unpin,
          Er: Send+Sync+Unpin
{
    pub fn is_request(&self) -> bool {
        match self {
            Message::Request(_) => true,
            _ => false
        }
    }

    pub fn is_response(&self) -> bool {
        match self {
            Message::Response(_) => true,
            _ => false
        }
    }

    pub fn is_error(&self) -> bool {
        match self {
            Message::Error(_) => true,
            _ => false
        }
    }
}


