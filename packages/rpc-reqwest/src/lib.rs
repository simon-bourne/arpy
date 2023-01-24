use async_trait::async_trait;
use reqwest::{header::ACCEPT, Client};
use rpc::{RemoteFn, RpcClient};
use thiserror::Error;

pub struct Connection(reqwest::RequestBuilder);

impl Connection {
    pub fn new(client: &Client, url: &str) -> Self {
        Self(client.post(url))
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't deserialize result: {0}")]
    DeserializeResult(String),
    #[error("Error sending request: {0}")]
    Send(reqwest::Error),
    #[error("Error receiving response: {0}")]
    Receive(reqwest::Error),
}

#[async_trait]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<'a, F>(self, function: &'a F) -> Result<F::ResultType, Self::Error>
    where
        F: RemoteFn,
        &'a F: Send,
    {
        let mut body = Vec::new();

        ciborium::ser::into_writer(&function, &mut body).unwrap();

        // TODO: Accept and content_type headers
        let result = self
            .0
            .header(ACCEPT, "application/cbor")
            .body(body)
            .send()
            .await
            .map_err(Error::Send)?;

        println!("{}", result.status());
        let result_bytes = result.bytes().await.map_err(Error::Receive)?;
        let result: F::ResultType = ciborium::de::from_reader(result_bytes.as_ref())
            .map_err(|e| Error::DeserializeResult(e.to_string()))?;

        Ok(result)
    }
}
