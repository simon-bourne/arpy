use std::sync::Arc;

use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::{Message, MessageStream, Session};
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
        while let Some(msg) = msg_stream.next().await {
            match msg? {
                Message::Ping(bytes) => session.pong(&bytes).await?,
                Message::Text(_) => bail!("`Text` messages are unsupported"),
                Message::Binary(body) => {
                    let reply = self.0.handle_msg(body.as_ref()).await?;

                    session.binary(reply).await?;
                }
                Message::Continuation(_) => bail!("`Continuation` messages are unsupported"),
                Message::Pong(_) => (),
                Message::Close(reason) => {
                    session.close(reason).await?;
                    return Ok(());
                }
                Message::Nop => (),
            }
        }

        session.close(None).await?;

        Ok(())
    }
}

pub async fn handler(
    handler: WebSocketHandler,
    req: HttpRequest,
    body: web::Payload,
) -> Result<HttpResponse, Error> {
    let (response, session, msg_stream) = actix_ws::handle(&req, body)?;

    actix_rt::spawn(handler.handle(session, msg_stream));

    Ok(response)
}
