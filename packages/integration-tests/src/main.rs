use std::{net::SocketAddr, thread, time::Duration};

use async_trait::async_trait;
use axum::{Router, Server};
use hyper::server::{self, conn::AddrIncoming};
use rpc::{RemoteFn, RpcClient};
use rpc_axum::handle_rpc;
use rpc_reqwest::Connection;
use serde::{Deserialize, Serialize};
use tokio::spawn;

#[derive(Serialize, Deserialize)]
struct Add(i32, i32);

#[async_trait]
impl RemoteFn for Add {
    type ResultType = i32;

    async fn run(&self) -> Self::ResultType {
        self.0 + self.1
    }
}

#[tokio::main]
async fn main() {
    let listener = Server::bind(&SocketAddr::from(([0, 0, 0, 0], 9090)));

    let client = spawn(client());
    let server = spawn(server(listener));

    assert_eq!(3, client.await.unwrap().unwrap());
    server.abort();
}

async fn client() -> Result<i32, rpc_reqwest::Error> {
    let client = Connection::new(&reqwest::Client::new(), "http://127.0.0.1:9090/api/add");
    client.call(&Add(1, 2)).await
}

async fn server(listener: server::Builder<AddrIncoming>) {
    thread::sleep(Duration::from_secs(10));
    let app = Router::new().route("/api/add", handle_rpc::<Add>());
    listener.serve(app.into_make_service()).await.unwrap();
}
