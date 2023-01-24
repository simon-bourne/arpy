use std::str::FromStr;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// TODO: `RemoteFallibleFn`,
#[async_trait]
pub trait RemoteFn: Send + Serialize + for<'a> Deserialize<'a> {
    type ResultType: Serialize + for<'a> Deserialize<'a>;

    async fn run(&self) -> Self::ResultType;
}

#[async_trait]
pub trait RpcClient {
    type Error;

    async fn call<'a, F>(self, function: &'a F) -> Result<F::ResultType, Self::Error>
    where
        F: RemoteFn,
        &'a F: Send;
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
