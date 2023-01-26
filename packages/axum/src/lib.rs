use arpy::FnRemote;
use axum::Router;

mod http;
mod websocket;

pub use websocket::WebSocketRouter;

pub trait RpcRoute {
    fn http_rpc_route<T>(self, prefix: &str) -> Self
    where
        T: FnRemote + Send + Sync + 'static;

    fn ws_rpc_route(self, route: &str, router: WebSocketRouter) -> Self;
}

impl RpcRoute for Router {
    fn http_rpc_route<T>(self, prefix: &str) -> Self
    where
        T: FnRemote + Send + Sync + 'static,
    {
        let id = T::ID;
        self.route(&format!("{prefix}/{id}",), http::handle_rpc::<T>())
    }

    fn ws_rpc_route(self, route: &str, router: WebSocketRouter) -> Self {
        self.route(route, router.into())
    }
}
