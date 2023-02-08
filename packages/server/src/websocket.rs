//! Building blocks to implement an Arpy Websocket server.
//!
//! See the `axum` and `actix` implementations under `packages` in the
//! repository.
use std::{
    collections::HashMap,
    error,
    io::{self, Read, Write},
    mem, result,
    sync::Arc,
};

use arpy::{protocol, FnRemote, FnSubscription};
use bincode::Options;
use futures::{
    channel::mpsc::{self, Sender},
    future::BoxFuture,
    stream_select, Sink, SinkExt, Stream, StreamExt,
};
use serde::{de::DeserializeOwned, Serialize};
use slotmap::DefaultKey;
use thiserror::Error;
use tokio::{
    spawn,
    sync::{OwnedSemaphorePermit, Semaphore},
};

use crate::{FnRemoteBody, FnSubscriptionBody};

/// A collection of RPC calls to be handled by a WebSocket.
#[derive(Default)]
pub struct WebSocketRouter(HashMap<Id, RpcHandler>);

impl WebSocketRouter {
    /// Construct an empty router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a handler for any RPC calls to `FSig`.
    pub fn handle<F, FSig>(mut self, f: F) -> Self
    where
        F: FnRemoteBody<FSig> + Send + Sync + 'static,
        FSig: FnRemote + Send + Sync + 'static,
        FSig::Output: Send + Sync + 'static,
    {
        let id = FSig::ID.as_bytes().to_vec();
        let f = Arc::new(f);
        self.0.insert(
            id,
            Box::new(move |body, result_sink| {
                Box::pin(Self::run(f.clone(), body, result_sink.clone()))
            }),
        );

        self
    }

    /// Add a handler for any subscriptions to `FSig`.
    pub fn handle_subscription<F, FSig>(mut self, f: F) -> Self
    where
        F: FnSubscriptionBody<FSig> + Send + Sync + 'static,
        FSig: FnSubscription + Send + Sync + 'static,
        FSig::Item: Send + Sync + 'static,
    {
        let id = FSig::ID.as_bytes().to_vec();
        let f = Arc::new(f);

        self.0.insert(
            id,
            Box::new(move |body, result_sink| {
                Box::pin(Self::run_subscription(f.clone(), body, result_sink.clone()))
            }),
        );

        self
    }

    fn serialize_msg<Msg: Serialize>(client_id: DefaultKey, msg: &Msg) -> Vec<u8> {
        let mut body = Vec::new();
        serialize(&mut body, &protocol::VERSION);
        serialize(&mut body, &client_id);
        serialize(&mut body, &msg);
        body
    }

    fn deserialize_msg<Msg: DeserializeOwned>(
        mut input: impl io::Read,
    ) -> Result<(DefaultKey, Msg)> {
        let msg: Msg = deserialize_part(&mut input)?;
        let client_id: DefaultKey = deserialize(input)?;
        Ok((client_id, msg))
    }

    async fn run_subscription<F, FSig>(
        f: Arc<F>,
        input: impl io::Read,
        mut result_sink: ResultSink,
    ) -> Result<()>
    where
        F: FnSubscriptionBody<FSig> + Send + Sync + 'static,
        FSig: FnSubscription + Send + Sync + 'static,
        FSig::Item: Send + Sync + 'static,
    {
        let (client_id, args) = Self::deserialize_msg::<FSig>(input)?;

        let mut items = Box::pin(f.run(args));

        let reply = Self::serialize_msg(client_id, &());
        result_sink
            .send(Ok(reply))
            .await
            .unwrap_or_else(client_disconnected);

        spawn(async move {
            // TODO: Cancellation
            while let Some(item) = items.next().await {
                let item_bytes = Self::serialize_msg(client_id, &item);

                if result_sink.send(Ok(item_bytes)).await.is_err() {
                    break;
                }
            }
        });

        Ok(())
    }

    async fn run<F, FSig>(
        f: Arc<F>,
        input: impl io::Read,
        mut result_sink: ResultSink,
    ) -> Result<()>
    where
        F: FnRemoteBody<FSig> + Send + Sync + 'static,
        FSig: FnRemote + Send + Sync + 'static,
    {
        let (client_id, args) = Self::deserialize_msg::<FSig>(input)?;

        let result = f.run(args).await;
        let result_bytes = Self::serialize_msg(client_id, &result);

        result_sink
            .send(Ok(result_bytes))
            .await
            .unwrap_or_else(client_disconnected);

        Ok(())
    }
}

/// Handle raw messages from a websocket.
///
/// Use `WebSocketHandler` to implement a Websocket server.
pub struct WebSocketHandler {
    runners: HashMap<Id, RpcHandler>,
    in_flight: Arc<Semaphore>,
}

