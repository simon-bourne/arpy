use arpy::{FnRemote, MsgId, RpcClient};
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

pub async fn http_client() {
    my_app(&mut http::Connection::new("http://127.0.0.1:9090/api/http"))
        .await
        .unwrap();
}

pub async fn websocket_client() {
    let ws = WebSocket::open("ws://127.0.0.1:9090/api/ws").unwrap();
    my_app(&mut websocket::Connection::new(ws)).await.unwrap();
}
