use anyhow::bail;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
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

async fn handler<T: FnRemote + 'static>(ws: WebSocketUpgrade) -> Response
where
    for<'a> &'a T: Send,
{
    ws.on_upgrade(handle_socket::<T>)
}

async fn handle_socket<T: FnRemote>(socket: WebSocket)
where
    for<'a> &'a T: Send,
{
    if let Err(e) = try_handle_socket::<T>(socket).await {
        tracing::error!("Error on WebSocket: {e}");
    }
}

async fn try_handle_socket<T: FnRemote>(mut socket: WebSocket) -> anyhow::Result<()>
where
    T: Send,
    for<'a> &'a T: Send,
{
    while let Some(msg) = socket.recv().await {
        match msg? {
            Message::Text(_) => bail!("Text message type is unsupported"),
            Message::Binary(bytes) => {
                let function: T = ciborium::de::from_reader(bytes.as_slice())?;
                let result = function.run().await;
                let mut body = Vec::new();
                ciborium::ser::into_writer(&result, &mut body).unwrap();
                socket.send(Message::Binary(body)).await?;
            }
            Message::Ping(_) => (),
            Message::Pong(_) => (),
            Message::Close(_) => return Ok(()),
        }
    }

    Ok(())
}
