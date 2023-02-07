//! Websocket Client.
//!
//! See [`Connection`] for an example.
use std::{
    future::Future,
    io::{Read, Write},
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
use tokio_stream::wrappers::ReceiverStream;
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
pub struct Connection(mpsc::Sender<SendMsg>);

impl Connection {
    /// Constructor.
    pub fn new(ws: WebSocket) -> Self {
        // TODO: Benchmark and see if make capacity > 1 improves perf.
        // This is to send messages to the websocket. We want this to block when we
        // can't send to the websocket, hence the small capacity.
        let (send, to_send) = mpsc::channel::<SendMsg>(1);
        let to_send = ReceiverStream::new(to_send);
        let bg_ws = BackgroundWebsocket {
            msg_ids: SlotMap::new(),
            subscription_ids: SlotMap::new(),
        };

        spawn_local(async move { bg_ws.run(ws, to_send).await });

        Self(send)
    }

    fn serialize_msg<M>(&self, msg: M) -> Vec<u8>
    where
        M: protocol::MsgId + Serialize,
    {
        let mut msg_bytes = Vec::new();
        serialize(&mut msg_bytes, &protocol::VERSION);
        serialize(&mut msg_bytes, M::ID.as_bytes());
        serialize(&mut msg_bytes, &msg);

        msg_bytes
    }

    pub async fn close(self) {
        self.0.send(SendMsg::Close).await.unwrap_or(());
    }
}

#[pin_project]
pub struct SubscriptionStream<Item> {
    #[pin]
    stream: ReceiverStream<ReceiveMsgOrError>,
    phantom: PhantomData<Item>,
}

impl<Item: DeserializeOwned> Stream for SubscriptionStream<Item> {
    type Item = Result<Item, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx).map(|msg| {
            msg.map(|msg| {
                let msg = msg?;
                deserialize(&msg.message[msg.payload_offset..])
            })
        })
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
        let (notify, recv) = oneshot::channel();

        self.0
            .send(SendMsg::Msg {
                msg: self.serialize_msg(function),
                notify,
            })
            .await
            .map_err(Error::send)?;

        Ok(Call {
            recv,
            phantom: PhantomData,
        })
    }

    async fn subscribe<S>(&self, service: S) -> Result<SubscriptionStream<S::Item>, Error>
    where
        S: FnSubscription,
    {
        // TODO: Benchmark and adjust size.
        // We use a small channel buffer as this is just to get messages to the
        // websocket handler task.
        let (subscription_sink, subscription_stream) = mpsc::channel(1);

        self.0
            .send(SendMsg::Subscribe {
                msg: self.serialize_msg(service),
                subscription: subscription_sink,
            })
            .await
            .map_err(Error::send)?;

        let mut subscription_stream = ReceiverStream::new(subscription_stream);

        // Discard the first message. It's the reply to the subscription and will
        // eventually contain the cancellation ID.
        subscription_stream.next().await;

        Ok(SubscriptionStream {
            stream: subscription_stream,
            phantom: PhantomData,
        })
    }
}

#[pin_project]
pub struct Call<Output> {
    #[pin]
    recv: oneshot::Receiver<ReceiveMsgOrError>,
    phantom: PhantomData<Output>,
}

impl<Output: DeserializeOwned> Future for Call<Output> {
    type Output = Result<Output, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().recv.poll(cx).map(|reply| {
            let reply = reply.map_err(Error::receive)??;
            deserialize(&reply.message[reply.payload_offset..])
        })
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
    msg_ids: SlotMap<DefaultKey, oneshot::Sender<ReceiveMsgOrError>>,
    subscription_ids: SlotMap<DefaultKey, mpsc::Sender<ReceiveMsgOrError>>,
}

impl BackgroundWebsocket {
    async fn run(mut self, ws: WebSocket, to_send: ReceiverStream<SendMsg>) {
        let (mut ws_sink, ws_stream) = ws.split();

        let mut ws_task_stream =
            stream_select!(ws_stream.map(WsTask::Incoming), to_send.map(WsTask::ToSend));

        while let Some(task) = ws_task_stream.next().await {
            let result = match task {
                WsTask::Incoming(incoming) => self.receive(incoming).await,
                WsTask::ToSend(outgoing) => self.send(&mut ws_sink, outgoing).await,
            };

            if let Err(err) = result {
                self.send_errors(err).await;
                break;
            }
        }
    }

    async fn send_errors(self, err: Error) {
        for (_id, notifier) in self.msg_ids {
            notifier.send(Err(err.clone())).ok();
        }

        for (_id, notifier) in self.subscription_ids {
            notifier.send(Err(err.clone())).await.ok();
        }
    }

    async fn receive(&mut self, msg: Result<Message, WebSocketError>) -> Result<(), Error> {
        match msg.map_err(Error::receive)? {
            Message::Text(_) => return Err(Error::receive("Text messages are unsupported")),
            Message::Bytes(message) => {
                let mut reader = message.as_slice();

                let protocol_version: usize = deserialize_part(&mut reader)?;

                if protocol_version != protocol::VERSION {
                    return Err(Error::receive(format!(
                        "Unknown protocol version. Expected {}, got {}.",
                        protocol::VERSION,
                        protocol_version
                    )));
                }

                let id: DefaultKey = deserialize_part(&mut reader)?;
                let payload_offset = message.len() - reader.len();

                if let Some(notifier) = self.msg_ids.remove(id) {
                    notifier
                        .send(Ok(ReceiveMsg {
                            payload_offset,
                            message,
                        }))
                        .map_err(|_| Error::receive("Unable to send message to client"))?;
                } else if let Some(subscription) = self.subscription_ids.get(id) {
                    subscription
                        .send(Ok(ReceiveMsg {
                            payload_offset,
                            message,
                        }))
                        .await
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
            SendMsg::Close => ws.close().await,
            SendMsg::Msg { mut msg, notify } => {
                let key = self.msg_ids.insert(notify);
                serialize(&mut msg, &key);
                ws.send(Message::Bytes(msg)).await
            }
            SendMsg::Subscribe {
                mut msg,
                subscription,
            } => {
                let key = self.subscription_ids.insert(subscription);
                serialize(&mut msg, &key);
                ws.send(Message::Bytes(msg)).await
            }
        }
        .map_err(Error::send)?;

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
        notify: oneshot::Sender<ReceiveMsgOrError>,
    },
    Subscribe {
        msg: Vec<u8>,
        subscription: mpsc::Sender<ReceiveMsgOrError>,
    },
}

struct ReceiveMsg {
    payload_offset: usize,
    message: Vec<u8>,
}

type ReceiveMsgOrError = Result<ReceiveMsg, Error>;

fn serialize<W, T>(writer: W, t: &T)
where
    W: Write,
    T: Serialize + ?Sized,
{
    bincode::DefaultOptions::new()
        .serialize_into(writer, t)
        .unwrap()
}

fn deserialize<R, T>(reader: R) -> Result<T, Error>
where
    R: Read,
    T: DeserializeOwned,
{
    bincode::DefaultOptions::new()
        .deserialize_from(reader)
        .map_err(Error::deserialize_result)
}

fn deserialize_part<R, T>(reader: R) -> Result<T, Error>
where
    R: Read,
    T: DeserializeOwned,
{
    bincode::DefaultOptions::new()
        .allow_trailing_bytes()
        .deserialize_from(reader)
        .map_err(Error::deserialize_result)
}
