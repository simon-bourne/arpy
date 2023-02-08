use arpy::{FnRemote, ServerSentEvents};
use arpy_example_common::{MyFunction, Name};
use arpy_reqwasm::{eventsource, http};
use futures::StreamExt;
use gloo_console::{error, info};
use wasm_bindgen_futures::spawn_local;

fn main() {
    spawn_local(async {
        if let Err(err) = app().await {
            error!(err.to_string());
        }
    });
}

async fn app() -> anyhow::Result<()> {
    http().await?;
    sse().await
}

async fn http() -> anyhow::Result<()> {
    info!("HTTP Example");
    let conn = http::Connection::new("/api/http");

    let greeting = MyFunction("Arpy".to_string()).call(&conn).await?;
    info!(greeting);

    Ok(())
}

async fn sse() -> anyhow::Result<()> {
    info!("Server Sent Events Example");
    error!("This is a work in progress and isn't currently working");
    let events = eventsource::Connection::new("/api/sse");
    let mut names = events.subscribe::<Name>().await?;

    while let Some(name) = names.next().await {
        info!(name?.0);
    }

    Ok(())
}
