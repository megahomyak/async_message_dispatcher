/// This module's utils are "quick solutions" and are not recommended for use in big applications.
/// They are poorly made on purpose.
use std::future::Future;

use async_trait::async_trait;

use crate::{Consumer, Dispatcher, Key};

#[async_trait]
pub trait Message<R> {
    async fn respond(&self, response: R);
}

pub struct Filter<'a, F, K: Key, M> {
    filter: &'a F,
    consumer: &'a mut Consumer<K, M>,
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

impl<'a, K, Response, M, Output, F> Filter<'a, F, K, M>
where
    K: Key,
    M: Message<Response>,
    F: Fn(&mut Consumer<K, M>) -> Result<Output, Response>,
{
    pub fn new(filter: &'a F, consumer: &'a mut Consumer<K, M>) -> Self {
        Self { filter, consumer }
    }

    pub async fn take(&mut self) -> Output {
        loop {
            let message = self.consumer.take().await.unwrap();
            match (self.filter)(&mut self.consumer) {
                Ok(output) => return output,
                Err(error) => message.respond(error).await,
            }
        }
    }
}
