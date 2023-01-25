use std::{fmt::Debug, str::FromStr};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

#[async_trait]
pub trait FnRemote: Serialize + DeserializeOwned + Debug + Send {
    type Output: Serialize + DeserializeOwned + Debug + Send;

    async fn run(&self) -> Self::Output;
}

#[derive(Error, Debug)]
pub enum ErrorFrom<C, S> {
    #[error("Connection: {0}")]
    Connection(C),
    #[error("Server: {0}")]
    Server(S),
}

#[async_trait(?Send)]
pub trait RpcClient {
    type Error;

    async fn call<F>(&mut self, function: &F) -> Result<F::Output, Self::Error>
    where
        F: FnRemote;

    async fn try_call<F, Success, Error>(
        &mut self,
        function: &F,
    ) -> Result<Success, ErrorFrom<Self::Error, Error>>
    where
        Self: Sized,
        F: FnRemote<Output = Result<Success, Error>>,
    {
        match self.call(function).await {
            Ok(Ok(ok)) => Ok(ok),
            Ok(Err(e)) => Err(ErrorFrom::Server(e)),
            Err(e) => Err(ErrorFrom::Connection(e)),
        }
    }
}

#[derive(Copy, Clone)]
pub enum MimeType {
    Cbor,
    Json,
}

impl MimeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cbor => "application/cbor",
            Self::Json => "application/json",
        }
    }
}

impl FromStr for MimeType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with(Self::Cbor.as_str()) {
            Ok(Self::Cbor)
        } else if s.starts_with(Self::Json.as_str()) {
            Ok(Self::Json)
        } else {
            Err(())
        }
    }
}
