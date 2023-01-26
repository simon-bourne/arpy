use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use arpy_axum::{RpcRoute, WebSocketRouter};
use arpy_example_common::{MyFallibleFunction, MyFunction, PORT};
use axum::{Router, Server};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let ws = WebSocketRouter::new()
        .handle::<MyFunction>()
        .handle::<MyFallibleFunction>();
    let app = Router::new()
        .http_rpc_route::<MyFunction>("/http")
        .http_rpc_route::<MyFallibleFunction>("/http")
        .ws_rpc_route("/ws", ws)
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT))
        .serve(app.into_make_service())
        .await
        .unwrap()
}
