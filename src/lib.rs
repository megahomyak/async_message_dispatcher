use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    hash::Hash,
    sync::{Arc, Mutex, Weak},
    task::{Poll, Waker},
};

pub trait Key: Eq + Hash + Copy {}
impl<T: Eq + Hash + Copy> Key for T {}

pub struct Waiter<'a, K: Key, M> {
    consumer: &'a mut Consumer<K, M>,
}

pub struct Consumer<K: Key, M> {
    contexts: Weak<Mutex<HashMap<K, Context<K, M>>>>,
    key: K,
}

impl<K: Key, M> Consumer<K, M> {
    pub fn take(&mut self) -> Waiter<K, M> {
        Waiter { consumer: self }
    }
}

#[derive(Debug)]
pub enum AwaitingError {
    DispatcherWasDropped,
}

impl<'a, K: Key, M> Future for Waiter<'a, K, M> {
    type Output = Result<M, AwaitingError>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let Some(contexts) = self.consumer.contexts.upgrade() else {
            return Poll::Ready(Err(AwaitingError::DispatcherWasDropped));
        };
        let mut contexts = contexts.lock().unwrap();
        let context = contexts.get_mut(&self.consumer.key).unwrap();
        if let Some(message) = context.queue.pop_front() {
            context.execution_state = ExecutionState::Running(MessageAwaitingState::NotWaiting);
            return Poll::Ready(Ok(message));
        }
        let waker = cx.waker().clone();
        context.execution_state = ExecutionState::Running(MessageAwaitingState::Waiting(waker));
        Poll::Pending
    }
}

impl<K: Key, M> Drop for Consumer<K, M> {
    fn drop(&mut self) {
        if let Some(contexts) = self.contexts.upgrade() {
            let mut contexts_guard = contexts.lock().unwrap();
            let context = contexts_guard.get_mut(&self.key).unwrap();
            if context.queue.is_empty() {
                contexts_guard.remove(&self.key);
            } else {
                context.execution_state = ExecutionState::NotRunning(Consumer {
                    key: self.key,
                    contexts: self.contexts.clone(),
                })
            }
        }
    }
}

enum MessageAwaitingState {
    Waiting(Waker),
    NotWaiting,
}

/// Taking the old and the new states, returning the old one while updating the old to the new
fn swap<T>(old_state: &mut T, mut new_state: T) -> T {
    std::mem::swap(&mut new_state, old_state);
    let old_state = new_state;
    old_state
}

enum ExecutionState<K: Key, M> {
    NotRunning(Consumer<K, M>),
    Running(MessageAwaitingState),
}

struct Context<K: Key, M> {
    queue: VecDeque<M>,
    execution_state: ExecutionState<K, M>,
}

pub enum ConsumerState<K: Key, M> {
    Free(Consumer<K, M>),
    Taken,
}

pub struct Storage<K: Key, M> {
    contexts: Arc<Mutex<HashMap<K, Context<K, M>>>>,
}

impl<K: Key, M> Storage<K, M> {
    pub fn new() -> Self {
        Self {
            contexts: Default::default(),
        }
    }

    pub fn add(&self, key: K, message: M) -> ConsumerState<K, M> {
        let mut contexts = self.contexts.lock().unwrap();
        let context = contexts.entry(key).or_insert_with(|| Context {
            queue: VecDeque::new(),
            execution_state: ExecutionState::NotRunning(Consumer {
                contexts: Arc::downgrade(&self.contexts),
                key,
            }),
        });
        context.queue.push_back(message);
        match swap(
            &mut context.execution_state,
            ExecutionState::Running(MessageAwaitingState::NotWaiting),
        ) {
            ExecutionState::Running(message_awaiting_state) => {
                match message_awaiting_state {
                    MessageAwaitingState::NotWaiting => (),
                    MessageAwaitingState::Waiting(waker) => waker.wake(),
                }
                ConsumerState::Taken
            }
            ExecutionState::NotRunning(consumer) => ConsumerState::Free(consumer),
        }
    }
}

pub struct Dispatcher<K: Key, M, F> {
    handler: F,
    storage: Storage<K, M>,
}

impl<K: Key, M, F: Fn(Consumer<K, M>)> Dispatcher<K, M, F> {
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            storage: Storage::new(),
        }
    }

    pub fn storage(&self) -> &Storage<K, M> {
        &self.storage
    }

    pub fn dispatch(&self, key: K, message: M) {
        match self.storage.add(key, message) {
            ConsumerState::Free(consumer) => (self.handler)(consumer),
            ConsumerState::Taken => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    async fn dummy_handler<K: Key, M: Hash + Eq>(mut consumer: Consumer<K, M>) -> HashSet<M> {
        let mut values = HashSet::new();
        for _ in 0..3 {
            values.insert(consumer.take().await.unwrap());
        }
        values
    }

    /// # Soundness
    ///
    /// I don't know whether or not this test covers everything and even if it is written right, I
    /// only *hope* so.
    #[test]
    fn it_works() {
        let storage = Storage::new();
        for _ in 0..2 {
            let consumer_1 = match storage.add(1, "1: 1") {
                ConsumerState::Free(consumer) => consumer,
                ConsumerState::Taken => unreachable!(),
            };
            assert!(matches!(storage.add(1, "1: 3"), ConsumerState::Taken));
            let consumer_2 = match storage.add(2, "2: 1") {
                ConsumerState::Free(consumer) => consumer,
                ConsumerState::Taken => unreachable!(),
            };
            assert!(matches!(storage.add(2, "2: 2"), ConsumerState::Taken));
            assert!(matches!(storage.add(1, "1: 2"), ConsumerState::Taken));
            assert!(matches!(storage.add(2, "2: 3"), ConsumerState::Taken));
            let (result_1, result_2) = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
                .block_on(async move {
                    tokio::join!(dummy_handler(consumer_1), dummy_handler(consumer_2))
                });
            assert!(storage.contexts.lock().unwrap().is_empty());
            assert_eq!(result_1, HashSet::from(["1: 1", "1: 2", "1: 3"]));
            assert_eq!(result_2, HashSet::from(["2: 1", "2: 2", "2: 3"]));
        }
    }
}
