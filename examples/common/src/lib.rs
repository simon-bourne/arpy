use arpy::{FnRemote, RpcId};
use serde::{Deserialize, Serialize};

pub const PORT: u16 = 9090;

#[derive(RpcId, Serialize, Deserialize, Debug)]
pub struct MyFunction(pub String);

impl FnRemote for MyFunction {
    type Output = String;
}

#[derive(RpcId, Serialize, Deserialize, Debug)]
pub struct MyFallibleFunction(pub String);

impl FnRemote for MyFallibleFunction {
    type Output = Result<String, String>;
}
