//! HTTP Client.
//!
//! See [`Connection`] for an example.
use arpy::{FnRemote, MimeType, RpcClient};
use async_trait::async_trait;
use js_sys::Uint8Array;
use reqwasm::http;

use crate::Error;

/// A connection to the server.
///
/// # Example
///
/// ```
/// # use arpy::{FnClient, FnRemote, RpcId};
/// # use serde::{Deserialize, Serialize};
/// # use arpy_reqwasm::http::Connection;
/// #
/// #[derive(RpcId, Serialize, Deserialize, Debug)]
/// struct MyAdd(u32, u32);
///
/// impl FnRemote for MyAdd {
///     type Output = u32;
/// }
///
/// async {
///     let conn = Connection::new("http://127.0.0.1:9090/api");
///     let result = MyAdd(1, 2).call(&conn).await.unwrap();
///
///     println!("1 + 2 = {result}");
/// };
/// ```
pub struct Connection(String);

impl Connection {
    /// Constructor.
    ///
    /// `url` is the base url of the server, and will have [`RpcId::ID`]
    /// appended for each RPC.
    ///
    /// [`RpcId::ID`]: arpy::id::RpcId::ID
    pub fn new(url: &str) -> Self {
        Self(url.to_string())
    }
}

#[async_trait(?Send)]
impl RpcClient for Connection {
    type Error = Error;

    async fn call<Args>(&self, args: Args) -> Result<Args::Output, Self::Error>
    where
        Args: FnRemote,
    {
        let content_type = MimeType::Cbor;
        let mut body = Vec::new();
        ciborium::ser::into_writer(&args, &mut body).unwrap();

        let js_body = Uint8Array::new_with_length(body.len().try_into().unwrap());
        js_body.copy_from(&body);

        let result = http::Request::post(&format!("{}/{}", self.0, Args::ID))
            .header(CONTENT_TYPE, content_type.as_str())
            .header(ACCEPT, content_type.as_str())
            .body(js_body)
            .send()
            .await
            .map_err(Error::send)?;

        if !result.ok() {
            return Err(Error::Receive(format!(
                "HTTP error code {}",
                result.status()
            )));
        }

        if let Some(result_type) = result.headers().get(CONTENT_TYPE) {
            if result_type != content_type.as_str() {
                return Err(Error::UnknownContentType(result_type));
            }
        }

        let result_bytes = result.binary().await.map_err(Error::receive)?;
        let result: Args::Output = ciborium::de::from_reader(result_bytes.as_slice())
            .map_err(Error::deserialize_result)?;

        Ok(result)
    }
}

const ACCEPT: &str = "accept";
const CONTENT_TYPE: &str = "content-type";
