mod websocket;

use arpy::FnRemote;
use std::future::Future;
pub use websocket::{WebSocketHandler, WebSocketRouter};

pub trait FnRemoteBody<Args>
where
    Args: FnRemote,
{
    type Fut: Future<Output = Args::Output> + Send + Sync;

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
