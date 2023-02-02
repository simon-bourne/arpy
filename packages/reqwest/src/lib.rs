//! Reqwest client for Arpy.
//!
//! See [`Connection`] for an example.
use arpy::{FnRemote, MimeType, RpcClient};
use async_trait::async_trait;
use reqwest::{
    header::{HeaderValue, ACCEPT, CONTENT_TYPE},
    Client,
};
use thiserror::Error;

/// A connection to the server.
///
/// # Example
///
/// ```
/// # use arpy::{FnClient, FnRemote, RpcId};
/// # use reqwest::Client;
/// # use serde::{Deserialize, Serialize};
/// # use arpy_reqwest::Connection;
/// #
/// #[derive(RpcId, Serialize, Deserialize, Debug)]
/// struct MyAdd(u32, u32);
///
/// impl FnRemote for MyAdd {
///     type Output = u32;
/// }
///
/// async {
///     let mut conn = Connection::new(&Client::new(), "http://127.0.0.1:9090/api");
///     let result = MyAdd(1, 2).call(&mut conn).await.unwrap();
///
///     println!("1 + 2 = {result}");
/// };
/// ```
pub struct Connection {
    client: Client,
    url: String,
}

impl Connection {
    /// Constructor.
    ///
    /// This stores [`Client`] for connection pooling. `url` is the base url of
    /// the server, and will have [`RpcId::ID`] appended for each RPC.
    ///
    /// [`RpcId::ID`]: arpy::id::RpcId::ID
    pub fn new(client: &Client, url: impl Into<String>) -> Self {
        Self {
            client: client.clone(),
            url: url.into(),
        }
    }
}

/// The errors that can happen during an RPC.
///
/// Note; This may contain sensitive information. In particular,
/// [`reqwest::Error`] may contain URLs, and `DeserializeResult` may contain
/// argument names/values.
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
    UnknownContentType(HeaderValue),
}

#[async_trait(?Send)]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<Args>(&mut self, args: Args) -> Result<Args::Output, Self::Error>
    where
        Args: FnRemote,
    {
        let content_type = MimeType::Cbor;
        let mut body = Vec::new();

        ciborium::ser::into_writer(&args, &mut body).unwrap();

        let result = self
            .client
            .post(format!("{}/{}", self.url, Args::ID))
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
                return Err(Error::UnknownContentType(result_type.clone()));
            }
        }

        let result_bytes = result.bytes().await.map_err(Error::Receive)?;
        let result: Args::Output = ciborium::de::from_reader(result_bytes.as_ref())
            .map_err(|e| Error::DeserializeResult(e.to_string()))?;

        Ok(result)
    }
}
