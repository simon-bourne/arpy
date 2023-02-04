use std::{future::ready, ops::ControlFlow, sync::Arc};

use arpy_server::WebSocketRouter;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};

#[derive(Clone)]
pub struct WebSocketHandler(Arc<arpy_server::WebSocketHandler>);

impl WebSocketHandler {
    pub fn new(router: WebSocketRouter) -> Self {
        Self(arpy_server::WebSocketHandler::new(router))
    }

    pub async fn handle_socket(&self, socket: WebSocket) {
        let (socket_sink, socket_stream) = socket.split();
        let incoming = socket_stream
            .filter_map(Self::handle_incoming)
            .take_while(|msg| ready(msg.is_continue()))
            .filter_map(|msg| {
                ready(match msg {
                    ControlFlow::Continue(msg) => Some(msg),
                    ControlFlow::Break(()) => None,
                })
            });

        if let Err(e) = self
            .0
            .handle_socket(
                socket_sink
                    .with(|msg| ready(Result::<Message, axum::Error>::Ok(Message::Binary(msg)))),
                incoming,
            )
            .await
        {
            tracing::error!("Error on WebSocket: {e}");
        }
    }

    async fn handle_incoming(
        msg: Result<Message, axum::Error>,
    ) -> Option<ControlFlow<(), Vec<u8>>> {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!("Error on WebSocket: {e}");
                return Some(ControlFlow::Break(()));
            }
        };

        match msg {
            Message::Text(_) => {
                tracing::error!("Text message type is unsupported");
                Some(ControlFlow::Break(()))
            }
            Message::Binary(params) => Some(ControlFlow::Continue(params)),
            Message::Ping(_) => None,
            Message::Pong(_) => None,
            Message::Close(_) => Some(ControlFlow::Break(())),
        }
    }
}
