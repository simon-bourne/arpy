use std::sync::Arc;

use arpy::{FnRemote, MimeType, MsgId};
use arpy_axum::{
    http::{handler, ArpyRequest, ArpyResponse},
    RpcRoute,
};
use arpy_server::WebSocketRouter;
use axum::{
    response::IntoResponse,
    routing::{post, Router},
};
use serde::{Deserialize, Serialize};

#[derive(MsgId, Serialize, Deserialize, Debug)]
struct MyAdd(u32, u32);

impl FnRemote for MyAdd {
    type Output = u32;
}

async fn my_handler(ArpyRequest(args): ArpyRequest<MyAdd>) -> impl IntoResponse {
    ArpyResponse::new(MimeType::Cbor, args.0 + args.1)
}

#[allow(unused_must_use)]
pub fn extractor_example() {
    Router::<()>::new().route("/api/my-add", post(my_handler));
}

async fn my_add(args: MyAdd) -> u32 {
    args.0 + args.1
}

#[allow(unused_must_use)]
pub fn handler_example() {
    Router::<()>::new().route(
        "/api/my-add",
        post(|headers, args| handler(headers, args, Arc::new(my_add))),
    );
}

#[allow(unused_must_use)]
pub fn router_example() {
    let ws = WebSocketRouter::new().handle(my_add);

    Router::new()
        .http_rpc_route("/http", my_add)
        .ws_rpc_route("/ws", ws, 1000);
}
