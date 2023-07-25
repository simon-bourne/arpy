use arpy::{FnRemote, FnSubscription, MsgId};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub mod server;

#[derive(MsgId, Serialize, Deserialize, Debug)]
pub struct Add(pub i32, pub i32);

impl FnRemote for Add {
    type Output = i32;
}

#[derive(MsgId, Serialize, Deserialize, Debug)]
pub struct TryMultiply(pub i32, pub i32);

impl FnRemote for TryMultiply {
    type Output = Result<i32, ()>;
}

#[derive(MsgId, Serialize, Deserialize, Debug)]
pub struct Counter(pub i32);

impl FnSubscription for Counter {
    type InitialReply = ();
    type Item = i32;
    type Update = ();
}

#[derive(MsgId, Serialize, Deserialize, Debug)]
pub struct AddN(pub i32);

impl FnSubscription for AddN {
    type InitialReply = String;
    type Item = i32;
    type Update = i32;
}

pub const ADD_N_REPLY: &str = "AddN initialized";
pub const PORT: u16 = 9090;
