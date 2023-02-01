# Arpy: RPC for Rust

Define your RPC function signatures and use them with various client/server implementations.

## Current Transport Implementations

### Client

- [Reqwest]
- [Reqwasm]

### Server

- [Axum]
- [Actix]

## Usage

Define your RPC signatures, implement them on the server and call them on the client. These can be in separate crates, or all lumped into one depending on your workflow.

### Defining RPC Signatures

```rust
#[derive(RpcId, Serialize, Deserialize, Debug)]
pub struct Add(pub i32, pub i32);

impl FnRemote for Add {
    type Output = i32;
}

#[derive(RpcId, Serialize, Deserialize, Debug)]
pub struct TryMultiply(pub i32, pub i32);

impl FnRemote for TryMultiply {
    type Output = Result<i32, ()>;
}
```

### Implementing a Server

We use [Axum] for this example.

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

We use [Reqwasm] for this example:

```rust
let mut connection = http::Connection::new(&format!("http://127.0.0.1:9090/api"));
let result = Add(1, 2).call(&connection).await?;

assert_eq!(3, result);
```

[Reqwest]: https://github.com/seanmonstar/reqwest
[Reqwasm]: https://github.com/hamza1311/reqwasm
[Axum]: https://github.com/tokio-rs/axum
[Actix]: https://github.com/actix/actix-web
