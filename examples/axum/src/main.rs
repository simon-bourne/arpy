use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use arpy_axum::{RpcRoute, WebSocketRouter};
use arpy_example_common::{MyFallibleFunction, MyFunction, PORT};
use axum::{Router, Server};
use tower_http::cors::CorsLayer;

async fn my_function(args: &MyFunction) -> String {
    format!("Hello, {}", args.0)
}

async fn my_fallible_function(args: &MyFallibleFunction) -> Result<String, String> {
    if args.0.is_empty() {
        Err("No name provided".to_string())
    } else {
        Ok(format!("Hello, {}", args.0))
    }
}

#[tokio::main]
async fn main() {
    let ws = WebSocketRouter::new()
        .handle(my_function)
        .handle(my_fallible_function);
    let app = Router::new()
        .http_rpc_route("/http", my_function)
        .http_rpc_route("/http", my_fallible_function)
        .ws_rpc_route("/ws", ws)
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT))
        .serve(app.into_make_service())
        .await
        .unwrap()
}
