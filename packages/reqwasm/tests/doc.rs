use arpy::{FnRemote, RpcClient, MsgId};
use arpy_reqwasm::{http, websocket};
use reqwasm::websocket::futures::WebSocket;
use serde::{Deserialize, Serialize};

#[derive(MsgId, Serialize, Deserialize, Debug)]
struct MyAdd(u32, u32);

impl FnRemote for MyAdd {
    type Output = u32;
}

async fn my_app<Conn: RpcClient>(conn: &mut Conn) -> Result<(), Conn::Error> {
    let result = MyAdd(1, 2).call(conn).await?;

    assert_eq!(3, result);

    Ok(())
}

#[allow(unused_must_use)]
pub fn http_client() {
    async {
        my_app(&mut http::Connection::new("http://127.0.0.1:9090/api/http"));
    };
}

#[allow(unused_must_use)]
pub fn websocket_client() {
    async {
        let ws = WebSocket::open("ws://127.0.0.1:9090/api/ws").unwrap();
        my_app(&mut websocket::Connection::new(ws));
    };
}
