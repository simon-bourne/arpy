use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use axum::{Router, Server};
use reqwest::Client;
use rpc::RpcClient;
use rpc_axum::http::handle_rpc;
use rpc_reqwest::Connection;
use rpc_test_data::{Add, TryMultiply};

#[tokio::test]
async fn fallible_http_call() {
    end_to_end_test(|port| async move {
        let client = Client::new();
        let result = connection(&client, port, "add")
            .call(&Add(2, 3))
            .await
            .unwrap();
        assert_eq!(result, 5);
    })
    .await
}

#[tokio::test]
async fn infallible_http_call() {
    end_to_end_test(|port| async move {
        let client = Client::new();
        let result = connection(&client, port, "multiply")
            .try_call(&TryMultiply(2, 3))
            .await
            .unwrap();
        assert_eq!(result, 6);
    })
    .await
}

fn connection<'a>(client: &'a Client, port: u16, function: &str) -> Connection<'a> {
    Connection::new(client, format!("http://127.0.0.1:{port}/api/{function}"))
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
