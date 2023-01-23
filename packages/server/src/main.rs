use std::{thread, time::Duration};

use async_trait::async_trait;
use axum::{
    body::{boxed, Body, Full},
    extract::Path,
    http::{header::ACCEPT, HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router, Server,
};
use hyper::{body, header::CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Add(i32, i32);

#[async_trait]
impl Rpc for Add {
    type ResultType = i32;

    async fn serve(&self) -> Self::ResultType {
        self.0 + self.1
    }
}

#[async_trait]
trait Rpc {
    type ResultType: Serialize;

    // TODO: Error types
    async fn call(&self) -> Result<Self::ResultType, &'static str>
    where
        Self: Serialize,
        Self::ResultType: for<'a> Deserialize<'a>,
    {
        let mut body = Vec::new();

        ciborium::ser::into_writer(self, &mut body).map_err(|_| "Error serializing")?;

        // TODO: Accept and content_type headers
        // TODO: Pass in client, wrapped in something.
        let client = reqwest::Client::new();
        let result = client
            .post("http://127.0.0.1:9090/api/add")
            .header(ACCEPT, "application/cbor")
            .body(body)
            .send()
            .await
            .map_err(|_| "Error sending request")?;

        println!("{}", result.status());
        let result_bytes = result
            .bytes()
            .await
            .map_err(|_| "Error receiving response")?;
        let result: Self::ResultType = ciborium::de::from_reader(result_bytes.as_ref())
            .map_err(|_| "Error deserializing response")?;

        Ok(result)
    }

    // TODO: `serve_fallible`
    async fn serve(&self) -> Self::ResultType;
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

    async fn serve<'a, T: Rpc + Deserialize<'a>>(
        self,
        data: &'a [u8],
    ) -> Result<Response, StatusCode> {
        match self {
            Self::Cbor => {
                let result =
                    ciborium::de::from_reader(data).map_err(|_| StatusCode::BAD_REQUEST)?;

                let mut body = Vec::new();

                ciborium::ser::into_writer(&T::serve(&result).await, &mut body)
                    .map_err(|_| StatusCode::BAD_REQUEST)?;

                Response::builder()
                    .header(CONTENT_TYPE, "application/cbor")
                    .body(boxed(Full::from(body)))
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
            }
            Self::Json => {
                let result = serde_json::from_slice(data).map_err(|_| StatusCode::BAD_REQUEST)?;

                let body = serde_json::to_vec(&T::serve(&result).await)
                    .map_err(|_| StatusCode::BAD_REQUEST)?;

                Response::builder()
                    .header(CONTENT_TYPE, "application/json")
                    .body(boxed(Full::from(body)))
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
            }
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
    println!(
        "{}",
        Add(1, 2).call().await.map_err(|e| println!("{e}")).unwrap()
    );
}

#[tokio::main]
async fn server() {
    let app = Router::new().route("/api/:function", post(handler));
    Server::bind(&"0.0.0.0:9090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(
    Path(function): Path<String>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<impl IntoResponse, StatusCode> {
    println!("Function: {function}");

    // TODO: Better logging and error reporting
    let content_type = headers
        .get(ACCEPT)
        .map(|e| {
            println!("Missing accept");
            e
        })
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

    match function.as_str() {
        "add" => content_type.serve::<Add>(body.as_ref()).await,
        _ => Err(StatusCode::NOT_FOUND),
    }
}
