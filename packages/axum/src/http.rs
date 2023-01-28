use std::{str::FromStr, sync::Arc};

use arpy::{FnRemote, MimeType};
use arpy_server::FnRemoteBody;
use async_trait::async_trait;
use axum::{
    body::{boxed, Bytes, Full},
    extract::FromRequest,
    http::{header::ACCEPT, HeaderMap, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{post, MethodRouter},
};
use hyper::header::CONTENT_TYPE;
use serde::Serialize;

pub fn handle_rpc<F, T>(f: F) -> MethodRouter
where
    F: FnRemoteBody<T> + Send + Sync + 'static,
    T: FnRemote + Send + Sync + 'static,
{
    let f = Arc::new(f);
    post(move |headers: HeaderMap, arpy: ArpyRequest<T>| handler(headers, arpy, f))
}

pub struct ArpyRequest<T>(T);

#[async_trait]
impl<T, S, B> FromRequest<S, B> for ArpyRequest<T>
where
    T: FnRemote,
    Bytes: FromRequest<S, B>,
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request(request: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = mime_type(request.headers().get(CONTENT_TYPE))?;

        let body = Bytes::from_request(request, state)
            .await
            .map_err(|_| StatusCode::PARTIAL_CONTENT)?;
        let body = body.as_ref();

        let thunk: T = match content_type {
            MimeType::Cbor => {
                ciborium::de::from_reader(body).map_err(|_| StatusCode::BAD_REQUEST)?
            }
            MimeType::Json => serde_json::from_slice(body).map_err(|_| StatusCode::BAD_REQUEST)?,
        };

        Ok(Self(thunk))
    }
}

pub struct ArpyResponse<T> {
    mime_type: MimeType,
    response: T,
}

impl<T> ArpyResponse<T> {
    pub fn new(mime_type: MimeType, response: T) -> Self {
        Self {
            mime_type,
            response,
        }
    }
}

impl<T> IntoResponse for ArpyResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        match self.mime_type {
            MimeType::Cbor => cbor_response(self.response).into_response(),
            MimeType::Json => json_response(self.response).into_response(),
        }
    }
}

fn cbor_response<T>(response: T) -> Result<impl IntoResponse, StatusCode>
where
    T: Serialize,
{
    let mut body = Vec::new();

    ciborium::ser::into_writer(&response, &mut body).map_err(|_| StatusCode::BAD_REQUEST)?;

    Response::builder()
        .header(CONTENT_TYPE, MimeType::Cbor.as_str())
        .body(boxed(Full::from(body)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn json_response<T>(response: T) -> Result<impl IntoResponse, StatusCode>
where
    T: Serialize,
{
    let body = serde_json::to_vec(&response).map_err(|_| StatusCode::BAD_REQUEST)?;

    Response::builder()
        .header(CONTENT_TYPE, "application/json")
        .body(boxed(Full::from(body)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn handler<F, T>(
    headers: HeaderMap,
    ArpyRequest(args): ArpyRequest<T>,
    f: Arc<F>,
) -> Result<impl IntoResponse, StatusCode>
where
    F: FnRemoteBody<T>,
    T: FnRemote,
{
    let response = f.run(args).await;
    let response_type = mime_type(headers.get(ACCEPT))?;
    Ok(ArpyResponse::new(response_type, response))
}

fn mime_type(header_value: Option<&HeaderValue>) -> Result<MimeType, StatusCode> {
    if let Some(accept) = header_value {
        MimeType::from_str(accept.to_str().map_err(|_| StatusCode::NOT_ACCEPTABLE)?)
            .map_err(|_| StatusCode::UNSUPPORTED_MEDIA_TYPE)
    } else {
        Ok(MimeType::Cbor)
    }
}
