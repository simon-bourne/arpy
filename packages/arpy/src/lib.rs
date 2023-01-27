use std::{error::Error, fmt::Debug, future::Future, str::FromStr};

pub use arpy_macros::RpcId;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

pub trait FnRemote: id::RpcId + Serialize + DeserializeOwned + Debug {
    type Output: Serialize + DeserializeOwned + Debug;
}

pub trait FnRemoteBody<'a, Args>
where
    Args: FnRemote + 'a,
{
    type Fut: Future<Output = Args::Output> + Send + Sync + 'a;

    fn run(&self, args: &'a Args) -> Self::Fut;
}

impl<'a, Args, Fut, F> FnRemoteBody<'a, Args> for F
where
    Args: FnRemote + 'a,
    F: Fn(&'a Args) -> Fut,
    Fut: Future<Output = Args::Output> + Send + Sync + 'a,
{
    type Fut = Fut;

    fn run(&self, args: &'a Args) -> Self::Fut {
        self(args)
    }
}

#[async_trait(?Send)]
pub trait FnClient: FnRemote {
    async fn call<C>(&self, connection: &mut C) -> Result<Self::Output, C::Error>
    where
        C: RpcClient,
    {
        connection.call(self).await
    }
}

impl<T: FnRemote> FnClient for T {}

#[async_trait(?Send)]
pub trait FnTryCient<Success, Error>: FnRemote<Output = Result<Success, Error>> {
    async fn try_call<C>(&self, connection: &mut C) -> Result<Success, ErrorFrom<C::Error, Error>>
    where
        C: RpcClient,
    {
        connection.try_call(self).await
    }
}

impl<Success, Error, T> FnTryCient<Success, Error> for T where
    T: FnRemote<Output = Result<Success, Error>>
{
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
    type Error: Error + Debug + Send + Sync + 'static;

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

pub mod id {
    pub trait RpcId {
        const ID: &'static str;
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
