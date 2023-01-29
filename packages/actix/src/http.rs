use std::{convert::identity, str::FromStr, sync::Arc};

use actix_web::{
    body::BoxBody,
    http::header::{HeaderValue, ACCEPT, CONTENT_TYPE},
    web::{self},
    HttpRequest, HttpResponse, Responder,
};
use arpy::{FnRemote, MimeType};
use arpy_server::FnRemoteBody;
use serde::Serialize;

pub struct ArpyResponse<T>(T);

impl<T: Serialize> Responder for ArpyResponse<T> {
    type Body = BoxBody;

    fn respond_to(self, req: &HttpRequest) -> HttpResponse<Self::Body> {
        try_respond_to(self.0, req).map_or_else(identity, identity)
    }
}

fn try_respond_to<T>(response: T, req: &HttpRequest) -> Result<HttpResponse, HttpResponse>
where
    T: Serialize,
{
    let response_type = mime_type(req.headers().get(ACCEPT))?;

    let response = match response_type {
        MimeType::Cbor => {
            let mut response_body = Vec::new();

            ciborium::ser::into_writer(&response, &mut response_body)
                .map_err(|_| HttpResponse::BadRequest().finish())?;

            HttpResponse::Ok()
                .content_type(MimeType::Cbor.as_str())
                .body(response_body)
        }
        MimeType::Json => {
            let response_body =
                serde_json::to_vec(&response).map_err(|_| HttpResponse::BadRequest().finish())?;

            HttpResponse::Ok()
                .content_type(MimeType::Json.as_str())
                .body(response_body)
        }
    };

    Ok(response)
}

pub async fn handler<F, T>(f: Arc<F>, req: HttpRequest, body: web::Bytes) -> impl Responder
where
    F: FnRemoteBody<T> + Send + Sync + 'static,
    T: FnRemote + Send + Sync + 'static,
{
    try_handler(f.clone(), &req, body)
        .await
        .map_or_else(identity, |res| res.respond_to(&req))
}

async fn try_handler<F, T>(
    f: Arc<F>,
    req: &HttpRequest,
    body: web::Bytes,
) -> Result<ArpyResponse<T::Output>, HttpResponse>
where
    F: FnRemoteBody<T> + Send + Sync + 'static,
    T: FnRemote + Send + Sync + 'static,
{
    let body = body.as_ref();
    let headers = req.headers();
    let content_type = mime_type(headers.get(CONTENT_TYPE))?;

    let thunk: T = match content_type {
        MimeType::Cbor => {
            ciborium::de::from_reader(body).map_err(|_| HttpResponse::BadRequest().finish())?
        }
        MimeType::Json => {
            serde_json::from_slice(body).map_err(|_| HttpResponse::BadRequest().finish())?
        }
    };

    Ok(ArpyResponse(f.run(thunk).await))
}

// TODO: Bodies for http errors (call `.body` instead of `.finish`)
fn mime_type(header_value: Option<&HeaderValue>) -> Result<MimeType, HttpResponse> {
    if let Some(accept) = header_value {
        MimeType::from_str(
            accept
                .to_str()
                .map_err(|_| HttpResponse::NotAcceptable().finish())?,
        )
        .map_err(|_| HttpResponse::UnsupportedMediaType().finish())
    } else {
        Ok(MimeType::Cbor)
    }
}
