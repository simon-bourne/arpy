//! # Arpy
//!
//! Define RPC call signatures for use with Arpy providers. See the `examples`
//! folder in this repo for various client/server provider examples.
use std::{
    error::Error,
    fmt::Debug,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};

/// Derive a [`protocol::MsgId`].
///
/// It will use the kebab cased type name without any generics or module path.
pub use arpy_macros::MsgId;
use async_trait::async_trait;
use futures::{Future, Stream};
use pin_project::pin_project;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

/// A remote procedure.
///
/// This defines the signature of an RPC call, which can then be used by the
/// client or the server. The data items in the implementor are the parameters
/// to the remote call.
#[async_trait(?Send)]
pub trait FnRemote: protocol::MsgId + Serialize + DeserializeOwned + Debug {
    /// The return type.
    type Output: Serialize + DeserializeOwned + Debug;

    /// Allow `function.call(connection)` instead of
    /// `connection.call(function)`.
    ///
    /// The default implementation defers to [`RpcClient::call`]. You shouldn't
    /// need to implement this.
    async fn call<C>(self, connection: &C) -> Result<Self::Output, C::Error>
    where
        C: RpcClient,
    {
        connection.call(self).await
    }

    /// Allow `function.call(connection)` instead of
    /// `connection.call(function)`.
    ///
    /// The default implementation defers to
    /// [`ConcurrentRpcClient::begin_call`]. You shouldn't need to implement
    /// this.
    async fn begin_call<C>(self, connection: &C) -> Result<C::Call<Self::Output>, C::Error>
    where
        C: ConcurrentRpcClient,
    {
        connection.begin_call(self).await
    }
}

/// Allow a fallible `FnRemote` to be called like a method.
///
/// A blanket implementation is provided for any `T: FnRemote`.
#[async_trait(?Send)]
pub trait FnTryRemote<Success, Error>: FnRemote<Output = Result<Success, Error>> {
    /// Allow `function.call(connection)` instead of
    /// `connection.call(function)`.
    ///
    /// The default implementation defers to [`RpcClient::try_call`]. You
    /// shouldn't need to implement this.
    async fn try_call<C>(self, connection: &C) -> Result<Success, ErrorFrom<C::Error, Error>>
    where
        C: RpcClient,
    {
        connection.try_call(self).await
    }

    /// Allow `function.call(connection)` instead of
    /// `connection.call(function)`.
    ///
    /// The default implementation defers to
    /// [`ConcurrentRpcClient::try_begin_call`]. You shouldn't need to implement
    /// this.
    async fn try_begin_call<C>(self, connection: &C) -> Result<TryCall<Success, Error, C>, C::Error>
    where
        Self: Sized,
        Success: DeserializeOwned,
        Error: DeserializeOwned,
        C: ConcurrentRpcClient,
    {
        connection.try_begin_call(self).await
    }
}

impl<Success, Error, T> FnTryRemote<Success, Error> for T where
    T: FnRemote<Output = Result<Success, Error>>
{
}

/// A parameterized subscription.
///
/// The data items in the implementor are the parameters to the subscription.
pub trait FnSubscription: protocol::MsgId + Serialize + DeserializeOwned + Debug {
    /// The initial reply that you'll receive when you subscribe.
    type InitialReply: Serialize + DeserializeOwned + Debug;

    /// The subscription will give you back a stream of `Item`.
    type Item: Serialize + DeserializeOwned + Debug;

    /// The subscription can be updated with a stream of `Update`.
    type Update: Serialize + DeserializeOwned + Debug;
}

/// An RPC client.
///
/// Implement this to provide an RPC client. It uses [`async_trait`] to provide
/// `async` methods. See the `arpy_reqwest` crate for an example.
///
/// [`async_trait`]: async_trait::async_trait
#[async_trait(?Send)]
pub trait RpcClient {
    /// A transport error
    type Error: Error + Debug + Send + Sync + 'static;

