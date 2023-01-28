use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use arpy_axum::RpcRoute;
use arpy_server::WebSocketRouter;
use axum::{routing::IntoMakeService, Router, Server};
use hyper::server::conn::AddrIncoming;
use tower_http::cors::CorsLayer;

use crate::{Add, TryMultiply};

async fn add(args: Add) -> i32 {
    args.0 + args.1
}

async fn try_multiply(args: TryMultiply) -> Result<i32, ()> {
    Ok(args.0 * args.1)
}

pub fn dev_server(port: u16) -> axum::Server<AddrIncoming, IntoMakeService<Router>> {
    let ws = WebSocketRouter::new().handle(add).handle(try_multiply);

    let app = Router::new()
        .http_rpc_route("/http", add)
        .http_rpc_route("/http", try_multiply)
        .ws_rpc_route("/ws", ws)
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
        .serve(app.into_make_service())
}
