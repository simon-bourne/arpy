use std::sync::Arc;

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

    async fn try_handle_socket(&self, mut socket: WebSocket) -> anyhow::Result<()> {
        let (send, mut recv) = mpsc::channel(1000);

        loop {
            select! {
                incoming = socket.recv() => {
                    let Some(incoming) = incoming else { break; };
                    self.handle_incoming(incoming?, &send)?;
                },
                outgoing = recv.recv() => {
                    // If we're receiving bad messages, we want to abort and close the websocket.
                    let outgoing = outgoing.unwrap()?;
                    socket.send(Message::Binary(outgoing)).await?;
                }
            }
        }

        Ok(())
    }

    fn handle_incoming(
        &self,
        msg: Message,
        send: &mpsc::Sender<websocket::Result<Vec<u8>>>,
    ) -> anyhow::Result<()> {
        match msg {
            Message::Text(_) => bail!("Text message type is unsupported"),
            Message::Binary(params) => {
                let send = send.clone();
                let handler = self.0.clone();
                spawn(async move { send.send(handler.handle_msg(&params).await).await.unwrap() });
            }
            Message::Ping(_) => (),
            Message::Pong(_) => (),
            Message::Close(_) => return Ok(()),
        }

        Ok(())
    }
}
