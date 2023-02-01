//! Building blocks to implement an Arpy Websocket server.
//!
//! See the `axum` and `actix` implementations under `packages` in the
//! repository.
use std::{collections::HashMap, io, mem::size_of, result, sync::Arc};

use arpy::FnRemote;
use ciborium::de;
use futures::future::BoxFuture;
use thiserror::Error;

use crate::FnRemoteBody;

/// A collection of RPC calls to be handled by a WebSocket.
#[derive(Default)]
pub struct WebSocketRouter(HashMap<Id, RpcHandler>);

impl WebSocketRouter {
    /// Construct an empty router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a handler for any RPC calls to `FSig`.
    pub fn handle<F, FSig>(mut self, f: F) -> Self
    where
        F: FnRemoteBody<FSig> + Send + Sync + 'static,
        FSig: FnRemote + Send + Sync + 'static,
    {
        let id = FSig::ID.as_bytes().to_vec();
        let f = Arc::new(f);
        self.0.insert(
            id,
            Box::new(move |body| Box::pin(Self::run(f.clone(), body))),
        );

        self
    }

    async fn run<F, FSig>(f: Arc<F>, input: &[u8]) -> Result<Vec<u8>>
    where
        F: FnRemoteBody<FSig> + Send + Sync + 'static,
        FSig: FnRemote + Send + Sync + 'static,
    {
        let args: FSig = ciborium::de::from_reader(input).map_err(Error::Deserialization)?;
        let result = f.run(args).await;
        let mut body = Vec::new();
        ciborium::ser::into_writer(&result, &mut body).unwrap();
        Ok(body)
    }
}

/// Handle raw messages from a websocket.
///
/// Use `WebSocketHandler` to implement a Websocket server.
pub struct WebSocketHandler(HashMap<Id, RpcHandler>);

impl WebSocketHandler {
    pub fn new(router: WebSocketRouter) -> Self {
        Self(router.0)
    }

    /// Handle a raw Websocket message.
    ///
    /// This will read an `RpcId` from the message and route it to the correct
    /// implementation.
    pub async fn handle_msg(&self, msg: &[u8]) -> Result<Vec<u8>> {
        let (id, msg) = split_message(msg, size_of::<u32>(), "ID len")?;
        let id_len = u32::from_le_bytes(id.try_into().unwrap());
        let (id, args) = split_message(msg, id_len as usize, "ID")?;

        let Some(function) = self.0.get(id)
        else { return Err(Error::FunctionNotFound) };

        function(args).await
    }
}

fn split_message<'a>(msg: &'a [u8], mid: usize, name: &str) -> Result<(&'a [u8], &'a [u8])> {
    if mid > msg.len() {
        return Err(Error::Protocol(format!("Not enought bytes for {name}")));
    }

    Ok(msg.split_at(mid))
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Function not found")]
    FunctionNotFound,
    #[error("Error unpacking message: {0}")]
    Protocol(String),
    #[error("Deserialization: {0}")]
    Deserialization(de::Error<io::Error>),
}

pub type Result<T> = result::Result<T, Error>;

type Id = Vec<u8>;
type RpcHandler =
    Box<dyn for<'a> Fn(&'a [u8]) -> BoxFuture<'a, Result<Vec<u8>>> + Send + Sync + 'static>;
