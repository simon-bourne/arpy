use std::{str::FromStr, sync::Arc};

use actix_web::{
    http::header::{HeaderValue, ACCEPT, CONTENT_TYPE},
    web::{self},
    HttpRequest, HttpResponse,
};
use arpy::{FnRemote, MimeType};
use arpy_server::FnRemoteBody;

pub async fn handler<F, T>(
    f: Arc<F>,
    req: HttpRequest,
    body: web::Bytes,
) -> Result<HttpResponse, HttpResponse>
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

    let response = f.run(thunk).await;
    let response_type = mime_type(headers.get(ACCEPT))?;

    let response = match response_type {
        MimeType::Cbor => {
            let mut respose_body = Vec::new();

            ciborium::ser::into_writer(&response, &mut respose_body)
                .map_err(|_| HttpResponse::BadRequest().finish())?;

            HttpResponse::Ok()
                .content_type(MimeType::Cbor.as_str())
                .body(respose_body)
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
