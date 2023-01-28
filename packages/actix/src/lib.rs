use std::{convert::identity, str::FromStr, sync::Arc};

use actix_web::{
    dev::{ServiceFactory, ServiceRequest},
    http::header::{HeaderValue, ACCEPT, CONTENT_TYPE},
    web::{self, Bytes},
    App, Error, HttpRequest, HttpResponse,
};
use actix_ws::{CloseReason, Message, MessageStream, Session};
use anyhow::bail;
use arpy::{FnRemote, MimeType};
use arpy_server::{FnRemoteBody, WebSocketRouter};
use futures::StreamExt;

#[derive(Clone)]
pub struct WebSocketHandler(Arc<arpy_server::WebSocketHandler>);

impl WebSocketHandler {
    pub fn new(handler: WebSocketRouter) -> Self {
        Self(Arc::new(arpy_server::WebSocketHandler::new(handler)))
    }

    pub async fn handle(
        self,
        mut session: Session,
        mut msg_stream: MessageStream,
    ) -> anyhow::Result<()> {
        loop {
            let id = match Self::next_msg(&mut session, &mut msg_stream).await? {
                Msg::Close(reason) => {
                    session.close(reason).await?;
                    return Ok(());
                }
                Msg::Msg(id) => id,
            };
            let body = match Self::next_msg(&mut session, &mut msg_stream).await? {
                Msg::Close(_) => bail!("Expect RPC body, got close"),
                Msg::Msg(body) => body,
            };

            let reply = self
                .0
                .handle_msg(id.as_ref(), body.as_ref().to_vec())
                .await?;

            session.binary(reply).await?;
        }
    }

    async fn next_msg(
        session: &mut Session,
        msg_stream: &mut MessageStream,
    ) -> anyhow::Result<Msg> {
        while let Some(msg) = msg_stream.next().await {
            match msg? {
                Message::Ping(bytes) => session.pong(&bytes).await?,
                Message::Text(_) => bail!("`Text` messages are unsupported"),
                Message::Binary(msg) => return Ok(Msg::Msg(msg)),
                Message::Continuation(_) => bail!("`Continuation` messages are unsupported"),
                Message::Pong(_) => (),
                Message::Close(reason) => return Ok(Msg::Close(reason)),
                Message::Nop => (),
            }
        }

        Ok(Msg::Close(None))
    }
}

enum Msg {
    Close(Option<CloseReason>),
    Msg(Bytes),
}

async fn ws(
    handler: WebSocketHandler,
    req: HttpRequest,
    body: web::Payload,
) -> Result<HttpResponse, Error> {
    let (response, session, msg_stream) = actix_ws::handle(&req, body)?;

    actix_rt::spawn(handler.handle(session, msg_stream));

    Ok(response)
}

async fn http<F, T>(
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

pub trait RpcApp {
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static;

    fn ws_rpc_route(self, path: &str, routes: WebSocketRouter) -> Self;
}

impl<S> RpcApp for App<S>
where
    S: ServiceFactory<ServiceRequest, Config = (), Error = Error, InitError = ()>,
{
    fn http_rpc_route<F, T>(self, prefix: &str, f: F) -> Self
    where
        F: FnRemoteBody<T> + Send + Sync + 'static,
        T: FnRemote + Send + Sync + 'static,
    {
        let id = T::ID;
        let f = Arc::new(f);

        self.route(
            &format!("{prefix}/{id}"),
            web::post().to(move |req, body| {
                let f = f.clone();
                async move {
                    http(f.clone(), req, body)
                        .await
                        .map_or_else(identity, identity)
                }
            }),
        )
    }

    fn ws_rpc_route(self, path: &str, routes: WebSocketRouter) -> Self {
        let handler = WebSocketHandler::new(routes);
        self.route(
            path,
            web::get().to(move |req: HttpRequest, body: web::Payload| {
                let handler = handler.clone();
                ws(handler, req, body)
            }),
        )
    }
}
