use std::sync::Arc;

use actix::{Actor, ActorContext, StreamHandler};
use actix_web::{
    dev::{ServiceFactory, ServiceRequest},
    web::{self, Bytes},
    App, Error, HttpRequest,
};
use actix_web_actors::ws;
use anyhow::bail;
use arpy_server::WebSocketRouter;
use futures::executor::block_on;

#[derive(Clone)]
pub struct WebSocketHandler {
    handler: Arc<arpy_server::WebSocketHandler>,
    id: Option<Bytes>,
}

impl WebSocketHandler {
    pub fn new(handler: WebSocketRouter) -> Self {
        Self {
            handler: Arc::new(arpy_server::WebSocketHandler::new(handler)),
            id: None,
        }
    }

    fn handle_fallible(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut <Self as Actor>::Context,
    ) -> anyhow::Result<()> {
        match msg? {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Text(_) => bail!("Text messages are not handled"),
            ws::Message::Binary(msg) => match self.id.take() {
                // TODO: How do we avoid blocking?
                Some(id) => ctx.binary(block_on(
                    self.handler.handle_msg(id.as_ref(), msg.as_ref().to_vec()),
                )?),
                None => self.id = Some(msg),
            },
            ws::Message::Continuation(_) => todo!(),
            ws::Message::Pong(_) => (),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Nop => (),
        }

        Ok(())
    }
}

impl Actor for WebSocketHandler {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketHandler {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        if let Err(_e) = self.handle_fallible(msg, ctx) {
            // TODO: Log error
        }
    }
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
            web::get().to(move |req: HttpRequest, stream: web::Payload| {
                let handler = handler.clone();
                async move { ws::start(handler.clone(), &req, stream) }
            }),
        )
    }
}
