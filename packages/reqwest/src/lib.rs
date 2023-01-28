use arpy::{FnRemote, MimeType, RpcClient};
use async_trait::async_trait;
use reqwest::{
    header::{HeaderValue, ACCEPT, CONTENT_TYPE},
    Client,
};
use thiserror::Error;

pub struct Connection<'a> {
    client: &'a Client,
    url: String,
}

impl<'a> Connection<'a> {
    pub fn new(client: &'a Client, url: impl Into<String>) -> Self {
        Self {
            client,
            url: url.into(),
        }
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
    #[error("HTTP error code: {0}")]
    Http(reqwest::StatusCode),
    #[error("Invalid response 'content_type'")]
    InvalidResponseType(HeaderValue),
}

#[async_trait(?Send)]
impl<'a> RpcClient for Connection<'a> {
    type Error = Error;

    async fn call<F>(&mut self, function: F) -> Result<F::Output, Self::Error>
    where
        F: FnRemote,
    {
        let content_type = MimeType::Cbor;
        let mut body = Vec::new();

        ciborium::ser::into_writer(&function, &mut body).unwrap();

        let result = self
            .client
            .post(format!("{}/{}", self.url, F::ID))
            .header(CONTENT_TYPE, content_type.as_str())
            .header(ACCEPT, content_type.as_str())
            .body(body)
            .send()
            .await
            .map_err(Error::Send)?;

        let status = result.status();

        if !status.is_success() {
            return Err(Error::Http(status));
        }

        if let Some(result_type) = result.headers().get(CONTENT_TYPE) {
            if result_type != HeaderValue::from_static(content_type.as_str()) {
                return Err(Error::InvalidResponseType(result_type.clone()));
            }
        }

        let result_bytes = result.bytes().await.map_err(Error::Receive)?;
        let result: F::Output = ciborium::de::from_reader(result_bytes.as_ref())
            .map_err(|e| Error::DeserializeResult(e.to_string()))?;

        Ok(result)
    }
}
