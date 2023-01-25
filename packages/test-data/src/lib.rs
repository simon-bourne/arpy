use async_trait::async_trait;
use rpc::FnRemote;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Add(pub i32, pub i32);

#[async_trait]
impl FnRemote for Add {
    type Output = i32;

    async fn run(&self) -> Self::Output {
        self.0 + self.1
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TryMultiply(pub i32, pub i32);

#[async_trait]
impl FnRemote for TryMultiply {
    type Output = Result<i32, ()>;

    async fn run(&self) -> Self::Output {
        Ok(self.0 * self.1)
    }
}

pub const PORT: u16 = 9090;
