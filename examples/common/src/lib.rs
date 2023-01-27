use arpy::{FnRemote, RpcId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const PORT: u16 = 9090;

#[derive(RpcId, Serialize, Deserialize, Debug)]
pub struct MyFunction(pub String);

#[async_trait]
impl FnRemote for MyFunction {
    type Output = String;
}

#[derive(RpcId, Serialize, Deserialize, Debug)]
pub struct MyFallibleFunction(pub String);

#[async_trait]
impl FnRemote for MyFallibleFunction {
    type Output = Result<String, String>;
}
