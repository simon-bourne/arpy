use std::{io, ops::Deref};

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

impl Rpc for Add {
    type ResultType = i32;

    fn serve(&self) -> Self::ResultType {
        self.0 + self.1
    }
}

trait Rpc {
    type ResultType: Serialize;

    fn call(&self) -> Result<Self::ResultType, io::Error> {
        todo!()
    }

    fn serve(&self) -> Self::ResultType;
}

#[derive(Copy, Clone)]
enum MimeType {
    Cbor,
    Json,
}

impl MimeType {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "application/cbor" => Some(Self::Cbor),
            "application/json" => Some(Self::Json),
            _ => None,
        }
    }

    fn serve<'a, T: Rpc + Deserialize<'a>>(self, data: &'a [u8]) -> Result<Response, StatusCode> {
        match self {
            Self::Cbor => {
                let result =
                    ciborium::de::from_reader(data).map_err(|_| StatusCode::BAD_REQUEST)?;

                let mut body = Vec::new();

                ciborium::ser::into_writer(&T::serve(&result), &mut body)
                    .map_err(|_| StatusCode::BAD_REQUEST)?;

                Response::builder()
                    .header(CONTENT_TYPE, "application/cbor")
                    .body(boxed(Full::from(body)))
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
            }
            Self::Json => {
                let result = serde_json::from_slice(data).map_err(|_| StatusCode::BAD_REQUEST)?;

                let body =
                    serde_json::to_vec(&T::serve(&result)).map_err(|_| StatusCode::BAD_REQUEST)?;

                Response::builder()
                    .header(CONTENT_TYPE, "application/json")
                    .body(boxed(Full::from(body)))
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

#[tokio::main]
async fn main() {
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

    let content_type = headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .and_then(MimeType::from_str)
        .ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?;

    let (_header, body) = request.into_parts();
    let body = body::to_bytes(body)
        .await
        .map_err(|_| StatusCode::PARTIAL_CONTENT)?;

    match function.as_str() {
        "add" => content_type.serve::<Add>(body.deref()),
        _ => Err(StatusCode::NOT_FOUND),
    }
}
