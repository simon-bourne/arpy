//! Websocket Client.
//!
//! See [`Connection`] for an example.
use arpy::{ConcurrentRpcClient, FnRemote, FnSubscription, RpcClient};
use arpy_client::websocket::{Call, SubscriptionStream};
use async_trait::async_trait;
use futures::{SinkExt, Stream, StreamExt, TryStreamExt};
use reqwasm::websocket::{futures::WebSocket, Message};
use serde::de::DeserializeOwned;

use crate::{Error, LocalSpawner};

/// A connection to the server.
///
/// # Example
///
/// ```
#[doc = include_doc::function_body!("tests/doc.rs", websocket_client, [my_app, MyAdd])]
/// ```
#[derive(Clone)]
pub struct Connection(arpy_client::websocket::Connection<LocalSpawner>);

impl Connection {
    /// Constructor.
    pub fn new(ws: WebSocket) -> Self {
        let (ws_sink, ws_stream) = ws.split();
        let ws_sink = ws_sink
            .sink_map_err(Error::send)
            .with(|msg| async { Ok(Message::Bytes(msg)) });
        let ws_stream = ws_stream.map_err(Error::receive).map(|msg| match msg? {
            Message::Text(_) => Err(Error::receive("Text messages are unsupported")),
            Message::Bytes(message) => Ok(message),
        });

        Self(arpy_client::websocket::Connection::new(
            LocalSpawner,
            ws_sink,
            ws_stream,
        ))
    }

    pub async fn close(self) {
        self.0.close().await
    }
}

#[async_trait(?Send)]
impl ConcurrentRpcClient for Connection {
    type Call<Output: DeserializeOwned> = Call<Output>;
    type Error = Error;
    type SubscriptionStream<Item: DeserializeOwned> = SubscriptionStream<Item>;

    async fn begin_call<F>(&self, function: F) -> Result<Self::Call<F::Output>, Self::Error>
    where
        F: FnRemote,
    {
        self.0.begin_call(function).await
    }

    async fn subscribe<S>(
        &self,
        service: S,
        updates: impl Stream<Item = S::Update> + 'static,
    ) -> Result<SubscriptionStream<S::Item>, Error>
    where
        S: FnSubscription + 'static,
    {
        self.0.subscribe(service, updates).await
    }
}

#[async_trait(?Send)]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<Args>(&self, args: Args) -> Result<Args::Output, Self::Error>
    where
        Args: FnRemote,
    {
        self.0.call(args).await
    }
}
