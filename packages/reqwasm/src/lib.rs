//! Reqwasm client for Arpy.
//!
//! This provides an [`http`] and a [`websocket`] client, suitable for use from
//! a browser.
use arpy_client::Spawner;
use thiserror::Error;

pub mod eventsource;
pub mod http;
pub mod websocket;

pub use arpy_client::Error;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone)]
struct LocalSpawner;

impl Spawner for LocalSpawner {
    fn spawn_local<F>(&self, future: F)
    where
        F: futures::Future<Output = ()> + 'static,
    {
        spawn_local(future)
    }
}
