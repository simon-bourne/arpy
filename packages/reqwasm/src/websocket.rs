//! Websocket Client.
//!
//! See [`Connection`] for an example.
use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use arpy::{protocol, ConcurrentRpcClient, FnRemote, FnSubscription, RpcClient};
use async_trait::async_trait;
use bincode::Options;
use futures::{stream_select, Sink, SinkExt, Stream, StreamExt};
use pin_project::pin_project;
use reqwasm::websocket::{futures::WebSocket, Message, WebSocketError};
use serde::{de::DeserializeOwned, Serialize};
use slotmap::{DefaultKey, SlotMap};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::UnboundedReceiverStream;
use wasm_bindgen_futures::spawn_local;

use crate::Error;

/// A connection to the server.
///
/// # Example
///
/// ```
#[doc = include_doc::function_body!("tests/doc.rs", websocket_client, [my_app, MyAdd])]
/// ```
#[derive(Clone)]
pub struct Connection(mpsc::UnboundedSender<SendMsg>);

impl Connection {
    /// Constructor.
    pub fn new(ws: WebSocket) -> Self {
        // We use an `unbounded_channel` because bounding could cause deadlocks or lost
        // messages (depending on the implementaiton). The queue will be no larger than
        // the number of in-flight calls.
        let (send, to_send) = mpsc::unbounded_channel::<SendMsg>();
        let to_send = UnboundedReceiverStream::new(to_send);
        let bg_ws = BackgroundWebsocket {
            msg_ids: SlotMap::new(),
            subscription_ids: SlotMap::new(),
        };

        spawn_local(async move {
            // TODO: We probably want to report this error, or just ignore it if it's a
            // close
            bg_ws.run(ws, to_send).await.unwrap()
        });

        Self(send)
    }

    pub async fn subscribe<S>(&self, service: S) -> Result<SubscriptionStream<S::Item>, Error>
    where
        S: FnSubscription,
    {
        let (subscription, recv) = mpsc::unbounded_channel();

        self.0
            .send(SendMsg::Subscribe {
                msg: self.serialize_msg(service),
                subscription,
            })
            .map_err(Error::send)?;

        let mut recv = UnboundedReceiverStream::new(recv);

        // Discard the first message. It's the reply to the subscription and will
        // eventually contain the cancellation ID.
        recv.next().await;

        Ok(SubscriptionStream {
            stream: recv,
            phantom: PhantomData,
        })
    }

    fn serialize_msg<M>(&self, msg: M) -> Vec<u8>
    where
        M: protocol::MsgId + Serialize,
    {
        let mut msg_bytes = Vec::new();
        let serializer = bincode::DefaultOptions::new();
        serializer
            .serialize_into(&mut msg_bytes, &protocol::VERSION)
            .unwrap();
        serializer
            .serialize_into(&mut msg_bytes, M::ID.as_bytes())
            .unwrap();
        serializer.serialize_into(&mut msg_bytes, &msg).unwrap();

        msg_bytes
    }

    pub async fn close(self) {
        self.0.send(SendMsg::Close).unwrap_or(());
    }
}

#[pin_project]
pub struct SubscriptionStream<Item> {
    #[pin]
    stream: UnboundedReceiverStream<ReceiveMsg>,
    phantom: PhantomData<Item>,
}

