use arpy::{FnRemote, RpcId};
use serde::{Deserialize, Serialize};

#[derive(RpcId, Serialize, Deserialize, Debug)]
struct MyAdd(u32, u32);

impl FnRemote for MyAdd {
    type Output = u32;
}
