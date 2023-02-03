//! Building blocks to implement an Arpy Websocket server.
//!
//! See the `axum` and `actix` implementations under `packages` in the
//! repository.
use std::{collections::HashMap, io, result, sync::Arc};

use arpy::FnRemote;
use bincode::Options;
use futures::future::BoxFuture;
use slotmap::DefaultKey;
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

    async fn run<F, FSig>(f: Arc<F>, mut input: impl io::Read) -> Result<Vec<u8>>
    where
        F: FnRemoteBody<FSig> + Send + Sync + 'static,
        FSig: FnRemote + Send + Sync + 'static,
    {
        let serializer = bincode::DefaultOptions::new();
        let args: FSig = serializer
            .deserialize_from(&mut input)
            .map_err(Error::Deserialization)?;
        let id: DefaultKey = serializer
            .deserialize_from(&mut input)
            .map_err(Error::Deserialization)?;
        let result = f.run(args).await;
        let mut body = Vec::new();
        serializer.serialize_into(&mut body, &id).unwrap();
        serializer.serialize_into(&mut body, &result).unwrap();

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
    /// This will read an `MsgId` from the message and route it to the correct
    /// implementation.
    pub async fn handle_msg(&self, mut msg: &[u8]) -> Result<Vec<u8>> {
        let id: Vec<u8> = bincode::DefaultOptions::new()
            .allow_trailing_bytes()
            .deserialize_from(&mut msg)
            .map_err(Error::Deserialization)?;

        let Some(function) = self.0.get(&id)
        else { return Err(Error::FunctionNotFound) };

        function(msg).await
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Function not found")]
    FunctionNotFound,
    #[error("Error unpacking message: {0}")]
    Protocol(String),
    #[error("Deserialization: {0}")]
    Deserialization(bincode::Error),
}

pub type Result<T> = result::Result<T, Error>;

type Id = Vec<u8>;
type RpcHandler =
    Box<dyn for<'a> Fn(&'a [u8]) -> BoxFuture<'a, Result<Vec<u8>>> + Send + Sync + 'static>;
