use std::{
    marker::PhantomData,
    pin::Pin,
    task::{self, Poll},
};

use arpy::SubscriptionClient;
use async_trait::async_trait;
use futures::Stream;
use gloo_net::eventsource::futures::{EventSource, EventSourceSubscription};
use pin_project::pin_project;
use serde::de::DeserializeOwned;
use web_sys::MessageEvent;

use crate::Error;

pub struct Connection {
    url: String,
}

#[async_trait(?Send)]
impl SubscriptionClient for Connection {
    type Error = Error;
    type Output<Item: DeserializeOwned> = SubscriptionMessage<Item>;

    async fn subscribe<T>(&self, event_type: &str) -> Result<Self::Output<T>, Self::Error>
    where
        T: DeserializeOwned,
    {
        let subscription = EventSource::new(&self.url)
            .map_err(Error::send)?
            .subscribe(event_type)
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
        match self.project().subscription.poll_next(cx) {
            Poll::Ready(result) => Poll::Ready(result.map(|result| match result {
                Ok((_id, msg)) => deserialize_message(&msg),
                Err(e) => Err(Error::receive(e)),
            })),
            Poll::Pending => Poll::Pending,
        }
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
