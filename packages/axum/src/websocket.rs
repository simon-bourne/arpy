use std::{collections::HashMap, sync::Arc};

use anyhow::bail;
use arpy::FnRemote;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    routing::{get, MethodRouter},
};
use futures::future::BoxFuture;

#[derive(Default)]
pub struct WebSocketRouter(HashMap<Id, RpcHandler>);

impl WebSocketRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle<T>(mut self) -> Self
    where
        T: FnRemote + Send + Sync + 'static,
    {
        let id = T::ID.as_bytes().to_vec();
        self.0
            .insert(id, Box::new(|body| Box::pin(Self::run::<T>(body))));

        self
    }

    async fn run<T>(input: Vec<u8>) -> anyhow::Result<Vec<u8>>
    where
        T: FnRemote,
    {
        let function: T = ciborium::de::from_reader(input.as_slice())?;
        let result = function.run().await;
        let mut body = Vec::new();
        ciborium::ser::into_writer(&result, &mut body).unwrap();
        Ok(body)
    }
}

#[derive(Clone)]
struct WebSocketHandler(Arc<HashMap<Id, RpcHandler>>);

impl WebSocketHandler {
    async fn handle_socket(&self, socket: WebSocket) {
        if let Err(e) = self.try_handle_socket(socket).await {
            tracing::error!("Error on WebSocket: {e}");
        }
    }

    async fn try_handle_socket(&self, mut socket: WebSocket) -> anyhow::Result<()> {
        while let Some(id) = Self::next_msg(&mut socket).await? {
            let Some(function) = self.0.get(&id)
            else { bail!("Function not found") };

            let Some(params) = Self::next_msg(&mut socket).await?
            else { bail!("Expected params message")};

            let output = function(params).await?;
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

impl From<WebSocketRouter> for MethodRouter {
    fn from(value: WebSocketRouter) -> Self {
        let value = WebSocketHandler(Arc::new(value.0));

        get(|ws: WebSocketUpgrade| async {
            ws.on_upgrade(|socket: WebSocket| async move { value.handle_socket(socket).await })
        })
    }
}

type Id = Vec<u8>;
type RpcHandler = Box<dyn Fn(Vec<u8>) -> BoxFuture<'static, anyhow::Result<Vec<u8>>> + Send + Sync>;
