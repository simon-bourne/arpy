//! These tests require `test-server` to be running
use reqwasm::websocket::futures::WebSocket;
use rpc::RpcClient;
use rpc_reqwasm::{http, websocket};
use rpc_test_data::{Add, PORT};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn http_client() {
    let mut request = http::Connection::new(&format!("http://127.0.0.1:{PORT}/http/add"));

    assert_eq!(3, request.call(&Add(1, 2)).await.unwrap());
}

#[wasm_bindgen_test]
async fn websocket_client() {
    let ws = WebSocket::open(&format!("ws://127.0.0.1:{PORT}/websocket/add")).unwrap();
    let mut request = websocket::Connection::new(ws);

    assert_eq!(3, request.call(&Add(1, 2)).await.unwrap());
}
