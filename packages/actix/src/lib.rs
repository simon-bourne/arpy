use std::{convert::identity, sync::Arc};

use actix_web::{
    dev::{ServiceFactory, ServiceRequest},
    web::{self},
    App, Error, HttpRequest,
};
use arpy::FnRemote;
use arpy_server::{FnRemoteBody, WebSocketRouter};
use websocket::WebSocketHandler;

mod http;

mod websocket;

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
                    http::handler(f.clone(), req, body)
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
                websocket::handler(handler, req, body)
            }),
        )
    }
}
