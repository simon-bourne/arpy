# Arpy: RPC for Rust

[![tests](https://github.com/simon-bourne/arpy/actions/workflows/tests.yml/badge.svg)](https://github.com/simon-bourne/arpy/actions/workflows/tests.yml)
[![crates.io](https://img.shields.io/crates/v/arpy.svg)](https://crates.io/crates/arpy)
[![Documentation](https://docs.rs/arpy/badge.svg)](https://docs.rs/arpy)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/arpy)](./LICENSE-APACHE)

Define your RPC signatures, and use them with various client/server implementations.

## Project Status

Arpy is in it's infancy, and not well tested yet.

## Transport Implementations

[Reqwest] and [Reqwasm] clients are available, along with [Axum] and [Actix] servers.

## Usage

Define your RPC signatures, implement them on the server, and call them on the client. These can be in separate crates, or all in one depending on your workflow.

### Defining RPC Signatures

```rust
#[derive(MsgId, Serialize, Deserialize, Debug)]
pub struct Add(pub i32, pub i32);

impl FnRemote for Add {
    type Output = i32;
}

#[derive(MsgId, Serialize, Deserialize, Debug)]
pub struct TryMultiply(pub i32, pub i32);

impl FnRemote for TryMultiply {
    type Output = Result<i32, ()>;
}
```

### Implementing a Server

Example using [Axum]:

```rust
async fn add(args: &Add) -> i32 {
    args.0 + args.1
}

async fn try_multiply(args: &TryMultiply) -> Result<i32, ()> {
    Ok(args.0 * args.1)
}

let app = Router::new()
    .http_rpc_route("/http", add)
    .http_rpc_route("/http", try_multiply);

Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9090))
    .serve(app.into_make_service())
    .await
    .unwrap();
```

### Calling Remote Procedures

Example using [Reqwasm]:

```rust
let connection = http::Connection::new(&format!("http://127.0.0.1:9090/api"));
let result = Add(1, 2).call(&connection).await?;

assert_eq!(3, result);
```

## Other Features

- Websockets support, including:
  - multiple in flight RPC calls
  - parameterized subscriptions
- Server sent events

[Reqwest]: https://github.com/seanmonstar/reqwest
[Reqwasm]: https://github.com/hamza1311/reqwasm
[Axum]: https://github.com/tokio-rs/axum
[Actix]: https://github.com/actix/actix-web
