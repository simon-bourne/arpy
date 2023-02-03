//! These tests are run from `provide_server`
use arpy::FnClient;
use arpy_reqwasm::{http, websocket};
use arpy_test::{Add, PORT};
use reqwasm::websocket::futures::WebSocket;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn simple_http() {
    let mut connection = http::Connection::new(&server_url("http", "http"));

    assert_eq!(3, Add(1, 2).call(&mut connection).await.unwrap());
}

#[wasm_bindgen_test]
async fn simple_websocket() {
    let ws = WebSocket::open(&server_url("ws", "ws")).unwrap();
    let mut connection = websocket::Connection::new(ws);

    assert_eq!(3, Add(1, 2).call(&mut connection).await.unwrap());
}

#[wasm_bindgen_test]
async fn out_of_order_websocket() {
    let ws = WebSocket::open(&server_url("ws", "ws")).unwrap();
    let connection = websocket::Connection::new(ws);

    let result1 = connection.async_call(Add(1, 2)).await.unwrap();
    let result2 = connection.async_call(Add(3, 4)).await.unwrap();

    // Await in reverse order
    assert_eq!(7, result2.await.unwrap());
    assert_eq!(3, result1.await.unwrap());
}

fn server_url(scheme: &str, route: &str) -> String {
    let port_str = format!("{PORT}");
    let port = option_env!("TCP_PORT").unwrap_or(&port_str);
    format!("{scheme}://127.0.0.1:{port}/{route}")
}
