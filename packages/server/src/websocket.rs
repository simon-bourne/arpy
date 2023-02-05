//! Building blocks to implement an Arpy Websocket server.
//!
//! See the `axum` and `actix` implementations under `packages` in the
//! repository.
use std::{collections::HashMap, error, io, mem, result, sync::Arc};

use arpy::{FnRemote, PROTOCOL_VERSION};
use bincode::Options;
use futures::{channel::mpsc, future::BoxFuture, stream_select, Sink, SinkExt, Stream, StreamExt};
use slotmap::DefaultKey;
use thiserror::Error;
use tokio::{
    spawn,
    sync::{Semaphore, SemaphorePermit},
};

use crate::FnRemoteBody;

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
    {
        let id = FSig::ID.as_bytes().to_vec();
        let f = Arc::new(f);
        self.0.insert(
            id,
            Box::new(move |body| Box::pin(Self::run(f.clone(), body))),
        );

        self
    }

    async fn run<F, FSig>(f: Arc<F>, mut input: impl io::Read) -> Result<Vec<u8>>
    where
        F: FnRemoteBody<FSig> + Send + Sync + 'static,
        FSig: FnRemote + Send + Sync + 'static,
    {
        let serializer = bincode::DefaultOptions::new();
        let args: FSig = serializer
            .deserialize_from(&mut input)
            .map_err(Error::Deserialization)?;
        let id: DefaultKey = serializer
            .deserialize_from(input)
            .map_err(Error::Deserialization)?;
        let result = f.run(args).await;
        let mut body = Vec::new();
        serializer
            .serialize_into(&mut body, &PROTOCOL_VERSION)
            .unwrap();
        serializer.serialize_into(&mut body, &id).unwrap();
        serializer.serialize_into(&mut body, &result).unwrap();

        Ok(body)
    }
}

/// Handle raw messages from a websocket.
///
/// Use `WebSocketHandler` to implement a Websocket server.
pub struct WebSocketHandler {
    runners: HashMap<Id, RpcHandler>,
    in_flight: Semaphore,
}

impl WebSocketHandler {
    pub fn new(router: WebSocketRouter) -> Arc<Self> {
        // TODO: Semaphore count arg
        Arc::new(Self {
            runners: router.0,
            // We use a semaphore so we have a resource limit shared between all connection, but
            // each connection can maintain it's own unbounded queue of in-flight RPC calls.
            in_flight: Semaphore::new(1000),
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
        enum Event<'a, Incoming0> {
            Incoming {
                in_flight_permit: SemaphorePermit<'a>,
                msg: Incoming0,
            },
            Outgoing(Result<Vec<u8>>),
        }

        let incoming = incoming.then(|msg| async {
            Event::Incoming {
                // Get the in-flight permit on the message stream, so we block the stream until we
                // have a permit.
                in_flight_permit: self.in_flight.acquire().await.unwrap(),
                msg,
            }
        });

        let (pending_runs, completed_runs) = mpsc::unbounded::<Result<Vec<u8>>>();
        let completed_runs = completed_runs.map(Event::Outgoing);
        let mut events = stream_select!(Box::pin(incoming), completed_runs);

        while let Some(event) = events.next().await {
            match event {
                Event::Incoming {
                    in_flight_permit,
                    msg,
                } => {
                    let mut pending_runs = pending_runs.clone();
                    let handler = self.clone();
                    spawn(async move {
                        pending_runs
                            .send(handler.handle_msg(msg.as_ref()).await)
                            .await
                            .unwrap()
                    });

                    mem::drop(in_flight_permit);
                }
                Event::Outgoing(msg) => outgoing
                    .send(msg?.into())
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?,
            }
        }

        Ok(())
    }

    /// Handle a raw Websocket message.
    ///
    /// This will read an `MsgId` from the message and route it to the correct
    /// implementation. Prefer using [`Self::handle_socket`] if it's general
    /// enough.
    pub async fn handle_msg(&self, mut msg: &[u8]) -> Result<Vec<u8>> {
        // TODO: Add a protocol version check
        let serializer = bincode::DefaultOptions::new().allow_trailing_bytes();
        let protocol_version: usize = serializer
            .deserialize_from(&mut msg)
            .map_err(Error::Deserialization)?;

        if protocol_version != PROTOCOL_VERSION {
            return Err(Error::Protocol(format!(
                "Unknown protocol version: Expected {}, got {}",
                PROTOCOL_VERSION, protocol_version
            )));
        }

        let id: Vec<u8> = serializer
            .deserialize_from(&mut msg)
            .map_err(Error::Deserialization)?;

        let Some(function) = self.runners.get(&id)
        else { return Err(Error::FunctionNotFound) };

        function(msg).await
    }
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
    Box<dyn for<'a> Fn(&'a [u8]) -> BoxFuture<'a, Result<Vec<u8>>> + Send + Sync + 'static>;
