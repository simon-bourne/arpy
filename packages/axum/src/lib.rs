//! # Arpy Axum
//!
//! [`arpy`] integration for [`axum`].
//!
//! ## Example
//!
//! ```
#![doc = include_doc::function_body!("tests/doc.rs", router_example, [my_add, MyAdd])]
//! ```
use std::sync::Arc;

use arpy::{protocol, FnRemote};
use arpy_server::{FnRemoteBody, WebSocketRouter};
use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
    routing::{get, post},
    BoxError, Router,
};
use futures::{Stream, StreamExt};
use http::ArpyRequest;
use hyper::HeaderMap;
use serde::Serialize;
use websocket::WebSocketHandler;

pub mod http;
mod websocket;

/// Extension trait to add RPC routes. See [module level documentation][crate]
/// for an example.
pub trait RpcRoute {
    /// Add an HTTP route to handle a single RPC endpoint.
    ///
    /// Routes are constructed with `"{prefix}/{msg_id}"` where `msg_id = T::ID`
    /// from [`MsgId`][arpy::protocol::MsgId].
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + 'static;

    /// Add all the RPC endpoints in `router` to a websocket endpoint at `path`.
    fn ws_rpc_route(self, path: &str, router: WebSocketRouter, max_in_flight: usize) -> Self;

    /// Add a Server Sent Events route.
    fn sse_route<T, S, Error>(
        self,
        path: &str,
        events: impl FnMut() -> S + Send + Clone + 'static,
        keep_alive: Option<KeepAlive>,
    ) -> Self
    where
        T: Serialize + protocol::MsgId + 'static,
        S: Stream<Item = Result<T, Error>> + Send + 'static,
        Error: Into<BoxError> + 'static;
}

impl RpcRoute for Router {
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + 'static,
    {
        let id = T::ID;
        let f = Arc::new(f);
        self.route(
            &format!("{prefix}/{id}",),
            post(move |headers: HeaderMap, arpy: ArpyRequest<T>| http::handler(headers, arpy, f)),
        )
    }

    fn ws_rpc_route(self, path: &str, router: WebSocketRouter, max_in_flight: usize) -> Self {
        let handler = WebSocketHandler::new(router, max_in_flight);

        self.route(
            path,
            get(|ws: WebSocketUpgrade| async {
                ws.on_upgrade(
                    |socket: WebSocket| async move { handler.handle_socket(socket).await },
                )
            }),
        )
    }

    /// Add an SSE route using [`sse_handler`].
    fn sse_route<T, S, Error>(
        self,
        path: &str,
        mut events: impl FnMut() -> S + Send + Clone + 'static,
        keep_alive: Option<KeepAlive>,
    ) -> Self
    where
        T: Serialize + protocol::MsgId + 'static,
        S: Stream<Item = Result<T, Error>> + Send + 'static,
        Error: Into<BoxError> + 'static,
    {
        self.route(
            path,
            get(|| async move {
                let sse = sse_handler(events()).await;

                if let Some(keep_alive) = keep_alive {
                    sse.keep_alive(keep_alive)
                } else {
                    sse
                }
            }),
        )
    }
}

/// SSE Handler.
///
/// This uses `serde_json` to serialize data, and assumes it can't fail. See
/// [`serde_json::to_writer`] for more details.
pub async fn sse_handler<T: Serialize + protocol::MsgId, Error: Into<BoxError>>(
    events: impl Stream<Item = Result<T, Error>> + Send + 'static,
) -> Sse<impl Stream<Item = Result<Event, Error>>> {
    Sse::new(
        events.map(|item| item.map(|item| Event::default().event(T::ID).json_data(item).unwrap())),
    )
}
