use std::str::FromStr;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[async_trait]
pub trait RemoteFn: Send + Serialize + for<'a> Deserialize<'a> {
    type ResultType: Serialize + for<'a> Deserialize<'a>;

    async fn run(&self) -> Self::ResultType;
}

#[derive(Error, Debug)]
pub enum ErrorFrom<C, S> {
    #[error("Connection: {0}")]
    Connection(C),
    #[error("Server: {0}")]
    Server(S),
}

#[async_trait]
pub trait RpcClient {
    type Error;

    async fn call<'a, F>(self, function: &'a F) -> Result<F::ResultType, Self::Error>
    where
        F: RemoteFn,
        &'a F: Send;

    async fn try_call<F, Success, Error>(
        self,
        function: &F,
    ) -> Result<Success, ErrorFrom<Self::Error, Error>>
    where
        Self: Sized,
        F: RemoteFn<ResultType = Result<Success, Error>>,
        for<'a> &'a F: Send,
        Success: Send,
        Error: Send,
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
