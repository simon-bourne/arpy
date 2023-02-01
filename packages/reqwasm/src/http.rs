use arpy::{FnRemote, MimeType, RpcClient};
use async_trait::async_trait;
use js_sys::Uint8Array;
use reqwasm::http;

use crate::Error;

pub struct Connection(String);

impl Connection {
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
