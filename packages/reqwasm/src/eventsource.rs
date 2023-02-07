use std::{
    marker::PhantomData,
    pin::Pin,
    task::{self, Poll},
};

use arpy::{protocol, ServerSentEvents};
use async_trait::async_trait;
use futures::Stream;
use gloo_net::eventsource::futures::{EventSource, EventSourceSubscription};
use pin_project::pin_project;
use serde::de::DeserializeOwned;
use web_sys::MessageEvent;

use crate::Error;

#[derive(Clone)]
pub struct Connection(String);

impl Connection {
    pub fn new(url: impl Into<String>) -> Self {
        Self(url.into())
    }
}

#[async_trait(?Send)]
impl ServerSentEvents for Connection {
    type Error = Error;
    type Output<Item: DeserializeOwned> = SubscriptionMessage<Item>;

    async fn subscribe<T>(&self) -> Result<Self::Output<T>, Self::Error>
    where
        T: DeserializeOwned + protocol::MsgId,
    {
        let subscription = EventSource::new(&self.0)
            .map_err(Error::send)?
            .subscribe(T::ID)
            .map_err(Error::send)?;

        Ok(SubscriptionMessage {
            subscription,
            phantom: PhantomData,
        })
    }
}

#[pin_project]
pub struct SubscriptionMessage<Item> {
    #[pin]
    subscription: EventSourceSubscription,
    phantom: PhantomData<Item>,
}

impl<Item: DeserializeOwned> Stream for SubscriptionMessage<Item> {
    type Item = Result<Item, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().subscription.poll_next(cx).map(|result| {
            result.map(|result| {
                let (_id, msg) = result.map_err(Error::receive)?;
                deserialize_message(&msg)
            })
        })
    }
}

fn deserialize_message<T: DeserializeOwned>(msg: &MessageEvent) -> Result<T, Error> {
    serde_json::from_str(
        &msg.data()
            .as_string()
            .ok_or_else(|| Error::deserialize_result("No message data"))?,
    )
    .map_err(Error::deserialize_result)
}
