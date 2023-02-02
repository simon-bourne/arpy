//! Websocket Client.
//!
//! See [`Connection`] for an example.
use arpy::{FnRemote, RpcClient};
use async_trait::async_trait;
use bincode::Options;
use futures::{SinkExt, StreamExt};
use reqwasm::websocket::{futures::WebSocket, Message};

use crate::Error;

/// A connection to the server.
///
/// # Example
///
/// ```
#[doc = include_doc::function_body!("tests/doc.rs", websocket_client, [my_app, MyAdd])]
/// ```
pub struct Connection(WebSocket);

impl Connection {
    /// Constructor.
    pub fn new(ws: WebSocket) -> Self {
        Self(ws)
    }
}

#[async_trait(?Send)]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<Args>(&mut self, args: Args) -> Result<Args::Output, Self::Error>
    where
        Args: FnRemote,
    {
        let mut body = Vec::new();
        let serializer = bincode::DefaultOptions::new();
        serializer
            .serialize_into(&mut body, Args::ID.as_bytes())
            .unwrap();
        serializer.serialize_into(&mut body, &args).unwrap();

        self.0
            .send(Message::Bytes(body))
            .await
            .map_err(|e| Error::Send(e.to_string()))?;

        let result = if let Some(result) = self.0.next().await {
            result.map_err(Error::receive)?
        } else {
            Err(Error::receive("End of stream"))?
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
