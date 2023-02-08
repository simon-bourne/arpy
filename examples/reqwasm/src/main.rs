use arpy::{FnRemote, ServerSentEvents};
use arpy_example_common::{Count, MyFunction, PORT};
use arpy_reqwasm::{eventsource, http};
use futures::StreamExt;
use gloo_console::{error, info};
use wasm_bindgen_futures::spawn_local;

fn main() {
    console_error_panic_hook::set_once();
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
    let conn = http::Connection::new(&format!("http://localhost:{PORT}/http"));

    let greeting = MyFunction("Arpy".to_string()).call(&conn).await?;
    info!(greeting);

    Ok(())
}

async fn sse() -> anyhow::Result<()> {
    info!("Server Sent Events Example");
    let events = eventsource::Connection::new(format!("http://localhost:{PORT}/sse"));
    let mut counter = events.subscribe::<Count>().await?;

    while let Some(i) = counter.next().await {
        info!(i?.0);
    }

    Ok(())
}
