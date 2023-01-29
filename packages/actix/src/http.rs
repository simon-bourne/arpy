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

pub struct ArpyRequest<T>(pub T);

impl<T: FnRemote> FromRequest for ArpyRequest<T> {
    type Error = error::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut actix_web::dev::Payload) -> Self::Future {
        let content_type = mime_type(req.headers().get(CONTENT_TYPE)).unwrap();
        let bytes = Bytes::from_request(req, payload);

        Box::pin(async move {
            let body = bytes.await?;
            let body = body.as_ref();

            let thunk: T = match content_type {
                MimeType::Cbor => ciborium::de::from_reader(body).map_err(ErrorBadRequest)?,
                MimeType::Json => serde_json::from_slice(body).map_err(ErrorBadRequest)?,
            };

            Ok(ArpyRequest(thunk))
        })
    }
}

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

    let response = match response_type {
        MimeType::Cbor => {
            let mut response_body = Vec::new();

            ciborium::ser::into_writer(&response, &mut response_body).map_err(ErrorBadRequest)?;

            HttpResponse::Ok()
                .content_type(MimeType::Cbor.as_str())
                .body(response_body)
        }
        MimeType::Json => {
            let response_body = serde_json::to_vec(&response).map_err(ErrorBadRequest)?;

            HttpResponse::Ok()
                .content_type(MimeType::Json.as_str())
                .body(response_body)
        }
    };

    Ok(response)
}

pub async fn handler<F, T>(f: Arc<F>, ArpyRequest(thunk): ArpyRequest<T>) -> impl Responder
where
    F: FnRemoteBody<T>,
    T: FnRemote,
{
    ArpyResponse(f.run(thunk).await)
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
