use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{routing::IntoMakeService, Router, Server};
use hyper::server::conn::AddrIncoming;
use rpc_axum::{http, websocket};
use rpc_test_data::{Add, TryMultiply};
use tower_http::cors::CorsLayer;

pub fn dev_server(port: u16) -> axum::Server<AddrIncoming, IntoMakeService<Router>> {
    let app = Router::new()
        .route("/http/add", http::handle_rpc::<Add>())
        .route("/http/multiply", http::handle_rpc::<TryMultiply>())
        .route("/ws/add", websocket::handle_rpc::<Add>())
        .route("/ws/multiply", websocket::handle_rpc::<TryMultiply>())
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
        .serve(app.into_make_service())
}
