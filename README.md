# RPC for Rust

This is very much a work in progress, but the general idea is that you could define some RPC functions like this:

```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct Add(pub i32, pub i32);

#[async_trait]
impl FnRemote for Add {
    type Output = i32;

    async fn run(&self) -> Self::Output {
        self.0 + self.1
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TryMultiply(pub i32, pub i32);

#[async_trait]
impl FnRemote for TryMultiply {
    type Output = Result<i32, ()>;

    async fn run(&self) -> Self::Output {
        Ok(self.0 * self.1)
    }
}
```

Then set up a server, using one of various implementations. For example, using the `axum` server:

```rust
let app = Router::new()
    .http_rpc_route::<Add>("/http")
    .http_rpc_route::<TryMultiply>("/http");

Server::bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
    .serve(app.into_make_service())
    .await
    .unwrap();
```

And then call them with various client implementations, for example, using the `reqwasm` client:

```rust
let mut connection = http::Connection::new(&format!("http://127.0.0.1:9090/api/add"));
let result = connection.call(&Add(1, 2)).await?;

assert_eq!(3, result);
```
