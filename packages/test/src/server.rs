use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{routing::IntoMakeService, Router, Server};
use hyper::server::conn::AddrIncoming;
use rpc_axum::RpcRoute;
use tower_http::cors::CorsLayer;

use crate::{Add, TryMultiply};

pub fn dev_server(port: u16) -> axum::Server<AddrIncoming, IntoMakeService<Router>> {
    let app = Router::new()
        .http_rpc_route::<Add>("/http")
        .http_rpc_route::<TryMultiply>("/http")
        .ws_rpc_route::<Add>("/ws")
        .ws_rpc_route::<TryMultiply>("/ws")
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
        .serve(app.into_make_service())
}