use std::{convert::Infallible, time::Duration};

use arpy::{FnRemote, MsgId};
use futures::{stream, Stream};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;

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
pub struct Count(pub i32);

pub fn counter_stream() -> impl Stream<Item = Result<Count, Infallible>> {
    stream::iter((0..).map(|count| Ok(Count(count)))).throttle(Duration::from_secs(1))
}
