use std::{thread, time::Duration, net::SocketAddr};

use async_trait::async_trait;
use axum::{Router, Server};
use rpc::{RemoteFn, RpcClient};
use rpc_axum::handle_rpc;
use rpc_reqwest::Connection;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Add(i32, i32);

#[async_trait]
impl RemoteFn for Add {
    type ResultType = i32;

    async fn run(&self) -> Self::ResultType {
        self.0 + self.1
    }
}

fn main() {
    thread::scope(|scope| {
        scope.spawn(server);
        scope.spawn(|| {
            thread::sleep(Duration::from_secs(1));
            client();
        });
    });
}

#[tokio::main]
async fn client() {
    let client = Connection::new(&reqwest::Client::new(), "http://127.0.0.1:9090/api/add");
    let result = client.call(&Add(1, 2)).await;
    println!("{:?}", result);
}

#[tokio::main]
async fn server() {
    let app = Router::new().route("/api/add", handle_rpc::<Add>());
    Server::bind(&SocketAddr::from(([0, 0, 0, 0], 9090)))
        .serve(app.into_make_service())
        .await
        .unwrap();
}
