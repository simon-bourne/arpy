use axum::Router;
use arpy::{FnRemote, RpcId};

mod http;
mod websocket;

pub use websocket::WebSocketRouter;

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
