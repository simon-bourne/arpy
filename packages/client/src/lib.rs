//! Portable client for Arpy.
use std::fmt::{Debug, Display};

use futures::Future;
use thiserror::Error;

pub mod websocket;

/// The errors that can happen during an RPC.
///
/// Note; This may contain sensitive information such as URLs or argument
/// names/values.
#[derive(Error, Debug, Clone)]
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
    pub fn send(e: impl Display) -> Self {
        Self::Send(e.to_string())
    }

    pub fn receive(e: impl Display) -> Self {
        Self::Receive(e.to_string())
    }

    pub fn deserialize_result(e: impl Display) -> Self {
        Self::DeserializeResult(e.to_string())
    }
}

pub trait Spawner {
    fn spawn_local<F>(&self, future: F)
    where
        F: Future<Output = ()> + 'static;
}
