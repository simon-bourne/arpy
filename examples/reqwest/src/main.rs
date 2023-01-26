use anyhow::Result;
use arpy::{FnClient, FnTryCient, RpcClient};
use arpy_example_common::{MyFallibleFunction, MyFunction, PORT};
use arpy_reqwest::Connection;
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<()> {
    app(Connection::new(
        &Client::new(),
        format!("http://127.0.0.1:{PORT}/http"),
    ))
    .await
}

async fn app<Rpc: RpcClient>(mut connection: Rpc) -> Result<()> {
    let response = MyFunction("Arpy".to_string()).call(&mut connection).await?;
    println!("Response: {response}");
    let response = MyFallibleFunction("Arpy C".to_string())
        .try_call(&mut connection)
        .await?;
    println!("Response: {response}");

    Ok(())
}
