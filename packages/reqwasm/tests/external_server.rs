// These tests require `test-servers` to be running
use rpc::RpcClient;
use rpc_reqwasm::http;
use rpc_test_data::{Add, PORT};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn http_client() {
    let mut request = http::Request::new(&format!("http://127.0.0.1:{PORT}/api/add"));

    assert_eq!(3, request.call(&Add(1, 2)).await.unwrap());
}
