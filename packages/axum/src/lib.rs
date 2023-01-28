use arpy::FnRemote;
use arpy_server::{FnRemoteBody, WebSocketRouter};
use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    routing::get,
    Router,
};
use websocket::WebSocketHandler;

pub mod http;
mod websocket;

// TODO: Forms:
// POST: x-www-form-urlencoded with serde?
// GET
pub trait RpcRoute {
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static;

    fn ws_rpc_route(self, route: &str, router: WebSocketRouter) -> Self;
}

impl RpcRoute for Router {
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static,
    {
        let id = T::ID;
        self.route(&format!("{prefix}/{id}",), http::handle_rpc(f))
    }

    fn ws_rpc_route(self, route: &str, router: WebSocketRouter) -> Self {
        let handler = WebSocketHandler::new(router);

        self.route(
            route,
            get(|ws: WebSocketUpgrade| async {
                ws.on_upgrade(
                    |socket: WebSocket| async move { handler.handle_socket(socket).await },
                )
            }),
        )
    }
}
