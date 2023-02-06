//! Building blocks to implement an Arpy Websocket server.
//!
//! See the `axum` and `actix` implementations under `packages` in the
//! repository.
use std::{collections::HashMap, error, io, mem, result, sync::Arc};

use arpy::{protocol, FnRemote, FnSubscription};
use bincode::Options;
use futures::{
    channel::mpsc::{self, Sender},
    future::BoxFuture,
    stream_select, Sink, SinkExt, Stream, StreamExt,
};
use serde::de::DeserializeOwned;
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

    fn deserialize_msg<Msg: DeserializeOwned>(
        mut input: impl io::Read,
    ) -> Result<(DefaultKey, Msg)> {
        let serializer = bincode::DefaultOptions::new().allow_trailing_bytes();
        let msg: Msg = serializer
            .deserialize_from(&mut input)
            .map_err(Error::Deserialization)?;
        let client_id: DefaultKey = serializer
            .deserialize_from(input)
            .map_err(Error::Deserialization)?;
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

        let serializer = bincode::DefaultOptions::new();
        let mut body = Vec::new();
        serializer
            .serialize_into(&mut body, &protocol::VERSION)
            .unwrap();
        serializer.serialize_into(&mut body, &client_id).unwrap();

        result_sink
            .send(Ok(body))
            .await
            .unwrap_or_else(client_disconnected);

        spawn(async move {
            // TODO: Cancellation
            // TODO: We need a subscription count semaphore. This needs to be separate from
            // the in-flight semaphore, as we still need to be able to receive cancellation
            // requests. Need to think about the protocol, particularly around subscriptions
            // backing up.
            while let Some(item) = items.next().await {
                let mut body = Vec::new();
                serializer
                    .serialize_into(&mut body, &protocol::VERSION)
                    .unwrap();
                serializer.serialize_into(&mut body, &client_id).unwrap();
                serializer.serialize_into(&mut body, &item).unwrap();

                if result_sink.send(Ok(body)).await.is_err() {
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
        let mut body = Vec::new();
        let serializer = bincode::DefaultOptions::new();
        serializer
            .serialize_into(&mut body, &protocol::VERSION)
            .unwrap();
        serializer.serialize_into(&mut body, &client_id).unwrap();
        serializer.serialize_into(&mut body, &result).unwrap();

        result_sink
            .send(Ok(body))
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
    pub fn new(router: WebSocketRouter) -> Arc<Self> {
        // TODO: Semaphore count arg
        Arc::new(Self {
            runners: router.0,
            // We use a semaphore so we have a resource limit shared between all connection, but
            // each connection can maintain it's own unbounded queue of in-flight RPC calls.
            in_flight: Arc::new(Semaphore::new(1000)),
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

        // TODO: Pass in channel bounds
        let (result_sink, result_stream) = mpsc::channel::<Result<Vec<u8>>>(1000);
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
        let serializer = bincode::DefaultOptions::new().allow_trailing_bytes();
        let protocol_version: usize = serializer
            .deserialize_from(&mut msg)
            .map_err(Error::Deserialization)?;

        if protocol_version != protocol::VERSION {
            return Err(Error::Protocol(format!(
                "Unknown protocol version: Expected {}, got {}",
                protocol::VERSION,
                protocol_version
            )));
        }

        let id: Vec<u8> = serializer
            .deserialize_from(&mut msg)
            .map_err(Error::Deserialization)?;

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
