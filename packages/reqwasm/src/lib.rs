use std::fmt::{Debug, Display};

use thiserror::Error;

pub mod http;
pub mod websocket;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't deserialize result: {0}")]
    DeserializeResult(String),
    #[error("Couldn't send request: {0}")]
    Send(String),
    #[error("Couldn't receive response: {0}")]
    Receive(String),
    #[error("Invalid response 'content_type'")]
    InvalidResponseType(String),
}

impl Error {
    fn send(e: impl Display) -> Self {
        Self::Send(e.to_string())
    }

    fn receive(e: impl Display) -> Self {
        Self::Receive(e.to_string())
    }

    fn deserialize_result(e: impl Display) -> Self {
        Self::DeserializeResult(e.to_string())
    }
}
