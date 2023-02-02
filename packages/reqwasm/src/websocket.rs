//! Websocket Client.
//!
//! See [`Connection`] for an example.
use arpy::{FnRemote, RpcClient};
use async_trait::async_trait;
use bincode::Options;
use futures::{SinkExt, StreamExt};
use reqwasm::websocket::{futures::WebSocket, Message};
use tokio::sync::Mutex;

use crate::Error;

/// A connection to the server.
///
/// # Example
///
/// ```
#[doc = include_doc::function_body!("tests/doc.rs", websocket_client, [my_app, MyAdd])]
/// ```
pub struct Connection(Mutex<WebSocket>);

impl Connection {
    /// Constructor.
    ///
    /// It's safe to make concurrent RPC calls, but only one can make progress
    /// at a time. Internally it will lock a [`tokio::sync::Mutex`] while a call
    /// is in flight.
    pub fn new(ws: WebSocket) -> Self {
        Self(Mutex::new(ws))
    }
}

#[async_trait(?Send)]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<Args>(&self, args: Args) -> Result<Args::Output, Self::Error>
    where
        Args: FnRemote,
    {
        let mut body = Vec::new();
        let serializer = bincode::DefaultOptions::new();
        serializer
            .serialize_into(&mut body, Args::ID.as_bytes())
            .unwrap();
        serializer.serialize_into(&mut body, &args).unwrap();

        let result = {
            let mut ws = self.0.lock().await;
            ws.send(Message::Bytes(body))
                .await
                .map_err(|e| Error::Send(e.to_string()))?;

            if let Some(result) = ws.next().await {
                result.map_err(Error::receive)?
            } else {
                Err(Error::receive("End of stream"))?
            }
        };

        let result: Args::Output = match result {
            Message::Text(_) => Err(Error::deserialize_result("Unexpected text result"))?,
            Message::Bytes(bytes) => serializer
                .deserialize_from(bytes.as_slice())
                .map_err(Error::deserialize_result)?,
        };

        Ok(result)
    }
}
