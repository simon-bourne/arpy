use std::sync::Arc;

use anyhow::bail;
use arpy_server::WebSocketRouter;
use axum::extract::ws::{Message, WebSocket};

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
        while let Some(id) = Self::next_msg(&mut socket).await? {
            let Some(params) = Self::next_msg(&mut socket).await?
            else { bail!("Expected params message")};

            let output = self.0.handle_msg(&id, &params).await?;
            socket.send(Message::Binary(output)).await?;
        }

        Ok(())
    }

    async fn next_msg(socket: &mut WebSocket) -> anyhow::Result<Option<Vec<u8>>> {
        while let Some(msg) = socket.recv().await {
            match msg? {
                Message::Text(_) => bail!("Text message type is unsupported"),
                Message::Binary(bytes) => return Ok(Some(bytes)),
                Message::Ping(_) => (),
                Message::Pong(_) => (),
                Message::Close(_) => return Ok(None),
            }
        }

        Ok(None)
    }
}