    /// Make an RPC call.
    async fn call<F>(&self, function: F) -> Result<F::Output, Self::Error>
    where
        F: FnRemote;

    /// Make a fallible RPC call.
    ///
    /// You shouldn't need to implement this. It just flattens any errors sent
    /// from the server into an [`ErrorFrom`].
    async fn try_call<F, Success, Error>(
        &self,
        function: F,
    ) -> Result<Success, ErrorFrom<Self::Error, Error>>
    where
        Self: Sized,
        F: FnRemote<Output = Result<Success, Error>>,
    {
        match self.call(function).await {
            Ok(Ok(ok)) => Ok(ok),
            Ok(Err(e)) => Err(ErrorFrom::Application(e)),
            Err(e) => Err(ErrorFrom::Transport(e)),
        }
    }
}

/// An RPC Client that can have many calls in-flight at once.
#[async_trait(?Send)]
pub trait ConcurrentRpcClient {
    /// A transport error
    type Error: Error + Debug + Send + Sync + 'static;
    type Call<Output: DeserializeOwned>: Future<Output = Result<Output, Self::Error>>;
    type SubscriptionStream<Item: DeserializeOwned>: Stream<Item = Result<Item, Self::Error>>
        + Unpin;

    /// Initiate a call, but don't wait for results until `await`ed again.
    ///
    /// `MyFn(...).begin_call(&conn).await` will asynchronously send the call
    /// message to the server and yield another future. It won't wait for the
    /// reply until you `await` the second future.
    ///
    /// This allows you to send off a bunch of requests to the server at once,
    /// without waiting for round trip results. When you want the results, await
    /// the second futures in any order. The connection will handle routing
    /// replies to the correct futures. The memory used will be proportional
    /// to the maximum number of requests in flight at once.
    ///
    /// # Example
    ///
    /// ```
    /// # use arpy::{ConcurrentRpcClient, FnRemote, MsgId};
    /// # use serde::{Serialize, Deserialize};
    /// # use std::future::Ready;
    /// #
    /// #[derive(MsgId, Serialize, Deserialize, Debug)]
    /// struct MyAdd(u32, u32);
    ///
    /// impl FnRemote for MyAdd {
    ///     type Output = u32;
    /// }
    ///
    /// async fn example(conn: impl ConcurrentRpcClient) {
    ///     // Send off 2 request to the server.
    ///     let result1 = MyAdd(1, 2).begin_call(&conn).await.unwrap();
    ///     let result2 = MyAdd(3, 4).begin_call(&conn).await.unwrap();
    ///
    ///     // Now wait for the results. The order doesn't matter here.
    ///     assert_eq!(7, result2.await.unwrap());
    ///     assert_eq!(3, result1.await.unwrap());
    /// }
    /// ```
    async fn begin_call<F>(&self, function: F) -> Result<Self::Call<F::Output>, Self::Error>
    where
        F: FnRemote;

    /// Fallible version of [`Self::begin_call`].
    ///
    /// This will flatten the transport and application errors into an
    /// [`ErrorFrom`].
    async fn try_begin_call<F, Success, Error>(
        &self,
        function: F,
    ) -> Result<TryCall<Success, Error, Self>, Self::Error>
    where
        Self: Sized,
        F: FnRemote<Output = Result<Success, Error>>,
        Success: DeserializeOwned,
        Error: DeserializeOwned,
    {
        Ok(TryCall {
            call: self.begin_call(function).await?,
        })
    }

