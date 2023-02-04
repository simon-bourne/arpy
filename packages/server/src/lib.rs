use std::future::Future;

use arpy::FnRemote;
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
