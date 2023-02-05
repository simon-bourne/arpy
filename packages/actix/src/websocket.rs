use std::{future::ready, ops::ControlFlow, sync::Arc};

use actix_web::{
    web::{self, Bytes},
    Error, HttpRequest, HttpResponse,
};
use actix_ws::{Closed, Message, MessageStream, ProtocolError, Session};
use arpy_server::WebSocketRouter;
use futures::{sink::unfold, StreamExt};

#[derive(Clone)]
pub struct WebSocketHandler(Arc<arpy_server::WebSocketHandler>);

impl WebSocketHandler {
    pub fn new(handler: WebSocketRouter) -> Self {
        Self(arpy_server::WebSocketHandler::new(handler))
    }

    pub async fn handle(self, mut session: Session, incoming: MessageStream) -> Result<(), Closed> {
        let incoming = incoming
            .filter_map(Self::handle_incoming)
            .take_while(|msg| ready(msg.is_continue()))
            .filter_map(|msg| {
                ready(match msg {
                    ControlFlow::Continue(msg) => Some(msg),
                    ControlFlow::Break(()) => None,
                })
            });
        let outgoing = Box::pin(unfold(&mut session, |outgoing, msg: Bytes| async {
            outgoing.binary(msg).await?;
            Result::<&mut Session, Closed>::Ok(outgoing)
        }));

        if let Err(e) = self.0.handle_socket(outgoing, incoming).await {
            tracing::error!("Error on WebSocket: {e}");
        }

        session.close(None).await?;

        Ok(())
    }

    async fn handle_incoming(
        msg: Result<Message, ProtocolError>,
    ) -> Option<ControlFlow<(), Bytes>> {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!("Error on WebSocket: {e}");
                return Some(ControlFlow::Break(()));
            }
        };

        match msg {
            Message::Text(_) => {
                tracing::error!("Text message type is unsupported");
                Some(ControlFlow::Break(()))
            }
            Message::Binary(params) => Some(ControlFlow::Continue(params)),
            Message::Continuation(_) => {
                tracing::error!("`Continuation` messages are unsupported");
                Some(ControlFlow::Break(()))
            }
            Message::Ping(_) => None,
            Message::Pong(_) => None,
            Message::Close(_) => Some(ControlFlow::Break(())),
            Message::Nop => None,
        }
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
