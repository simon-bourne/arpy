use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

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

#[derive(Serialize, Deserialize)]
struct TryMultiply(i32, i32);

#[async_trait]
impl RemoteFn for TryMultiply {
    type ResultType = Result<i32, ()>;

    async fn run(&self) -> Self::ResultType {
        Ok(self.0 * self.1)
    }
}

#[tokio::test]
async fn fallible_call() {
    end_to_end_test(|port| async move {
        let result = client(port, "add").call(&Add(2, 3)).await.unwrap();
        assert_eq!(result, 5);
    })
    .await
}

#[tokio::test]
async fn infallible_call() {
    end_to_end_test(|port| async move {
        let result = client(port, "multiply")
            .try_call(&TryMultiply(2, 3))
            .await
            .unwrap();
        assert_eq!(result, 6);
    })
    .await
}

fn client(port: u16, function: &str) -> Connection {
    Connection::new(
        &reqwest::Client::new(),
        format!("http://127.0.0.1:{port}/api/{function}"),
    )
}

async fn end_to_end_test<Client, Block>(client: Client)
where
    Client: FnOnce(u16) -> Block,
    Block: Future<Output = ()>,
{
    let app = Router::new()
        .route("/api/add", handle_rpc::<Add>())
        .route("/api/multiply", handle_rpc::<TryMultiply>());

    let server = Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .serve(app.into_make_service());
    let port = match server.local_addr() {
        SocketAddr::V4(addr) => addr.port(),
        SocketAddr::V6(_) => panic!("IPv6 address"),
    };
    server.with_graceful_shutdown(client(port)).await.unwrap();
}
