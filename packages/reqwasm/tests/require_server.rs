//! These tests are run from `provide_server`
use arpy::FnClient;
use arpy_reqwasm::{http, websocket};
use arpy_test::Add;
use reqwasm::websocket::futures::WebSocket;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn http_client() {
    let mut request = http::Connection::new(&server_url("http"));

    assert_eq!(3, Add(1, 2).call(&mut request).await.unwrap());
}

#[wasm_bindgen_test]
async fn websocket_client() {
    let ws = WebSocket::open(&server_url("ws")).unwrap();
    let mut request = websocket::Connection::new(ws);

    assert_eq!(3, Add(1, 2).call(&mut request).await.unwrap());
}

fn server_url(scheme: &str) -> String {
    let port = option_env!("TCP_PORT").unwrap();
    format!("{scheme}://127.0.0.1:{port}/{scheme}")
}
