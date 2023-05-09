/// This module's utils are "quick solutions" and are not recommended for use in big applications.
/// They are poorly made on purpose.
use std::future::Future;

use async_trait::async_trait;

use crate::{Consumer, Dispatcher, Key};

#[async_trait]
pub trait Message<R> {
    async fn respond(&self, response: R);
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

pub async fn take_filtered<O, F, R, K, M>(consumer: &mut Consumer<K, M>, filter: &F) -> O
where
    F: Fn(&mut Consumer<K, M>) -> Result<O, R>,
    K: Key,
    M: Message<R>,
{
    loop {
        let message = consumer.take().await.unwrap();
        match filter(consumer) {
            Ok(output) => return output,
            Err(error) => message.respond(error).await,
        }
    }
}
