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

pub async fn my_function(args: MyFunction) -> String {
    format!("Hello, {}", args.0)
}

pub async fn my_fallible_function(args: MyFallibleFunction) -> Result<String, String> {
    if args.0.is_empty() {
        Err("No name provided".to_string())
    } else {
        Ok(format!("Hello, {}", args.0))
    }
}
