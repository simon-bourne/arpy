use async_trait::async_trait;
use rpc::{FnRemote, RpcId};
use serde::{Deserialize, Serialize};

pub const PORT: u16 = 9090;
pub const MY_FUNCTION: &str = "my-function";

#[derive(Serialize, Deserialize, Debug)]
pub struct MyFunction(pub String);

#[async_trait]
impl FnRemote for MyFunction {
    type Output = String;

    async fn run(&self) -> Self::Output {
        format!("Hello, {}", self.0)
    }
}

impl RpcId for MyFunction {
    const ID: &'static str = "my-function";
}
