use std::future::Future;

use arpy::{FnRemote, FnSubscription};
use futures::{stream::BoxStream, Stream};
pub use websocket::{WebSocketHandler, WebSocketRouter};

pub mod websocket;

/// An implementation of a remote function.
///
/// You shouldn't need to implement this, as a blanket implementation is
/// provided for any `async` function or closure that takes a single
/// [`FnRemote`] argument and returns the [`FnRemote::Output`]. The future must
/// be `Send + Sync`.
pub trait FnRemoteBody<Args>
where
    Args: FnRemote,
{
    type Fut: Future<Output = Args::Output> + Send + Sync;

    /// Evaluate the function.
    fn run(&self, args: Args) -> Self::Fut;
}

impl<Args, Fut, F> FnRemoteBody<Args> for F
where
    Args: FnRemote,
    F: Fn(Args) -> Fut,
    Fut: Future<Output = Args::Output> + Send + Sync,
{
    type Fut = Fut;

    fn run(&self, args: Args) -> Self::Fut {
        self(args)
    }
}

/// An implementation of a subscription service.
///
/// You shouldn't need to implement this, as a blanket implementation is
/// provided for any `async` function or closure that takes a single
/// [`FnSubscription`] argument and returns a stream of
/// [`FnSubscription::Item`].
pub trait FnSubscriptionBody<Args>
where
    Args: FnSubscription,
    Args::Item: 'static,
    Args::Update: 'static,
{
    type Output: Stream<Item = Args::Item> + Send;

    /// Evaluate the function.
    fn run(
        &self,
        updates: impl Stream<Item = Args::Update> + Send + 'static,
        args: Args,
    ) -> (Args::InitialReply, Self::Output);
}

impl<Args, Output, F> FnSubscriptionBody<Args> for F
where
    Args: FnSubscription,
    F: Fn(BoxStream<'static, Args::Update>, Args) -> (Args::InitialReply, Output),
    Output: Stream<Item = Args::Item> + Send,
    Args::Item: 'static,
    Args::Update: 'static,
{
    type Output = Output;

    fn run(
        &self,
        updates: impl Stream<Item = Args::Update> + Send + 'static,
        args: Args,
    ) -> (Args::InitialReply, Self::Output) {
        self(Box::pin(updates), args)
    }
}
