use arpy::{FnRemote, RpcId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub mod server;

#[derive(Serialize, Deserialize, Debug)]
pub struct Add(pub i32, pub i32);

#[async_trait]
impl FnRemote for Add {
    type Output = i32;

    async fn run(&self) -> Self::Output {
        self.0 + self.1
    }
}

impl RpcId for Add {
    const ID: &'static str = "add";
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

impl RpcId for TryMultiply {
    const ID: &'static str = "multiply";
}

pub const PORT: u16 = 9090;
