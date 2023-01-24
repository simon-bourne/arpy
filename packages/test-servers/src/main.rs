use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{Router, Server};
use rpc_axum::handle_rpc;
use rpc_test_data::{Add, TryMultiply, PORT};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/add", handle_rpc::<Add>())
        .route("/api/multiply", handle_rpc::<TryMultiply>());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT))
        .serve(app.into_make_service())
        .await
        .unwrap();
}
