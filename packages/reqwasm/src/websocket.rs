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

        self.write.send(Message::Bytes(body)).await.unwrap(); // TODO:
        let result = self.read.next().await.unwrap().unwrap();

        let result: F::ResultType = match result {
            Message::Text(_) => todo!(),
            Message::Bytes(bytes) => ciborium::de::from_reader(bytes.as_slice()).unwrap(),
        };

        Ok(result)
    }
}
