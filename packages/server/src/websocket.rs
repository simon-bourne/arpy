use std::{collections::HashMap, io, result, sync::Arc};

use arpy::{FnRemote, FnRemoteBody};
use ciborium::de;
use futures::future::BoxFuture;
use thiserror::Error;

#[derive(Default)]
pub struct WebSocketRouter(HashMap<Id, RpcHandler>);

impl WebSocketRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle<F, T>(mut self, f: F) -> Self
    where
        F: for<'a> FnRemoteBody<'a, T> + Send + Sync + 'static,
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

    async fn run<F, T>(f: Arc<F>, input: Vec<u8>) -> Result<Vec<u8>>
    where
        F: for<'a> FnRemoteBody<'a, T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static,
    {
        let args: T =
            ciborium::de::from_reader(input.as_slice()).map_err(Error::Deserialization)?;
        let result = f.run(&args).await;
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

    pub async fn handle_msg(&self, id: &[u8], params: Vec<u8>) -> Result<Vec<u8>> {
        let Some(function) = self.0.get(id)
        else { return Err(Error::FunctionNotFound) };

        function(params).await
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Function not found")]
    FunctionNotFound,
    #[error("Deserialization: {0}")]
    Deserialization(de::Error<io::Error>),
}

pub type Result<T> = result::Result<T, Error>;

type Id = Vec<u8>;
type RpcHandler =
    Box<dyn Fn(Vec<u8>) -> BoxFuture<'static, Result<Vec<u8>>> + Send + Sync + 'static>;
