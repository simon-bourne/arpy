//! # Arpy Axum
//!
//! [`arpy`] integration for [`axum`].
//!
//! ## Example
//!
//! ```
//! # use arpy::{FnRemote, RpcId};
//! # use arpy_axum::RpcRoute;
//! # use axum::{Router, Server};
//! # use arpy_server::WebSocketRouter;
//! # use serde::{Deserialize, Serialize};
//! #
//! #[derive(RpcId, Serialize, Deserialize, Debug)]
//! pub struct MyFunction(pub String);
//!
//! impl FnRemote for MyFunction {
//!     type Output = String;
//! }
//!
//! pub async fn my_function(args: MyFunction) -> String {
//!     format!("Hello, {}", args.0)
//! }
//!
//! let ws = WebSocketRouter::new().handle(my_function);
//!
//! Router::new()
//!     .http_rpc_route("/http", my_function)
//!     .ws_rpc_route("/ws", ws);
//! ```
use std::sync::Arc;

use arpy::FnRemote;
use arpy_server::{FnRemoteBody, WebSocketRouter};
use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    routing::{get, post},
    Router,
};
use http::ArpyRequest;
use hyper::HeaderMap;
use websocket::WebSocketHandler;

pub mod http;
mod websocket;

/// Extension trait to add RPC routes. See [module level documentation][crate]
/// for an example.
pub trait RpcRoute {
    /// Add an HTTP route to handle a single RPC endpoint.
    ///
    /// Routes are constructed with `"{prefix}/{rpc_id}"` where `rpc_id = T::ID`
    /// from [`RpcId`][arpy::id::RpcId].
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static;

    /// Add all the RPC endpoints in `router` to a websocket endpoint at `path`.
    fn ws_rpc_route(self, path: &str, router: WebSocketRouter) -> Self;
}

impl RpcRoute for Router {
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static,
    {
        let id = T::ID;
        let f = Arc::new(f);
        self.route(
            &format!("{prefix}/{id}",),
            post(move |headers: HeaderMap, arpy: ArpyRequest<T>| http::handler(headers, arpy, f)),
        )
    }

    fn ws_rpc_route(self, path: &str, router: WebSocketRouter) -> Self {
        let handler = WebSocketHandler::new(router);

        self.route(
            path,
            get(|ws: WebSocketUpgrade| async {
                ws.on_upgrade(
                    |socket: WebSocket| async move { handler.handle_socket(socket).await },
                )
            }),
        )
    }
}
