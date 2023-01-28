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

pub fn handle_rpc<F, T>(f: F) -> MethodRouter
where
    F: FnRemoteBody<T> + Send + Sync + 'static,
    T: FnRemote + Send + Sync + 'static,
{
    let f = Arc::new(f);
    post(move |headers: HeaderMap, arpy: ArpyRequest<T>| handler(f, headers, arpy))
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

async fn handler<F, T>(
    f: Arc<F>,
    headers: HeaderMap,
    ArpyRequest(args): ArpyRequest<T>,
) -> Result<impl IntoResponse, StatusCode>
where
    F: FnRemoteBody<T>,
    T: FnRemote,
{
    let response = f.run(args).await;
    let response_type = mime_type(headers.get(ACCEPT))?;

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
