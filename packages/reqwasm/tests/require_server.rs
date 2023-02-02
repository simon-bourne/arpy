//! These tests are run from `provide_server`
use arpy_test::{Add, PORT};
use reqwasm::websocket::futures::WebSocket;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn http_client() {
    let connection = http::Connection::new(&server_url("http"));

    assert_eq!(3, Add(1, 2).call(&connection).await.unwrap());
}

#[wasm_bindgen_test]
async fn websocket_client() {
    let ws = WebSocket::open(&server_url("ws")).unwrap();
    let connection = websocket::Connection::new(ws);

    assert_eq!(3, Add(1, 2).call(&connection).await.unwrap());
}

fn server_url(scheme: &str, route: &str) -> String {
    let port_str = format!("{PORT}");
    let port = option_env!("TCP_PORT").unwrap_or(&port_str);
    format!("{scheme}://127.0.0.1:{port}/{route}")
}
