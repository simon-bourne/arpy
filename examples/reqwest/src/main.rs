use anyhow::Result;
use arpy::RpcClient;
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
    let response = connection.call(&MyFunction("Arpy".to_string())).await?;
    println!("Response: {response}");
    let response = connection
        .try_call(&MyFallibleFunction("Arpy C".to_string()))
        .await?;
    println!("Response: {response}");

    Ok(())
}
