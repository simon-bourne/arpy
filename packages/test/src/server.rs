use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use arpy_axum::RpcRoute;
use arpy_server::WebSocketRouter;
use axum::{routing::IntoMakeService, Router, Server};
use futures::{stream, Stream};
use hyper::server::conn::AddrIncoming;
use tower_http::cors::CorsLayer;

use crate::{Add, Counter, TryMultiply};

pub async fn add(args: Add) -> i32 {
    args.0 + args.1
}

pub async fn try_multiply(args: TryMultiply) -> Result<i32, ()> {
    Ok(args.0 * args.1)
}

fn sse_stream() -> impl Stream<Item = Result<Add, Infallible>> {
    stream::repeat_with(|| Ok(Add(1, 2)))
}

fn counter_stream(args: Counter) -> impl Stream<Item = i32> {
    stream::iter(args.0..)
}

pub fn dev_server(port: u16) -> axum::Server<AddrIncoming, IntoMakeService<Router>> {
    let ws = WebSocketRouter::new()
        .handle(add)
        .handle(try_multiply)
        .handle_subscription(counter_stream);

    let app = Router::new()
        .sse_route("/sse", sse_stream, None)
        .http_rpc_route("/http", add)
        .http_rpc_route("/http", try_multiply)
        .ws_rpc_route("/ws", ws)
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
        .serve(app.into_make_service())
}
