use reqwest::Client;
use rpc::RpcClient;
use rpc_example_rpc::{MyFunction, PORT};
use rpc_reqwest::Connection;

#[tokio::main]
async fn main() -> Result<(), rpc_reqwest::Error> {
    // TODO: We want to be able to call any registered function from the same
    // connection.
    app(Connection::new(
        &Client::new(),
        format!("http://127.0.0.1:{PORT}/http"),
    ))
    .await
}

async fn app<Rpc: RpcClient>(mut connection: Rpc) -> Result<(), Rpc::Error> {
    let response = connection.call(&MyFunction("Arpy".to_string())).await?;

    println!("Response: {response}");
    Ok(())
}
