use async_trait::async_trait;
use js_sys::Uint8Array;
use reqwasm::http;
use rpc::{FnRemote, MimeType, RpcClient, RpcId};

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

    async fn call<F>(&mut self, function: &F) -> Result<F::Output, Self::Error>
    where
        F: FnRemote + RpcId,
    {
        let content_type = MimeType::Cbor;
        let mut body = Vec::new();

        ciborium::ser::into_writer(&function, &mut body).unwrap();

        let js_body = Uint8Array::new_with_length(body.len() as u32);
        js_body.copy_from(&body);

        let result = http::Request::post(&format!("{}/{}", self.0, F::ID))
            .header(CONTENT_TYPE, content_type.as_str())
            .header(ACCEPT, content_type.as_str())
            .body(js_body)
            .send()
            .await
            .map_err(Error::send)?;

        if let Some(result_type) = result.headers().get(CONTENT_TYPE) {
            if result_type != content_type.as_str() {
                return Err(Error::InvalidResponseType(result_type));
            }
        }

        let result_bytes = result.binary().await.map_err(Error::receive)?;
        let result: F::Output = ciborium::de::from_reader(result_bytes.as_slice())
            .map_err(Error::deserialize_result)?;

        Ok(result)
    }
}

const ACCEPT: &str = "accept";
const CONTENT_TYPE: &str = "content-type";
