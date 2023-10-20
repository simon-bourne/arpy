//! These tests are run from `provide_server`
use arpy::{ConcurrentRpcClient, FnRemote, FnTryRemote, ServerSentEvents};
use arpy_reqwasm::{eventsource, http, websocket};
use arpy_test::{Add, AddN, Counter, TryMultiply, ADD_N_REPLY, PORT};
use futures::{stream, StreamExt};
use reqwasm::websocket::futures::WebSocket;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

// TODO: Subscription tests (SSE and Websockets) are hanging in release mode,
// and sometimes receive messages out of order (even websockets).

#[wasm_bindgen_test]
async fn simple_http() {
    let connection = http::Connection::new(&server_url("http", "http"));

    assert_eq!(3, Add(1, 2).call(&connection).await.unwrap());
}

#[wasm_bindgen_test]
async fn sse() {
    let connection = eventsource::Connection::new(server_url("http", "sse"));

    let mut counter = connection.subscribe::<Counter>().await.unwrap();

    for i in 0..10 {
        assert_eq!(i, counter.next().await.unwrap().unwrap().0);
    }
}

#[wasm_bindgen_test]
async fn simple_websocket() {
    let connection = websocket();

    assert_eq!(3, Add(1, 2).call(&connection).await.unwrap());
    assert_eq!(12, TryMultiply(3, 4).try_call(&connection).await.unwrap());
}

#[wasm_bindgen_test]
async fn out_of_order_websocket() {
    let connection = websocket();

    let result1 = Add(1, 2).begin_call(&connection).await.unwrap();
    let result2 = TryMultiply(3, 4).try_begin_call(&connection).await.unwrap();

    // Await in reverse order
    assert_eq!(12, result2.await.unwrap());
    assert_eq!(3, result1.await.unwrap());
}

#[wasm_bindgen_test]
async fn websocket_subscription() {
    let connection = websocket();

    let ((), stream) = connection
        .subscribe(Counter(5), stream::pending())
        .await
        .unwrap();

    assert_eq!(
        stream
            .take(10)
            .map(Result::unwrap)
            .collect::<Vec<i32>>()
            .await,
        (5..15).collect::<Vec<i32>>()
    )
}

#[wasm_bindgen_test]
async fn websocket_subscription_updates() {
    let connection = websocket();

    let (iniitial_reply, stream) = connection
        .subscribe(AddN(1), stream::iter(0..10))
        .await
        .unwrap();

    assert_eq!(iniitial_reply, ADD_N_REPLY);
    assert_eq!(
        stream
            .take(10)
            .map(Result::unwrap)
            .collect::<Vec<i32>>()
            .await,
        (1..11).collect::<Vec<i32>>()
    )
}

fn websocket() -> websocket::Connection {
    websocket::Connection::new(WebSocket::open(&server_url("ws", "ws")).unwrap())
}

fn server_url(scheme: &str, route: &str) -> String {
    let port_str = format!("{PORT}");
    let port = option_env!("TCP_PORT").unwrap_or(&port_str);
    format!("{scheme}://127.0.0.1:{port}/{route}")
}
