//! Building blocks for writing HTTP handlers.
//!
//! Try using [`RpcApp::http_rpc_route`] first, and if that doesn't give
//! enough control, use the building blocks in this module.
//!
//! [`RpcApp::http_rpc_route`]: crate::RpcApp::http_rpc_route
use std::{convert::identity, pin::Pin, str::FromStr, sync::Arc};

use actix_web::{
    body::BoxBody,
    error::{self, ErrorBadRequest, ErrorNotAcceptable, ErrorUnsupportedMediaType},
    http::header::{HeaderValue, ACCEPT, CONTENT_TYPE},
    web::Bytes,
    FromRequest, HttpRequest, HttpResponse, Responder,
};
use arpy::{FnRemote, MimeType};
use arpy_server::FnRemoteBody;
use futures::Future;
use serde::Serialize;

/// An extractor for RPC requests.
///
/// When you need more control over the handler than [`RpcApp::http_rpc_route`]
/// gives, you can implement your own RPC handler. Use this to extract an RPC
/// request in your handler implementation. See [`actix_web::Handler`] and
/// [`actix_web::FromRequest`] for more details.
///
/// # Example
///
/// ```
#[doc = include_doc::function_body!("tests/doc.rs", extractor_example, [my_handler])]
/// ```
/// 
/// [`RpcApp::http_rpc_route`]: crate::RpcApp::http_rpc_route
pub struct ArpyRequest<T>(pub T);

impl<Args: FnRemote> FromRequest for ArpyRequest<Args> {
    type Error = error::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut actix_web::dev::Payload) -> Self::Future {
        let content_type = mime_type(req.headers().get(CONTENT_TYPE)).unwrap();
        let bytes = Bytes::from_request(req, payload);

        Box::pin(async move {
            let body = bytes.await?;
            let body = body.as_ref();

            let args: Args = match content_type {
                MimeType::Cbor => ciborium::de::from_reader(body).map_err(ErrorBadRequest)?,
                MimeType::Json => serde_json::from_slice(body).map_err(ErrorBadRequest)?,
                MimeType::XwwwFormUrlencoded => {
                    serde_urlencoded::from_bytes(body).map_err(ErrorBadRequest)?
                }
            };

            Ok(ArpyRequest(args))
        })
    }
}

/// A responder for RPC requests.
///
/// Use this to construct a response for an RPC request handler when you need
/// more control than [`RpcApp::http_rpc_route`] gives. See
/// [`actix_web::Responder`] for more details, and [`ArpyRequest`] for an
/// example.
///
/// [`RpcApp::http_rpc_route`]: crate::RpcApp::http_rpc_route
pub struct ArpyResponse<T>(pub T);

impl<T> Responder for ArpyResponse<T>
where
    T: Serialize,
{
    type Body = BoxBody;

    fn respond_to(self, req: &HttpRequest) -> HttpResponse<Self::Body> {
        try_respond_to(self.0, req).map_or_else(|e| e.error_response(), identity)
    }
}

fn try_respond_to<T>(response: T, req: &HttpRequest) -> Result<HttpResponse, error::Error>
where
    T: Serialize,
{
    let response_type = mime_type(req.headers().get(ACCEPT))?;

    let body = match response_type {
        MimeType::Cbor => {
            let mut response_body = Vec::new();

            ciborium::ser::into_writer(&response, &mut response_body).map_err(ErrorBadRequest)?;
            BoxBody::new(response_body)
        }
        MimeType::Json => BoxBody::new(serde_json::to_vec(&response).map_err(ErrorBadRequest)?),
        MimeType::XwwwFormUrlencoded => BoxBody::new(serde_urlencoded::to_string(&response)?),
    };

    Ok(HttpResponse::Ok()
        .content_type(response_type.as_str())
        .body(body))
}

/// An Actix handler for RPC requests.
///
/// Use this when you want more control over the route than
/// [`RpcApp::http_rpc_route`] gives.
///
/// # Example
///
/// ```
#[doc = include_doc::function_body!("tests/doc.rs", router_example, [my_add])]
/// ```
/// 
/// [`RpcApp::http_rpc_route`]: crate::RpcApp::http_rpc_route
pub async fn handler<F, Args>(f: Arc<F>, ArpyRequest(args): ArpyRequest<Args>) -> impl Responder
where
    F: FnRemoteBody<Args>,
    Args: FnRemote,
{
    ArpyResponse(f.run(args).await)
}

fn mime_type(header_value: Option<&HeaderValue>) -> Result<MimeType, error::Error> {
    if let Some(accept) = header_value {
        let accept = accept.to_str().map_err(ErrorNotAcceptable)?;
        MimeType::from_str(accept)
            .map_err(|_| ErrorUnsupportedMediaType(format!("Unsupport mime type '{accept}'")))
    } else {
        Ok(MimeType::Cbor)
    }
}
