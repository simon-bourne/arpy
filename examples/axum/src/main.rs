use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{Router, Server};
use rpc_axum::{websocket::WebSocketRouter, RpcRoute};
use rpc_example_rpc::{MyFunction, PORT};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let ws = WebSocketRouter::new().handle::<MyFunction>();
    let app = Router::new()
        .http_rpc_route::<MyFunction>("/http")
        .ws_rpc_route("/ws", ws)
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT))
        .serve(app.into_make_service())
        .await
        .unwrap()
}
