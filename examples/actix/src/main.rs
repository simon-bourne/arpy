use actix_web::{middleware::Logger, App, HttpServer};
use arpy_actix::RpcApp;
use arpy_example_common::{my_fallible_function, my_function, PORT};
use arpy_server::WebSocketRouter;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    HttpServer::new(move || {
        let ws = WebSocketRouter::new()
            .handle(my_function)
            .handle(my_fallible_function);

        App::new()
            .wrap(Logger::default())
            .ws_rpc_route("ws", ws)
            .http_rpc_route("http", my_function)
            .http_rpc_route("http", my_fallible_function)
    })
    .bind(("127.0.0.1", PORT))?
    .run()
    .await
}
