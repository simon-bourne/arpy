//! # Arpy Actix
//!
//! [`arpy`] integration for [`actix_web`].
//!
//! ## Example
//!
//! ```
#![doc = include_doc::function_body!("tests/doc.rs", router_example, [my_add, MyAdd])]
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
pub trait RpcApp {
    /// Add an HTTP route to handle a single RPC endpoint.
    ///
    /// Routes are constructed with `"{prefix}/{msg_id}"` where `msg_id = T::ID`
    /// from [`MsgId`][arpy::protocol::MsgId].
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + 'static,
        T: FnRemote + 'static;

    /// Add all the RPC endpoints in `router` to a websocket endpoint at `path`.
    fn ws_rpc_route(self, path: &str, router: WebSocketRouter) -> Self;
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

    fn ws_rpc_route(self, path: &str, router: WebSocketRouter) -> Self {
        let handler = WebSocketHandler::new(router);
        self.route(
            path,
            web::get().to(move |req: HttpRequest, body: web::Payload| {
                let handler = handler.clone();
                websocket::handler(handler, req, body)
            }),
        )
    }
}