impl<Item: DeserializeOwned> Stream for SubscriptionStream<Item> {
    type Item = Result<Item, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().stream.poll_next(cx) {
            Poll::Ready(msg) => Poll::Ready(msg.map(|msg| {
                let serializer = bincode::DefaultOptions::new();
                serializer
                    .deserialize_from(&msg.message[msg.payload_offset..])
                    .map_err(Error::deserialize_result)
            })),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[async_trait(?Send)]
impl ConcurrentRpcClient for Connection {
    type Call<Output: DeserializeOwned> = Call<Output>;
    type Error = Error;

    // TODO: `fn send` to send a fire and forget message
    async fn begin_call<F>(&self, function: F) -> Result<Self::Call<F::Output>, Self::Error>
    where
        F: FnRemote,
    {
        let (notify, recv) = oneshot::channel();

        self.0
            .send(SendMsg::Msg {
                msg: self.serialize_msg(function),
                notify,
            })
            .map_err(Error::send)?;

        Ok(Call {
            recv,
            phantom: PhantomData,
        })
    }
}

#[pin_project]
pub struct Call<Output> {
    #[pin]
    recv: oneshot::Receiver<ReceiveMsg>,
    phantom: PhantomData<Output>,
}

impl<Output: DeserializeOwned> Future for Call<Output> {
    type Output = Result<Output, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().recv.poll(cx) {
            Poll::Ready(reply) => {
                let reply = reply.map_err(Error::receive)?;
                let serializer = bincode::DefaultOptions::new();
                Poll::Ready(
                    serializer
                        .deserialize_from(&reply.message[reply.payload_offset..])
                        .map_err(Error::deserialize_result),
                )
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[async_trait(?Send)]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<Args>(&self, args: Args) -> Result<Args::Output, Self::Error>
    where
        Args: FnRemote,
    {
        self.begin_call(args).await?.await
    }
}

struct BackgroundWebsocket {
    msg_ids: SlotMap<DefaultKey, oneshot::Sender<ReceiveMsg>>,
    subscription_ids: SlotMap<DefaultKey, mpsc::UnboundedSender<ReceiveMsg>>,
}

impl BackgroundWebsocket {
    async fn run(
        mut self,
        ws: WebSocket,
        to_send: UnboundedReceiverStream<SendMsg>,
    ) -> Result<(), Error> {
        let (mut ws_sink, ws_stream) = ws.split();

        let mut ws_task_stream =
            stream_select!(ws_stream.map(WsTask::Incoming), to_send.map(WsTask::ToSend));

        while let Some(task) = ws_task_stream.next().await {
            match task {
                WsTask::Incoming(incoming) => self.receive(incoming).await?,
                WsTask::ToSend(outgoing) => self.send(&mut ws_sink, outgoing).await?,
            }
        }

        // TODO: Send errors to everything
        // TODO: Send errors to any new messages

        Ok(())
    }

    async fn receive(&mut self, msg: Result<Message, WebSocketError>) -> Result<(), Error> {
        match msg.map_err(Error::receive)? {
            Message::Text(_) => return Err(Error::receive("Text messages are unsupported")),
            Message::Bytes(message) => {
                let serializer = bincode::DefaultOptions::new().allow_trailing_bytes();
                let mut reader = message.as_slice();

                let protocol_version: usize = serializer
                    .deserialize_from(&mut reader)
                    .map_err(Error::deserialize_result)?;

                if protocol_version != protocol::VERSION {
                    return Err(Error::receive(format!(
                        "Unknown protocol version. Expected {}, got {}.",
                        protocol::VERSION,
                        protocol_version
                    )));
                }

                let id: DefaultKey = serializer
                    .deserialize_from(&mut reader)
                    .map_err(Error::deserialize_result)?;
                let payload_offset = message.len() - reader.len();

                if let Some(notifier) = self.msg_ids.remove(id) {
                    notifier
                        .send(ReceiveMsg {
                            payload_offset,
                            message,
                        })
                        .map_err(|_| Error::receive("Unable to send message to client"))?;
                } else if let Some(subscription) = self.subscription_ids.get(id) {
                    subscription
                        .send(ReceiveMsg {
                            payload_offset,
                            message,
                        })
                        .map_err(|_| {
                            Error::receive("Unable to send subscription message to client")
                        })?;
                } else {
                    return Err(Error::deserialize_result("Unknown message id"));
                };
            }
        }

        Ok(())
    }

    async fn send<MessageSink>(&mut self, ws: &mut MessageSink, msg: SendMsg) -> Result<(), Error>
    where
        MessageSink: Sink<Message, Error = WebSocketError> + Unpin,
    {
        match msg {
            SendMsg::Close => todo!(),
            SendMsg::Msg { mut msg, notify } => {
                let serializer = bincode::DefaultOptions::new();
                let key = self.msg_ids.insert(notify);
                serializer.serialize_into(&mut msg, &key).unwrap();
                ws.send(Message::Bytes(msg)).await.map_err(Error::send)?;
            }
            SendMsg::Subscribe {
                mut msg,
                subscription,
            } => {
                let serializer = bincode::DefaultOptions::new();
                let key = self.subscription_ids.insert(subscription);
                serializer.serialize_into(&mut msg, &key).unwrap();
                ws.send(Message::Bytes(msg)).await.map_err(Error::send)?;
            }
        }

        Ok(())
    }
}

enum WsTask {
    Incoming(Result<Message, WebSocketError>),
    ToSend(SendMsg),
}

enum SendMsg {
    Close,
    Msg {
        msg: Vec<u8>,
        notify: oneshot::Sender<ReceiveMsg>,
    },
    Subscribe {
        msg: Vec<u8>,
        // TODO: This probably should be bounded.
        subscription: mpsc::UnboundedSender<ReceiveMsg>,
    },
}

struct ReceiveMsg {
    payload_offset: usize,
    message: Vec<u8>,
}
