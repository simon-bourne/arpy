//! Websocket Client.
//!
//! See [`Connection`] for an example.
use std::{
    cell::RefCell,
    future::Future,
    io::{Read, Write},
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
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
pub struct Connection {
    sender: mpsc::Sender<SendMsg>,
    msg_ids: ClientIdMap<oneshot::Sender<ReceiveMsgOrError>>,
    subscription_ids: ClientIdMap<mpsc::Sender<ReceiveMsgOrError>>,
}

impl Connection {
    /// Constructor.
    pub fn new(ws: WebSocket) -> Self {
        // TODO: Benchmark and see if make capacity > 1 improves perf.
        // This is to send messages to the websocket. We want this to block when we
        // can't send to the websocket, hence the small capacity.
        let (sender, to_send) = mpsc::channel::<SendMsg>(1);
        let to_send = ReceiverStream::new(to_send);
        let msg_ids = Rc::new(RefCell::new(SlotMap::new()));
        let subscription_ids = Rc::new(RefCell::new(SlotMap::new()));
        let bg_ws = BackgroundWebsocket {
            msg_ids: msg_ids.clone(),
            subscription_ids: subscription_ids.clone(),
        };

        spawn_local(async move { bg_ws.run(ws, to_send).await });

        Self {
            sender,
            msg_ids,
            subscription_ids,
        }
    }

    fn serialize_msg<M>(client_id: DefaultKey, msg: M) -> Vec<u8>
    where
        M: protocol::MsgId + Serialize,
    {
        let mut msg_bytes = Vec::new();
        serialize(&mut msg_bytes, &protocol::VERSION);
        serialize(&mut msg_bytes, M::ID.as_bytes());
        serialize(&mut msg_bytes, &client_id);
        serialize(&mut msg_bytes, &msg);

        msg_bytes
    }

    pub async fn close(self) {
        self.sender.send(SendMsg::Close).await.ok();
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
        let client_id = self.msg_ids.borrow_mut().insert(notify);

        self.sender
            .send(SendMsg::Msg(Self::serialize_msg(client_id, function)))
            .await
            .map_err(Error::send)?;

        Ok(Call {
            recv,
            phantom: PhantomData,
        })
    }

    async fn subscribe<S>(
        &self,
        service: S,
        updates: impl Stream<Item = S::Update> + 'static,
    ) -> Result<SubscriptionStream<S::Item>, Error>
    where
        S: FnSubscription + 'static,
    {
        // TODO: Benchmark and adjust size.
        // We use a small channel buffer as this is just to get messages to the
        // websocket handler task.
        let (subscription_sink, subscription_stream) = mpsc::channel(1);

        // TODO: Cleanup `subscription_ids`
        let client_id = self.subscription_ids.borrow_mut().insert(subscription_sink);

        self.sender
            .send(SendMsg::Msg(Self::serialize_msg(client_id, service)))
            .await
            .map_err(Error::send)?;

        let mut subscription_stream = ReceiverStream::new(subscription_stream);

        subscription_stream
            .next()
            .await
            .ok_or_else(|| Error::receive("Couldn't receive subscription confirmation"))??;

        let sender = self.sender.clone();

        spawn_local(async move {
            let mut updates = Box::pin(updates);

            while let Some(update) = updates.next().await {
                let mut msg_bytes = Vec::new();
                serialize(&mut msg_bytes, &protocol::VERSION);
                serialize(&mut msg_bytes, S::ID.as_bytes());
                serialize(&mut msg_bytes, &client_id);
                serialize(&mut msg_bytes, &update);

                let result = sender
                    .send(SendMsg::Msg(msg_bytes))
                    .await
                    .map_err(Error::send);

                // TODO: Error handling?
                result.unwrap();
            }
        });

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
    msg_ids: ClientIdMap<oneshot::Sender<ReceiveMsgOrError>>,
    subscription_ids: ClientIdMap<mpsc::Sender<ReceiveMsgOrError>>,
}

type ClientIdMap<T> = Rc<RefCell<SlotMap<DefaultKey, T>>>;

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
        for (_id, notifier) in self.msg_ids.take() {
            notifier.send(Err(err.clone())).ok();
        }

        for (_id, notifier) in self.subscription_ids.take() {
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

                let notifier = self.msg_ids.borrow_mut().remove(id);

                if let Some(notifier) = notifier {
                    notifier
                        .send(Ok(ReceiveMsg {
                            payload_offset,
                            message,
                        }))
                        .map_err(|_| Error::receive("Unable to send message to client"))?;
                    return Ok(());
                }

                let subscription = self.subscription_ids.borrow().get(id).cloned();

                if let Some(subscription) = subscription {
                    subscription
                        .send(Ok(ReceiveMsg {
                            payload_offset,
                            message,
                        }))
                        .await
                        .map_err(|_| {
                            Error::receive("Unable to send subscription message to client")
                        })?;
                    return Ok(());
                }
            }
        }

        Err(Error::deserialize_result("Unknown message id"))
    }

    async fn send<MessageSink>(&mut self, ws: &mut MessageSink, msg: SendMsg) -> Result<(), Error>
    where
        MessageSink: Sink<Message, Error = WebSocketError> + Unpin,
    {
        match msg {
            SendMsg::Close => ws.close().await,
            SendMsg::Msg(msg) => ws.send(Message::Bytes(msg)).await,
        }
        .map_err(Error::send)
    }
}

enum WsTask {
    Incoming(Result<Message, WebSocketError>),
    ToSend(SendMsg),
}

enum SendMsg {
    Close,
    Msg(Vec<u8>),
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
