/// This module's utils are "quick solutions" and are not recommended for use in big applications.
/// They are poorly made on purpose.
use std::future::Future;

use async_trait::async_trait;

use crate::{Consumer, Dispatcher, Key};

#[async_trait]
pub trait Message {
    async fn respond(&self, text: &str);
}

pub struct Filter<'a, F, K: Key, M> {
    filter: F,
    consumer: &'a mut Consumer<K, M>,
    error_message: &'a str,
}

pub fn handle<K, M, Fut, F>(dispatcher: &Dispatcher<K, M>, key: K, message: M, handler: F)
where
    K: Key,
    Fut: Future + Send + 'static,
    F: FnOnce(Consumer<K, M>) -> Fut,
    <Fut as Future>::Output: Send,
{
    use crate::ConsumerState::{Free, Taken};
    match dispatcher.notify(key, message) {
        Free(consumer) => {
            tokio::spawn(handler(consumer));
        }
        Taken => (),
    }
}

impl<'a, K: Key, M: Message, Output, F: Fn(&mut Consumer<K, M>) -> Option<Output>>
    Filter<'a, F, K, M>
{
    pub fn new(filter: F, consumer: &'a mut Consumer<K, M>, error_message: &'a str) -> Self {
        Self {
            filter,
            consumer,
            error_message,
        }
    }

    pub async fn take(&mut self) -> Output {
        loop {
            let message = self.consumer.take().await.unwrap();
            match (self.filter)(&mut self.consumer) {
                Some(output) => return output,
                None => message.respond(self.error_message).await,
            }
        }
    }
}
