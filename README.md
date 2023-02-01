# RPC for Rust

This is very much a work in progress, but the general idea is that you could define some RPC functions like this:

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

Then set up a server, using one of various implementations. For example, using the `axum` server:

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

And then call them with various client implementations, for example, using the `reqwasm` client:

```rust
let mut connection = http::Connection::new(&format!("http://127.0.0.1:9090/api"));
let result = Add(1, 2).call(&connection).await?;

assert_eq!(3, result);
```
