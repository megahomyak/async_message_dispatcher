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
let dispatcher = Dispatcher::new(|consumer| tokio::spawn(async move {
    let message = consumer.take().await.unwrap();
    message.respond("Please, enter your age.").await;
    let age = consumer.take().await.unwrap();
    age.respond("Now, enter your name.").await;
    let name = consumer.take().await.unwrap();
    name.respond("Now, enter your bio.").await;
    let bio = consumer.take().await.unwrap();
    self.add_user(age.text, name.text, bio.text);
    bio.respond("You're registered successfully!.").await;
}));

// Then, somewhere after that...

dispatcher.dispatch(key, message); // And the message will go to the handler!
```

Such approach gives code that is much more readable, especially when there are many states. Also, if you want, it's pretty easy to add message filtering:

```
async fn get_number<K: Key, M>(consumer: &mut Consumer<K, M>) -> i64 {
    loop {
        let message = consumer.take().await.unwrap();
        let number: Ok(i64) = message.text.parse() else {
            message.respond("Please, enter a number.").await;
            continue;
        };
    }
}
```

# Usage

Use a `Dispatcher` if you want to dispatch every message in the same way, or use a `Storage` if you want to manage `Consumer`s yourself.