    /// Subscripte to a stream of `S::Item`.
    ///
    /// # Example
    ///
    /// ```
    /// # use arpy::{ConcurrentRpcClient, FnSubscription, MsgId};
    /// # use serde::{Serialize, Deserialize};
    /// # use std::future::Ready;
    /// # use futures::{stream, StreamExt};
    /// #
    /// #[derive(MsgId, Serialize, Deserialize, Debug)]
    /// struct MyCounter {
    ///     start_at: i32,
    /// }
    ///
    /// impl FnSubscription for MyCounter {
    ///     type InitialReply = ();
    ///     type Item = i32;
    ///     type Update = ();
    /// }
    ///
    /// async fn example(conn: impl ConcurrentRpcClient) {
    ///     let (initial_reply, mut subscription) = conn
    ///         .subscribe(MyCounter { start_at: 10 }, stream::pending())
    ///         .await
    ///         .unwrap();
    ///
    ///     while let Some(count) = subscription.next().await {
    ///         println!("{}", count.unwrap());
    ///     }
    /// }
    /// ```
    async fn subscribe<S>(
        &self,
        service: S,
        updates: impl Stream<Item = S::Update> + 'static,
    ) -> Result<(S::InitialReply, Self::SubscriptionStream<S::Item>), Self::Error>
    where
        S: FnSubscription + 'static;
}

/// The [`Future`] returned from [`ConcurrentRpcClient::try_begin_call`].
///
/// Flattens a transport and application error into an [`ErrorFrom`].
#[pin_project]
pub struct TryCall<Success, Error, Client>
where
    Success: DeserializeOwned,
    Error: DeserializeOwned,
    Client: ConcurrentRpcClient,
{
    #[pin]
    call: Client::Call<Result<Success, Error>>,
}

impl<Success, Error, Client> Future for TryCall<Success, Error, Client>
where
    Success: DeserializeOwned,
    Error: DeserializeOwned,
    Client: ConcurrentRpcClient,
{
    type Output = Result<Success, ErrorFrom<Client::Error, Error>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().call.poll(cx).map(|reply| {
            reply
                .map_err(ErrorFrom::Transport)?
                .map_err(ErrorFrom::Application)
        })
    }
}

#[async_trait(?Send)]
pub trait ServerSentEvents {
    /// A transport error
    type Error: Error + Debug + Send + Sync + 'static;
    type Output<Item: DeserializeOwned>: Stream<Item = Result<Item, Self::Error>>;

    async fn subscribe<T>(&self) -> Result<Self::Output<T>, Self::Error>
    where
        T: DeserializeOwned + protocol::MsgId;
}

/// An error from a fallible RPC call.
///
/// A fallible RPC call is one where `FnRemote::Output = Result<_, _>`.
#[derive(Error, Debug)]
pub enum ErrorFrom<C, S> {
    /// A transport error.
    #[error("Transport: {0}")]
    Transport(C),
    /// An error from `FnRemote::Output`.
    #[error("Application: {0}")]
    Application(S),
}

/// Protocol related utilities.
pub mod protocol {
    use serde::{Deserialize, Serialize};

    /// The protocol version.
    ///
    /// This is this first item in every message and is checked when reading
    /// each message.
    pub const VERSION: usize = 0;

    /// This should be `derive`d with [`crate::MsgId`].
    pub trait MsgId {
        /// `ID` should be a short identifier to uniquely identify a message
        /// type on a server.
        const ID: &'static str;
    }

    #[derive(Serialize, Deserialize)]
    pub enum SubscriptionControl {
        New,
        Update,
    }
}

/// Some common mime types supported by Arpy providers.
#[derive(Copy, Clone)]
pub enum MimeType {
    Cbor,
    Json,
    XwwwFormUrlencoded,
}

impl MimeType {
    /// The mime type, for example `"application/cbor"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cbor => "application/cbor",
            Self::Json => "application/json",
            Self::XwwwFormUrlencoded => "application/x-www-form-urlencoded",
        }
    }
}

impl FromStr for MimeType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with(Self::Cbor.as_str()) {
            Ok(Self::Cbor)
        } else if s.starts_with(Self::Json.as_str()) {
            Ok(Self::Json)
        } else if s.starts_with(Self::XwwwFormUrlencoded.as_str()) {
            Ok(Self::XwwwFormUrlencoded)
        } else {
            Err(())
        }
    }
}
