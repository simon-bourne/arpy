use axum::Router;
use rpc::{FnRemote, RpcId};
use websocket::WebSocketRouter;

pub mod http;
pub mod websocket;

// TODO: Websockets should implement routing on the websocket, so we don't have
// to open multiple websockets. We need a WebSocket builder and have the
// implementations send/expect a "type" message before the main message.
pub trait RpcRoute {
    fn http_rpc_route<T: FnRemote + RpcId + 'static>(self, prefix: &str) -> Self
    where
        for<'a> &'a T: Send;

    fn ws_rpc_route(self, route: &str, router: WebSocketRouter) -> Self;
}

impl RpcRoute for Router {
    fn http_rpc_route<T: FnRemote + RpcId + Send + 'static>(self, prefix: &str) -> Self
    where
        for<'a> &'a T: Send,
    {
        let id = T::ID;
        self.route(&format!("{prefix}/{id}",), http::handle_rpc::<T>())
    }

    fn ws_rpc_route(self, route: &str, router: WebSocketRouter) -> Self {
        self.route(route, router.into())
    }
}
