# This library's purpose

Usually, when people want to write a textual bot for a social network, they use states as an explicit, separate entity:

```
enum UserState {
    Beginning,
    WaitingForAge,
    WaitingForName { age: String },
    WaitingForBio { age: String, name: String },
}

impl Handler {
    pub fn handle_message(&mut self, message: Message) -> &'static str {
        let state = self.states.entry(message.user_id).or_insert(UserState::Beginning);
        let (response, new_state) = match state {
            UserState::Beginning => ("Please, enter your age.", UserState::WaitingForAge),
            UserState::WaitingForAge => ("Now, enter your name.", UserState::WaitingForName { age: message.text }),
            UserState::WaitingForName { age } => ("Now, enter your bio.", UserState::WaitingForBio { age, name: message.text }),
            UserState::WaitingForBio { age, name } => {
                self.add_user(age, name, message.text);
                ("You're registered successfully!", UserState::Beginning)
            },
        };
        if let UserState::Beginning = state {
            states.remove(message.user_id);
        } else {
            *state = new_state;
        };
        response
    }
}
```

Even if used with a specialized library, this still requires the user to write an `enum` for that, and the enum must contain repetition if the data needs to be taken to the end. Such approach is very noisy.

This library allows you to write something like this:

```
impl Handler {
    fn handle_message(&self, message: Message) -> String {
        use async_message_dispatcher::ConsumerState::{Free, Taken};
        match self.dispatcher.notify(message.user_id, message) {
            Free(consumer) => {
                tokio::spawn(async move {
                    let message = consumer.take().await.unwrap();
                    message.respond("Please, enter your age.").await;
                    let age = consumer.take().await.unwrap();
                    age.respond("Now, enter your name.").await;
                    let name = consumer.take().await.unwrap();
                    name.respond("Now, enter your bio.").await;
                    let bio = consumer.take().await.unwrap();
                    self.add_user(age.text, name.text, bio.text);
                    bio.respond("You're registered successfully!.").await;
                });
            }
            Taken => (),
        }
    }
}
```

Such approach gives code that is much more readable, especially when there are many states. Also, if you want, it's pretty easy to add message filtering:

```
async fn get_number<K: Key, M>(consumer: Consumer<K, M>) -> i64 {
    loop {
        let message = consumer.take().await;
        let number: Ok(i64) = message.text.parse() else {
            message.respond("Please, enter a number.").await;
            continue;
        };
    }
}
```

Also, if you don't want to write boring abstractions yourself, you can use `utils` from this module (you need to enable them as a feature first). For example, you can easily run the handler of a new message:

```
utils::handle(&mut dispatcher, message.user_id, message, |consumer| async move {
    // Your logic here
});
```

Or, you can easily make a filter:

```
let filter = utils::Filter::new(|message| message.text != "", &mut consumer, "Your message shouldn't be empty");
```

Be aware that the `utils` module is not very flexible.

# Usage

Create a `Dispatcher`, `.notify()` it about new messages, and use the returned `Consumer` (if it is `Free`) in message handlers.
