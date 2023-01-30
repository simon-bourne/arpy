//! Building blocks for writing HTTP handlers.
//!
//! Try using [`RpcRoute::http_rpc_route`] first, and if that doesn't give
//! enough control, use the building blocks in this module.
//!
//! [`RpcRoute::http_rpc_route`]: crate::RpcRoute::http_rpc_route
use std::{convert::identity, fmt::Display, str::FromStr, sync::Arc};

use arpy::{FnRemote, MimeType};
use arpy_server::FnRemoteBody;
use async_trait::async_trait;
use axum::{
    body::{boxed, Bytes, Full},
    extract::FromRequest,
    http::{header::ACCEPT, HeaderMap, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
};
use hyper::header::CONTENT_TYPE;
use serde::Serialize;

/// An extractor for RPC requests.
///
/// When you need more control over the handler than
/// [`RpcRoute::http_rpc_route`] gives, you can implement your own RPC handler.
/// Use this to extract an RPC request in your handler implementation. See
/// [`axum::extract`] for more details.
///
/// # Example
///
/// ```
#[doc = include_doc::function_body!("tests/doc.rs", extractor_example, [my_handler])]
/// ```
/// 
/// [`RpcRoute::http_rpc_route`]: crate::RpcRoute::http_rpc_route
pub struct ArpyRequest<T>(pub T);

#[async_trait]
impl<Args, S, B> FromRequest<S, B> for ArpyRequest<Args>
where
    Args: FnRemote,
    Bytes: FromRequest<S, B>,
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(request: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = mime_type(request.headers().get(CONTENT_TYPE))?;

        let body = Bytes::from_request(request, state)
            .await
            .map_err(|_| error(StatusCode::PARTIAL_CONTENT, "Unable to read message"))?;
        let body = body.as_ref();

        let args: Args =
            match content_type {
                MimeType::Cbor => ciborium::de::from_reader(body)
                    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?,
                MimeType::Json => {
                    serde_json::from_slice(body).map_err(|e| error(StatusCode::BAD_REQUEST, e))?
                }
                MimeType::XwwwFormUrlencoded => serde_urlencoded::from_bytes(body)
                    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?,
            };

        Ok(Self(args))
    }
}

/// A responder for RPC requests.
///
/// Use this to construct a response for an RPC request handler when you need
/// more control than [`RpcRoute::http_rpc_route`] gives. See [`ArpyRequest`]
/// for more details and an example.
///
/// [`RpcRoute::http_rpc_route`]: crate::RpcRoute::http_rpc_route
pub struct ArpyResponse<T> {
    mime_type: MimeType,
    response: T,
}

impl<T> ArpyResponse<T> {
    /// Constructor.
    ///
    /// `response` will be serialized using an encoder corresponding to
    /// `mime_type` when it's converted into a [`Response`].
    pub fn new(mime_type: MimeType, response: T) -> Self {
        Self {
            mime_type,
            response,
        }
    }
}

impl<T> ArpyResponse<T>
where
    T: Serialize,
{
    fn try_into_response(self) -> Result<Response, Response> {
        let body = match self.mime_type {
            MimeType::Cbor => {
                let mut body = Vec::new();

                ciborium::ser::into_writer(&self.response, &mut body)
                    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
                boxed(Full::from(body))
            }
            MimeType::Json => {
                let body = serde_json::to_vec(&self.response)
                    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
                boxed(Full::from(body))
            }
            MimeType::XwwwFormUrlencoded => {
                let body = serde_urlencoded::to_string(&self.response)
                    .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
                boxed(Full::from(body))
            }
        };

        Response::builder()
            .header(CONTENT_TYPE, self.mime_type.as_str())
            .body(body)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    }
}

impl<T> IntoResponse for ArpyResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        self.try_into_response().map_or_else(identity, identity)
    }
}

/// An Axum handler for RPC requests.
///
/// Use this when you want more control over the route than
/// [`RpcRoute::http_rpc_route`] gives.
///
/// # Example
///
/// ```
#[doc = include_doc::function_body!("tests/doc.rs", handler_example, [my_add])]
/// ```
/// 
/// [`RpcRoute::http_rpc_route`]: crate::RpcRoute::http_rpc_route
pub async fn handler<F, Args>(
    headers: HeaderMap,
    ArpyRequest(args): ArpyRequest<Args>,
    f: Arc<F>,
) -> Result<impl IntoResponse, Response>
where
    F: FnRemoteBody<Args>,
    Args: FnRemote,
{
    let response = f.run(args).await;
    let response_type = mime_type(headers.get(ACCEPT))?;
    Ok(ArpyResponse::new(response_type, response))
}

fn mime_type(header_value: Option<&HeaderValue>) -> Result<MimeType, Response> {
    if let Some(accept) = header_value {
        let accept = accept
            .to_str()
            .map_err(|e| error(StatusCode::NOT_ACCEPTABLE, e))?;
        MimeType::from_str(accept).map_err(|_| error(StatusCode::UNSUPPORTED_MEDIA_TYPE, accept))
    } else {
        Ok(MimeType::Cbor)
    }
}

fn error(code: StatusCode, e: impl Display) -> Response {
    Response::builder()
        .status(code)
        .body(Full::from(e.to_string()))
        .map_or_else(|_| code.into_response(), Response::into_response)
}
