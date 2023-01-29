//! # Arpy Actix
//!
//! [`arpy`] integration for [`actix_web`].
//!
//! ## Example
//!
//! ```
//! # use actix_web::App;
//! # use arpy::{FnRemote, RpcId};
//! # use arpy_actix::RpcApp;
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
//! App::new()
//!     .ws_rpc_route("ws", ws)
//!     .http_rpc_route("http", my_function);
//! ```
use std::sync::Arc;

use actix_web::{
    dev::{ServiceFactory, ServiceRequest},
    web::{self},
    App, Error, HttpRequest,
};
use arpy::FnRemote;
use arpy_server::{FnRemoteBody, WebSocketRouter};
use websocket::WebSocketHandler;

pub mod http;
mod websocket;

/// Extension trait to add RPC routes. See [module level documentation][crate]
/// for an example.
///
/// Routes are constructed with `"{prefix}/{rpc_id}"` where `rpc_id = T::ID`
/// from [`RpcId`][arpy::id::RpcId].
pub trait RpcApp {
    /// Add an HTTP route to handle a single RPC endpoint.
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + 'static,
        T: FnRemote + 'static;

    /// Add an HTTP route to handle all the RPC endpoints in `routes`.
    fn ws_rpc_route(self, path: &str, routes: WebSocketRouter) -> Self;
}

impl<S> RpcApp for App<S>
where
    S: ServiceFactory<ServiceRequest, Config = (), Error = Error, InitError = ()>,
{
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + 'static,
        T: FnRemote + 'static,
    {
        let id = T::ID;
        let f = Arc::new(f);

        self.route(
            &format!("{prefix}/{id}"),
            web::post().to(move |request| http::handler(f.clone(), request)),
        )
    }

    fn ws_rpc_route(self, path: &str, routes: WebSocketRouter) -> Self {
        let handler = WebSocketHandler::new(routes);
        self.route(
            path,
            web::get().to(move |req: HttpRequest, body: web::Payload| {
                let handler = handler.clone();
                websocket::handler(handler, req, body)
            }),
        )
    }
}
