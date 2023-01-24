use async_trait::async_trait;
use reqwest::{
    header::{HeaderValue, ACCEPT, CONTENT_TYPE},
    Client, IntoUrl, RequestBuilder,
};
use rpc::{MimeType, RemoteFn, RpcClient};
use thiserror::Error;

pub struct Connection(RequestBuilder);

impl Connection {
    pub fn new(client: &Client, url: impl IntoUrl) -> Self {
        Self(client.post(url))
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't deserialize result: {0}")]
    DeserializeResult(String),
    #[error("Couldn't send request: {0}")]
    Send(reqwest::Error),
    #[error("Couldn't receive response: {0}")]
    Receive(reqwest::Error),
    #[error("Invalid response 'content_type'")]
    InvalidResponseType(HeaderValue),
}

#[async_trait]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<'a, F>(self, function: &'a F) -> Result<F::ResultType, Self::Error>
    where
        F: RemoteFn,
        &'a F: Send,
    {
        let content_type = MimeType::Cbor;
        let mut body = Vec::new();

        ciborium::ser::into_writer(&function, &mut body).unwrap();

        let result = self
            .0
            .header(CONTENT_TYPE, content_type.as_str())
            .header(ACCEPT, content_type.as_str())
            .body(body)
            .send()
            .await
            .map_err(Error::Send)?;

        if let Some(result_type) = result.headers().get(CONTENT_TYPE) {
            if result_type != HeaderValue::from_static(content_type.as_str()) {
                return Err(Error::InvalidResponseType(result_type.clone()));
            }
        }

        let result_bytes = result.bytes().await.map_err(Error::Receive)?;
        let result: F::ResultType = ciborium::de::from_reader(result_bytes.as_ref())
            .map_err(|e| Error::DeserializeResult(e.to_string()))?;

        Ok(result)
    }
}