impl WebSocketHandler {
    /// Constructor.
    ///
    /// `max_in_flight` limits the number of RPC/Subscription calls that can be
    /// in-flight at once. This stops clients spawning lots of tasks by blocking
    /// the websocket.
    ///
    /// Subscriptions are only considered in-flight until they've sent their
    /// initial response to the client. To limit the active subscriptions, use a
    /// [`Semaphore`] or similar mechanism in the function that generates the
    /// [`Stream`] and hold an [`OwnedSemaphorePermit`] permit for the life
    /// of the stream.
    pub fn new(router: WebSocketRouter, max_in_flight: usize) -> Arc<Self> {
        Arc::new(Self {
            runners: router.0,
            // We use a semaphore so we have a resource limit shared between all connection, but
            // each connection can maintain it's own unbounded queue of in-flight RPC calls.
            in_flight: Arc::new(Semaphore::new(max_in_flight)),
        })
    }

    pub async fn handle_socket<SocketSink, Incoming, Outgoing>(
        self: &Arc<Self>,
        mut outgoing: SocketSink,
        incoming: impl Stream<Item = Incoming>,
    ) -> Result<()>
    where
        Incoming: AsRef<[u8]> + Send + Sync + 'static,
        Outgoing: From<Vec<u8>>,
        SocketSink: Sink<Outgoing> + Unpin,
        SocketSink::Error: Send + Sync + error::Error + 'static,
    {
        let incoming = incoming.then(|msg| async {
            Event::Incoming {
                // Get the in-flight permit on the message stream, so we block the stream until we
                // have a permit.
                in_flight_permit: self
                    .in_flight
                    .clone()
                    .acquire_owned()
                    .await
                    .expect("Semaphore was closed unexpectedly"),
                msg,
            }
        });

        // We want this to block as a message is still in-flight until it's been sent to
        // the websocket, hence the queue size = 1.
        let (result_sink, result_stream) = mpsc::channel::<Result<Vec<u8>>>(1);
        let result_stream = result_stream.map(Event::Outgoing);
        let mut events = stream_select!(Box::pin(incoming), result_stream);

        while let Some(event) = events.next().await {
            match event {
                Event::Incoming {
                    in_flight_permit,
                    msg,
                } => {
                    let mut result_sink = result_sink.clone();
                    let handler = self.clone();
                    spawn(async move {
                        if let Err(e) = handler.handle_msg(msg.as_ref(), &result_sink).await {
                            result_sink
                                .send(Err(e))
                                .await
                                .unwrap_or_else(client_disconnected);
                        }

                        mem::drop(in_flight_permit);
                    });
                }
                Event::Outgoing(msg) => {
                    let is_err = outgoing
                        .send(msg?.into())
                        .await
                        .map_err(client_disconnected)
                        .is_err();

                    if is_err {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle a raw Websocket message.
    ///
    /// This will read a `MsgId` from the message and route it to the correct
    /// implementation. Prefer using [`Self::handle_socket`] if it's general
    /// enough.
    pub async fn handle_msg(&self, mut msg: &[u8], result_sink: &ResultSink) -> Result<()> {
        let protocol_version: usize = deserialize_part(&mut msg)?;

        if protocol_version != protocol::VERSION {
            return Err(Error::Protocol(format!(
                "Unknown protocol version: Expected {}, got {}",
                protocol::VERSION,
                protocol_version
            )));
        }

        let id: Vec<u8> = deserialize_part(&mut msg)?;

        let Some(function) = self.runners.get(&id)
        else { return Err(Error::FunctionNotFound) };

        function(msg, result_sink).await
    }
}

fn client_disconnected(e: impl error::Error) {
    tracing::info!("Send failed: Client disconnected ({e}).");
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Function not found")]
    FunctionNotFound,
    #[error("Error unpacking message: {0}")]
    Protocol(String),
    #[error("Deserialization: {0}")]
    Deserialization(bincode::Error),
}

pub type Result<T> = result::Result<T, Error>;

type Id = Vec<u8>;
type RpcHandler =
    Box<dyn for<'a> Fn(&'a [u8], &ResultSink) -> BoxFuture<'a, Result<()>> + Send + Sync + 'static>;
type ResultSink = Sender<Result<Vec<u8>>>;

enum Event<Incoming> {
    Incoming {
        in_flight_permit: OwnedSemaphorePermit,
        msg: Incoming,
    },
    Outgoing(Result<Vec<u8>>),
}

fn serialize<W, T>(writer: W, t: &T)
where
    W: Write,
    T: Serialize + ?Sized,
{
    bincode::DefaultOptions::new()
        .serialize_into(writer, t)
        .unwrap()
}

fn deserialize<R, T>(reader: R) -> Result<T>
where
    R: Read,
    T: DeserializeOwned,
{
    bincode::DefaultOptions::new()
        .deserialize_from(reader)
        .map_err(Error::Deserialization)
}

fn deserialize_part<R, T>(reader: R) -> Result<T>
where
    R: Read,
    T: DeserializeOwned,
{
    bincode::DefaultOptions::new()
        .allow_trailing_bytes()
        .deserialize_from(reader)
        .map_err(Error::Deserialization)
}
