//! These tests are run from a native test in `test-server`.
//!
//! They need a dev server running, and that requires various non-wasm crates,
//! so we can't build that test in this crate.
use reqwasm::websocket::futures::WebSocket;
use rpc::RpcClient;
use rpc_reqwasm::{http, websocket};
use rpc_test_data::Add;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn http_client() {
    let mut request = http::Connection::new(&server_url("http"));

    assert_eq!(3, request.call(&Add(1, 2)).await.unwrap());
}

#[wasm_bindgen_test]
async fn websocket_client() {
    let ws = WebSocket::open(&server_url("ws")).unwrap();
    let mut request = websocket::Connection::new(ws);

    assert_eq!(3, request.call(&Add(1, 2)).await.unwrap());
}

fn server_url(scheme: &str) -> String {
    let port = option_env!("TCP_PORT").unwrap();
    format!("{scheme}://127.0.0.1:{port}/{scheme}/add")
}
