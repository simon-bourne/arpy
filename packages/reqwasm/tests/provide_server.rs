#![cfg(not(target_arch = "wasm32"))]
use std::{
    env::{self, set_current_dir},
    path::Path,
};

use arpy_test::server::dev_server;
use tokio::process::Command;

#[tokio::test]
async fn with_server() {
    let server = dev_server(0);

    let port = server.local_addr().port();

    server
        .with_graceful_shutdown(async {
            let directory = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("../reqwasm");
            set_current_dir(directory).unwrap();
            env::set_var("TCP_PORT", format!("{port}"));
            let mut child = Command::new("wasm-pack")
                .arg("test")
                .arg("--headless")
                .arg("--firefox")
                .spawn()
                .unwrap();

            assert!(child.wait().await.unwrap().success());
        })
        .await
        .unwrap();
}
