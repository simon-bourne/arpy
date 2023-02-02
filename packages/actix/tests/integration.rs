use std::{future::Future, net::SocketAddr};

use actix_web::{App, HttpServer};
use arpy::FnClient;
use arpy_actix::RpcApp;
use arpy_reqwest::Connection;
use arpy_server::WebSocketRouter;
use arpy_test::{
    server::{add, try_multiply},
    Add,
};
use reqwest::Client;

#[tokio::test]
async fn http_call() {
    end_to_end_test(|port| async move {
        let client = Client::new();
        let result = Add(2, 3)
            .call(&mut connection(&client, port))
            .await
            .unwrap();
        assert_eq!(result, 5);
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
    let server = HttpServer::new(move || {
        let ws = WebSocketRouter::new().handle(add).handle(try_multiply);

        App::new()
            .ws_rpc_route("ws", ws)
            .http_rpc_route("http", add)
            .http_rpc_route("http", try_multiply)
    })
    .bind(("127.0.0.1", 0))
    .unwrap();

    let port = match server.addrs().first().unwrap() {
        SocketAddr::V4(addr) => addr.port(),
        SocketAddr::V6(_) => panic!("IPv6 address"),
    };

    let server = server.run();
    let handle = server.handle();

    tokio::spawn(server);
    client(port).await;
    handle.stop(true).await;
}
