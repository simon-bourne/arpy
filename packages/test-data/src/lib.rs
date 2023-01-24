use async_trait::async_trait;
use rpc::RemoteFn;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Add(pub i32, pub i32);

#[async_trait]
impl RemoteFn for Add {
    type ResultType = i32;

    async fn run(&self) -> Self::ResultType {
        self.0 + self.1
    }
}

#[derive(Serialize, Deserialize)]
pub struct TryMultiply(pub i32, pub i32);

#[async_trait]
impl RemoteFn for TryMultiply {
    type ResultType = Result<i32, ()>;

    async fn run(&self) -> Self::ResultType {
        Ok(self.0 * self.1)
    }
}

pub const PORT: u16 = 9090;
