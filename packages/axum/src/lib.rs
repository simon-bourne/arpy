use arpy::{FnRemote, FnRemoteBody};
use axum::Router;

mod http;
mod websocket;

pub use websocket::WebSocketRouter;

pub trait RpcRoute {
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: for<'a> FnRemoteBody<'a, T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static;

    fn ws_rpc_route(self, route: &str, router: WebSocketRouter) -> Self;
}

impl RpcRoute for Router {
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: for<'a> FnRemoteBody<'a, T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static,
    {
        let id = T::ID;
        self.route(&format!("{prefix}/{id}",), http::handle_rpc(f))
    }

    fn ws_rpc_route(self, route: &str, router: WebSocketRouter) -> Self {
        self.route(route, router.into())
    }
}
