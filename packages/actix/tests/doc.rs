use std::sync::Arc;

use actix_web::{web, App, Responder};
use arpy::{FnRemote, RpcId};
use arpy_actix::{
    http::{handler, ArpyRequest, ArpyResponse},
    RpcApp,
};
use arpy_server::WebSocketRouter;
use serde::{Deserialize, Serialize};

#[derive(RpcId, Serialize, Deserialize, Debug)]
struct MyAdd(u32, u32);

impl FnRemote for MyAdd {
    type Output = u32;
}

async fn my_handler(ArpyRequest(args): ArpyRequest<MyAdd>) -> impl Responder {
    ArpyResponse(args.0 + args.1)
}

pub fn extractor_example() {
    App::new().route("api/my-add", web::post().to(my_handler));
}

async fn my_add(args: MyAdd) -> u32 {
    args.0 + args.1
}

pub fn handler_example() {
    App::new().route(
        "api/my-add",
        web::post().to(|args| handler(Arc::new(my_add), args)),
    );
}

pub fn router_example() {
    let ws = WebSocketRouter::new().handle(my_add);

    App::new()
        .ws_rpc_route("ws", ws)
        .http_rpc_route("http", my_add);
}
