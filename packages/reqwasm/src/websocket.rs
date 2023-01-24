use async_trait::async_trait;
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use reqwasm::websocket::{futures::WebSocket, Message};
use rpc::{RemoteFn, RpcClient};

use crate::Error;

pub struct Connection {
    write: SplitSink<WebSocket, Message>,
    read: SplitStream<WebSocket>,
}

impl Connection {
    pub fn new(ws: WebSocket) -> Self {
        let (write, read) = ws.split();
        Self { write, read }
    }
}

#[async_trait(?Send)]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<F>(&mut self, function: &F) -> Result<F::ResultType, Self::Error>
    where
        F: RemoteFn,
    {
        let mut body = Vec::new();
        ciborium::ser::into_writer(&function, &mut body).unwrap();

        self.write
            .send(Message::Bytes(body))
            .await
            .map_err(|e| Error::Send(e.to_string()))?;

        let result = if let Some(result) = self.read.next().await {
            result.map_err(Error::receive)?
        } else {
            Err(Error::receive("End of stream"))?
        };

        let result: F::ResultType = match result {
            Message::Text(_) => Err(Error::deserialize_result("Unexpected text result"))?,
            Message::Bytes(bytes) => {
                ciborium::de::from_reader(bytes.as_slice()).map_err(Error::deserialize_result)?
            }
        };

        Ok(result)
    }
}
