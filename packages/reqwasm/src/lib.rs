//! Reqwasm client for Arpy.
//! 
//! This provides an [`http`] and a [`websocket`] client, suitable for use from a browser.
use std::fmt::{Debug, Display};

use thiserror::Error;

pub mod http;
pub mod websocket;

/// The errors that can happen during an RPC.
///
/// Note; This may contain sensitive information such as URLs or argument
/// names/values.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't deserialize result: {0}")]
    DeserializeResult(String),
    #[error("Couldn't send request: {0}")]
    Send(String),
    #[error("Couldn't receive response: {0}")]
    Receive(String),
    #[error("Invalid response 'content_type'")]
    UnknownContentType(String),
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
