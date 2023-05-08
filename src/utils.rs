/// This module's utils are "quick solutions" and are not recommended for use in big applications.
/// They are poorly made on purpose.
use std::future::Future;

use async_trait::async_trait;

use crate::{Consumer, Dispatcher, Key};

#[async_trait]
pub trait Message {
    async fn respond(&self, text: &str);
}

pub struct Filter<F, K: Key, M> {
    filter: F,
    consumer: Consumer<K, M>,
    error_message: &'static str,
}

pub trait AutoHandler<K, M, F> {
    fn handle(&mut self, key: K, message: M, handler: F);
}

impl<K, M, Fut, F> AutoHandler<K, M, F> for Dispatcher<K, M>
where
    K: Key,
    Fut: Future + Send + 'static,
    F: FnOnce(Consumer<K, M>) -> Fut,
    <Fut as Future>::Output: Send,
{
    fn handle(&mut self, key: K, message: M, handler: F) {
        use crate::ConsumerState::{Free, Taken};
        match self.notify(key, message) {
            Free(consumer) => {
                tokio::spawn(handler(consumer));
            }
            Taken => (),
        }
    }
}

impl<K: Key, M: Message, F: Fn(&mut Consumer<K, M>) -> bool> Filter<F, K, M> {
    pub fn new(filter: F, consumer: Consumer<K, M>, error_message: &'static str) -> Self {
        Self {
            filter,
            consumer,
            error_message,
        }
    }

    pub async fn take(&mut self) -> M {
        loop {
            let message = self.consumer.take().await.unwrap();
            if (self.filter)(&mut self.consumer) {
                return message;
            } else {
                message.respond(self.error_message).await;
            }
        }
    }
}
