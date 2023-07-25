//! Reqwasm client for Arpy.
//!
//! This provides an [`http`] and a [`websocket`] client, suitable for use from
//! a browser.
use thiserror::Error;

pub mod eventsource;
pub mod http;
pub mod websocket;

pub use arpy_client::Error;
