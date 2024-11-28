# scuffle-context

> [!WARNING]  
> This crate is under active development and may not be stable.

 [![crates.io](https://img.shields.io/crates/v/scuffle-context.svg)](https://crates.io/crates/scuffle-context) [![docs.rs](https://img.shields.io/docsrs/scuffle-context)](https://docs.rs/scuffle-context)

---

A crate designed to provide the ability to cancel futures using a context go-like approach, allowing for graceful shutdowns and cancellations.

## Why do we need this?

Its often useful to wait for all the futures to shutdown or to cancel them when we no longer care about the results. This crate provides an interface to cancel all futures associated with a context or wait for them to finish before shutting down. Allowing for graceful shutdowns and cancellations.

## Usage

Here is an example of how to use the `DataLoader` interface to batch multiple reads from a database.

```rust
let (ctx, handler) = Context::new();

tokio::spawn(async move {
    // Do some work
}.with_context(ctx));

// Will stop the spawned task and cancel all associated futures.
handler.cancel();
```

Another use case might be to batch multiple writes to a database.

```rust
struct MyUserUpdater(SomeDatabase);

impl BatchExecutor for MyUserUpdater {
    type Request = User;
    type Response = bool;

    async fn execute(&self, requests: Vec<(Self::Request, BatchResponse<Self::Response>)>) {
        let (users, responses) = requests.into_iter().unzip();

        // You would need to build the query somehow, this is just an example
        if let Err(e) = self.0.update("INSERT INTO users (id, name) VALUES ($1, $2), ($3, $4)").bind(users).await {
            error!("Failed to insert users: {}", e);

            for response in responses {
                // Reply back saying we failed
                response.send(false);
            }

            return;
        }

        // Reply back to the client that we successfully inserted the users
        for response in responses {
            response.send(true);
        }
    }
}

let batcher = Batcher::new(MyUserUpdater(database));

// Will only make a single request to the database and insert both users
// You can also use `batcher.execute_many` if you have more then one item to insert.
let (success1, success2) = join!(batcher.execute(user1), batcher.execute(user2));
if !success1 {
    error!("Failed to insert user 1");
}

if !success2 {
    error!("Failed to insert user 2");
}
```

## License

This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
You can choose between one of them if you use this work.

`SPDX-License-Identifier: MIT OR Apache-2.0`
