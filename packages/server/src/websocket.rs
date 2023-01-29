use std::{collections::HashMap, io, mem::size_of, result, sync::Arc};

use arpy::FnRemote;
use ciborium::de;
use futures::future::BoxFuture;
use thiserror::Error;

use crate::FnRemoteBody;

#[derive(Default)]
pub struct WebSocketRouter(HashMap<Id, RpcHandler>);

impl WebSocketRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle<F, T>(mut self, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static,
    {
        let id = T::ID.as_bytes().to_vec();
        let f = Arc::new(f);
        self.0.insert(
            id,
            Box::new(move |body| Box::pin(Self::run(f.clone(), body))),
        );

        self
    }

    async fn run<F, T>(f: Arc<F>, input: &[u8]) -> Result<Vec<u8>>
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static,
    {
        let args: T = ciborium::de::from_reader(input).map_err(Error::Deserialization)?;
        let result = f.run(args).await;
        let mut body = Vec::new();
        ciborium::ser::into_writer(&result, &mut body).unwrap();
        Ok(body)
    }
}

pub struct WebSocketHandler(HashMap<Id, RpcHandler>);

impl WebSocketHandler {
    pub fn new(router: WebSocketRouter) -> Self {
        Self(router.0)
    }

    pub async fn handle_msg(&self, msg: &[u8]) -> Result<Vec<u8>> {
        let (id, msg) = split_message(msg, size_of::<u32>(), "ID len")?;
        let id_len = u32::from_le_bytes(id.try_into().unwrap());
        let (id, params) = split_message(msg, id_len as usize, "ID")?;

        let Some(function) = self.0.get(id)
        else { return Err(Error::FunctionNotFound) };

        function(params).await
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
