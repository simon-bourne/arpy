use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    response::Response,
    routing::{get, MethodRouter},
};
use rpc::FnRemote;

pub fn handle_rpc<T: FnRemote + 'static>() -> MethodRouter
where
    for<'a> &'a T: Send,
{
    get(handler::<T>)
}

async fn handler<T: FnRemote + 'static>(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket::<T>)
}

async fn handle_socket<T: FnRemote>(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        // TODO: Implement RPC
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            // client disconnected
            return;
        };

        if socket.send(msg).await.is_err() {
            // client disconnected
            return;
        }
    }
}
