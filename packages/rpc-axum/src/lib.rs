use std::str::FromStr;

use axum::{
    body::{boxed, Body, Full},
    http::{header::ACCEPT, HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{post, MethodRouter},
};
use hyper::{body, header::CONTENT_TYPE};
use rpc::{MimeType, RemoteFn};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't deserialize result: {0}")]
    DeserializeResult(String),
}

pub fn handle_rpc<T: RemoteFn + 'static>() -> MethodRouter
where
    for<'a> &'a T: Send,
{
    post(handler::<T>)
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
        .and_then(|s| MimeType::from_str(s).ok())
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
