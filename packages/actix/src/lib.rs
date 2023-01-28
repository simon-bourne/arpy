use std::sync::Arc;

use actix_web::{
    dev::{ServiceFactory, ServiceRequest},
    web::{self, Bytes},
    App, Error, HttpRequest, HttpResponse,
};
use actix_ws::{CloseReason, Message, MessageStream, Session};
use anyhow::bail;
use arpy_server::WebSocketRouter;
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

pub trait RpcApp {
    fn ws_rpc_route(self, path: &str, routes: WebSocketRouter) -> Self;
}

impl<T> RpcApp for App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = Error, InitError = ()>,
{
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
