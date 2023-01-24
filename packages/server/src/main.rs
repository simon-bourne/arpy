use std::{thread, time::Duration};

use async_trait::async_trait;
use axum::{
    body::{boxed, Body, Full},
    http::{header::ACCEPT, HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{post, MethodRouter},
    Router, Server,
};
use hyper::{body, header::CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize)]
struct Add(i32, i32);

#[async_trait]
impl RemoteFn for Add {
    type ResultType = i32;

    async fn run(&self) -> Self::ResultType {
        self.0 + self.1
    }
}

// TODO: `RemoteFallibleFn`,
#[async_trait]
pub trait RemoteFn: Serialize + for<'a> Deserialize<'a> {
    type ResultType: Serialize + for<'a> Deserialize<'a>;

    async fn run(&self) -> Self::ResultType;
}

#[async_trait]
pub trait RpcClient {
    type Error;

    async fn call<'a, F>(self, function: &'a F) -> Result<F::ResultType, Self::Error>
    where
        F: RemoteFn,
        &'a F: Send;
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't deserialize result: {0}")]
    DeserializeResult(String),
    #[error("Error sending request: {0}")]
    Send(reqwest::Error),
    #[error("Error receiving response: {0}")]
    Receive(reqwest::Error),
}

#[async_trait]
impl RpcClient for reqwest::RequestBuilder {
    type Error = Error;

    async fn call<'a, F>(self, function: &'a F) -> Result<F::ResultType, Self::Error>
    where
        F: RemoteFn,
        &'a F: Send,
    {
        let mut body = Vec::new();

        ciborium::ser::into_writer(&function, &mut body).unwrap();

        // TODO: Accept and content_type headers
        // TODO: Pass in client, wrapped in something.
        let result = self
            .header(ACCEPT, "application/cbor")
            .body(body)
            .send()
            .await
            .map_err(Error::Send)?;

        println!("{}", result.status());
        let result_bytes = result.bytes().await.map_err(Error::Receive)?;
        let result: F::ResultType = ciborium::de::from_reader(result_bytes.as_ref())
            .map_err(|e| Error::DeserializeResult(e.to_string()))?;

        Ok(result)
    }
}

#[derive(Copy, Clone)]
enum MimeType {
    Cbor,
    Json,
}

impl MimeType {
    fn from_str(s: &str) -> Option<Self> {
        if s.starts_with("application/cbor") {
            Some(Self::Cbor)
        } else if s.starts_with("application/json") {
            Some(Self::Json)
        } else {
            None
        }
    }
}

fn main() {
    thread::scope(|scope| {
        scope.spawn(server);
        scope.spawn(|| {
            thread::sleep(Duration::from_secs(1));
            client();
        });
    });
}

#[tokio::main]
async fn client() {
    let client = reqwest::Client::new().post("http://127.0.0.1:9090/api/add");
    let result = client.call(&Add(1, 2)).await;
    println!("{}", result.map_err(|e| println!("{e}")).unwrap());
}

#[tokio::main]
async fn server() {
    let app = Router::new().route("/api/add", rpc::<Add>());
    Server::bind(&"0.0.0.0:9090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn rpc<T: RemoteFn>() -> MethodRouter {
    post(handler::<Add>)
}

async fn handler<T: RemoteFn>(
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<impl IntoResponse, StatusCode> {
    // TODO: Better logging and error reporting
    let content_type = headers
        .get(ACCEPT)
        .and_then(|value| {
            value
                .to_str()
                .map_err(|e| {
                    println!("Bad accept");
                    e
                })
                .ok()
        })
        .and_then(MimeType::from_str)
        .ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?;

    let (_header, body) = request.into_parts();
    let body = body::to_bytes(body)
        .await
        .map_err(|_| StatusCode::PARTIAL_CONTENT)?;

    let data = body.as_ref();

    match content_type {
        MimeType::Cbor => {
            let result = ciborium::de::from_reader(data).map_err(|_| StatusCode::BAD_REQUEST)?;

            let mut body = Vec::new();

            ciborium::ser::into_writer(&T::run(&result).await, &mut body)
                .map_err(|_| StatusCode::BAD_REQUEST)?;

            Response::builder()
                .header(CONTENT_TYPE, "application/cbor")
                .body(boxed(Full::from(body)))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        MimeType::Json => {
            let result = serde_json::from_slice(data).map_err(|_| StatusCode::BAD_REQUEST)?;

            let body =
                serde_json::to_vec(&T::run(&result).await).map_err(|_| StatusCode::BAD_REQUEST)?;

            Response::builder()
                .header(CONTENT_TYPE, "application/json")
                .body(boxed(Full::from(body)))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
