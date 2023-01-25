use std::{
    env::{self, set_current_dir},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
};

use axum::{Router, Server};
use rpc_axum::{http, websocket};
use rpc_test_data::{Add, TryMultiply};
use tokio::process::Command;
use tower_http::cors::CorsLayer;

#[tokio::test]
async fn with_server() {
    let app = Router::new()
        .route("/http/add", http::handle_rpc::<Add>())
        .route("/http/multiply", http::handle_rpc::<TryMultiply>())
        .route("/ws/add", websocket::handle_rpc::<Add>())
        .route("/ws/multiply", websocket::handle_rpc::<TryMultiply>())
        .layer(CorsLayer::permissive());

    let server = Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .serve(app.into_make_service());

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
