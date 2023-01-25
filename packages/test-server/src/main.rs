use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{Router, Server};
use rpc_axum::{http, websocket};
use rpc_test_data::{Add, TryMultiply, PORT};
use tower_http::cors::CorsLayer;

// TODO: HTTP and WebSocket servers
#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/http/add", http::handle_rpc::<Add>())
        .route("/http/multiply", http::handle_rpc::<TryMultiply>())
        .route("/websocket/add", websocket::handle_rpc::<Add>())
        .route(
            "/websocket/multiply",
            websocket::handle_rpc::<TryMultiply>(),
        )
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT))
        .serve(app.into_make_service())
        .await
        .unwrap();
}
