use std::{ops::ControlFlow, sync::Arc};

use anyhow::bail;
use arpy_server::{websocket, WebSocketRouter};
use axum::extract::ws::{Message, WebSocket};
use tokio::{select, spawn, sync::mpsc};

#[derive(Clone)]
pub struct WebSocketHandler(Arc<arpy_server::WebSocketHandler>);

impl WebSocketHandler {
    pub fn new(router: WebSocketRouter) -> Self {
        Self(Arc::new(arpy_server::WebSocketHandler::new(router)))
    }

    pub async fn handle_socket(&self, socket: WebSocket) {
        if let Err(e) = self.try_handle_socket(socket).await {
            tracing::error!("Error on WebSocket: {e}");
        }
    }

    // TODO: Factor out `try_handle_socket`
    async fn try_handle_socket(&self, mut socket: WebSocket) -> anyhow::Result<()> {
        let (send, mut recv) = mpsc::unbounded_channel();

        loop {
            // TODO: Check a semaphore to see if we can handle more incoming messages,
            // otherwise just `recv.recv().await ... `
            select! {
                incoming = socket.recv() => {
                    if self.handle_incoming(incoming, &send)?.is_break() {
                        break;
                    }
                },
                outgoing = recv.recv() => Self::handle_outgoing(&mut socket, outgoing).await?
            }
        }

        Ok(())
    }

    async fn handle_outgoing(
        socket: &mut WebSocket,
        outgoing: Option<websocket::Result<Vec<u8>>>,
    ) -> anyhow::Result<()> {
        let outgoing = outgoing.unwrap();

        // If we're receiving bad messages, we want to abort and close the websocket.
        let outgoing = outgoing?;
        socket.send(Message::Binary(outgoing)).await?;

        Ok(())
    }

    fn handle_incoming(
        &self,
        msg: Option<Result<Message, axum::Error>>,
        send: &mpsc::UnboundedSender<websocket::Result<Vec<u8>>>,
    ) -> anyhow::Result<ControlFlow<(), ()>> {
        let Some(msg) = msg else { return Ok(ControlFlow::Break(())) };

        match msg? {
            Message::Text(_) => bail!("Text message type is unsupported"),
            Message::Binary(params) => {
                let send = send.clone();
                let handler = self.0.clone();
                spawn(async move { send.send(handler.handle_msg(&params).await).unwrap() });
            }
            Message::Ping(_) => (),
            Message::Pong(_) => (),
            Message::Close(_) => return Ok(ControlFlow::Break(())),
        }

        Ok(ControlFlow::Continue(()))
    }
}
