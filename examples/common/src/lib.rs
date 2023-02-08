use std::convert::Infallible;

use arpy::{FnRemote, MsgId};
use futures::{stream, Stream};
use serde::{Deserialize, Serialize};

pub const PORT: u16 = 9090;

#[derive(MsgId, Serialize, Deserialize, Debug)]
pub struct MyFunction(pub String);

impl FnRemote for MyFunction {
    type Output = String;
}

#[derive(MsgId, Serialize, Deserialize, Debug)]
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

#[derive(MsgId, Serialize, Deserialize, Debug)]
pub struct Name(pub String);

pub fn name_stream() -> impl Stream<Item = Result<Name, Infallible>> {
    stream::iter(["Rita", "Sue", "Bob"].map(|name| Ok(Name(name.to_string()))))
}
