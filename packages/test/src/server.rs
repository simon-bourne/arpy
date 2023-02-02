use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use arpy_axum::RpcRoute;
use arpy_server::WebSocketRouter;
use axum::{
    response::{sse::Event, Sse},
    routing::{get, IntoMakeService},
    Router, Server,
};
use futures::{stream, Stream, StreamExt};
use hyper::server::conn::AddrIncoming;
use tower_http::cors::CorsLayer;

use crate::{Add, TryMultiply};

pub async fn add(args: Add) -> i32 {
    args.0 + args.1
}

pub async fn try_multiply(args: TryMultiply) -> Result<i32, ()> {
    Ok(args.0 * args.1)
}

async fn sse_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    Sse::new(stream::repeat_with(|| Event::default().json_data("Hi!".to_string()).unwrap()).map(Ok))
}

pub fn dev_server(port: u16) -> axum::Server<AddrIncoming, IntoMakeService<Router>> {
    let ws = WebSocketRouter::new().handle(add).handle(try_multiply);

    let app = Router::new()
        .route("/sse", get(sse_handler))
        .http_rpc_route("/http", add)
        .http_rpc_route("/http", try_multiply)
        .ws_rpc_route("/ws", ws)
        .layer(CorsLayer::permissive());

    Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
        .serve(app.into_make_service())
}
