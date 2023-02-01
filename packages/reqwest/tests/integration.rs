use std::{future::Future, net::SocketAddr};

use arpy::{FnClient, FnTryCient};
use arpy_reqwest::Connection;
use arpy_test::{server::dev_server, Add, TryMultiply};
use reqwest::Client;

#[tokio::test]
async fn fallible_http_call() {
    end_to_end_test(|port| async move {
        let client = Client::new();
        let result = Add(2, 3).call(&connection(&client, port)).await.unwrap();
        assert_eq!(result, 5);
    })
    .await
}

#[tokio::test]
async fn infallible_http_call() {
    end_to_end_test(|port| async move {
        let client = Client::new();
        let result = TryMultiply(2, 3)
            .try_call(&connection(&client, port))
            .await
            .unwrap();
        assert_eq!(result, 6);
    })
    .await
}

fn connection(client: &Client, port: u16) -> Connection {
    Connection::new(client, format!("http://127.0.0.1:{port}/http"))
}

async fn end_to_end_test<Client, Block>(client: Client)
where
    Client: FnOnce(u16) -> Block,
    Block: Future<Output = ()>,
{
    let server = dev_server(0);
    let port = match server.local_addr() {
        SocketAddr::V4(addr) => addr.port(),
        SocketAddr::V6(_) => panic!("IPv6 address"),
    };
    server.with_graceful_shutdown(client(port)).await.unwrap();
}
