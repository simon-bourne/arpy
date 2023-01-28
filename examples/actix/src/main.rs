use actix_web::{App, HttpServer};
use arpy_actix::RpcApp;
use arpy_server::WebSocketRouter;
use arpy_test::Add;

async fn add(args: &Add) -> i32 {
    args.0 + args.1
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        let ws = WebSocketRouter::new().handle(add);

        App::new().ws_rpc_route("/ws/", ws)
    })
    .bind(("127.0.0.1", 9090))?
    .run()
    .await
}
