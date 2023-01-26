use axum::{routing::MethodRouter, Router};
use rpc::{FnRemote, RpcId};

pub mod http;
pub mod websocket;

// TODO: Websockets should implement routing on the websocket, so we don't have
// to open multiple websockets. We need a WebSocket builder and have the
// implementations send/expect a "type" message before the main message.
pub trait RpcRoute {
    fn http_rpc_route<T: FnRemote + RpcId + 'static>(self, prefix: &str) -> Self
    where
        for<'a> &'a T: Send;

    fn ws_rpc_route<T: FnRemote + RpcId + 'static>(self, prefix: &str) -> Self
    where
        for<'a> &'a T: Send;
}

impl RpcRoute for Router {
    fn http_rpc_route<T: FnRemote + RpcId + Send + 'static>(self, prefix: &str) -> Self
    where
        for<'a> &'a T: Send,
    {
        route(self, T::ID, prefix, http::handle_rpc::<T>())
    }

    fn ws_rpc_route<T: FnRemote + RpcId + Send + 'static>(self, prefix: &str) -> Self
    where
        for<'a> &'a T: Send,
    {
        route(self, T::ID, prefix, websocket::handle_rpc::<T>())
    }
}

fn route(router: Router, id: &str, prefix: &str, method_router: MethodRouter) -> Router {
    router.route(&format!("{prefix}/{}", id), method_router)
}
