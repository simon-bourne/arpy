use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use arpy_axum::RpcRoute;
use arpy_example_common::{counter_stream, my_fallible_function, my_function, PORT};
use arpy_server::WebSocketRouter;
use axum::{Router, Server};
use tower_http::cors::CorsLayer;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let ws = WebSocketRouter::new()
        .handle(my_function)
        .handle(my_fallible_function);
    let app = Router::new()
        .sse_route("/sse", counter_stream, None)
        .http_rpc_route("/http", my_function)
        .http_rpc_route("/http", my_fallible_function)
        .ws_rpc_route("/ws", ws, 1000)
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT))
        .serve(app.into_make_service())
        .await
        .unwrap()
}
