use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{Router, Server};
use rpc_axum::{http, websocket};
use rpc_example_rpc::{MyFunction, PORT};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/http/my-function", http::handle_rpc::<MyFunction>())
        .route("/ws/my-function", websocket::handle_rpc::<MyFunction>())
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT))
        .serve(app.into_make_service())
        .await
        .unwrap()
}
