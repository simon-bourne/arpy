use std::str::FromStr;

use axum::{
    body::{boxed, Body, Full},
    http::{header::ACCEPT, HeaderMap, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{post, MethodRouter},
};
use hyper::{body, header::CONTENT_TYPE};
use rpc::{FnRemote, MimeType};

pub fn handle_rpc<T: FnRemote + 'static>() -> MethodRouter
where
    for<'a> &'a T: Send,
{
    post(handler::<T>)
}

async fn handler<T: FnRemote>(
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<impl IntoResponse, StatusCode> {
    let response_type = mime_type(headers.get(ACCEPT))?;

    let body = body::to_bytes(request.into_body())
        .await
        .map_err(|_| StatusCode::PARTIAL_CONTENT)?;
    let body = body.as_ref();
    let content_type = mime_type(headers.get(CONTENT_TYPE))?;

    let thunk: T = match content_type {
        MimeType::Cbor => ciborium::de::from_reader(body).map_err(|_| StatusCode::BAD_REQUEST)?,
        MimeType::Json => serde_json::from_slice(body).map_err(|_| StatusCode::BAD_REQUEST)?,
    };

    let response = T::run(&thunk).await;

    match response_type {
        MimeType::Cbor => {
            let mut body = Vec::new();

            ciborium::ser::into_writer(&response, &mut body)
                .map_err(|_| StatusCode::BAD_REQUEST)?;

            Response::builder()
                .header(CONTENT_TYPE, MimeType::Cbor.as_str())
                .body(boxed(Full::from(body)))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        MimeType::Json => {
            let body = serde_json::to_vec(&response).map_err(|_| StatusCode::BAD_REQUEST)?;

            Response::builder()
                .header(CONTENT_TYPE, "application/json")
                .body(boxed(Full::from(body)))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn mime_type(header_value: Option<&HeaderValue>) -> Result<MimeType, StatusCode> {
    if let Some(accept) = header_value {
        MimeType::from_str(accept.to_str().map_err(|_| StatusCode::NOT_ACCEPTABLE)?)
            .map_err(|_| StatusCode::UNSUPPORTED_MEDIA_TYPE)
    } else {
        Ok(MimeType::Cbor)
    }
}
