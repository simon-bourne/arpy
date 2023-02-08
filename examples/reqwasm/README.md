# Reqwasm Example

## Running

Install [Trunk]:

```bash
cargo install trunk --locked
```

Then start a server. From the project root, run:

```bash
cd examples/axum
cargo run
```

Then run the client:

```bash
cd ../../examples/reqwasm
trunk serve --open
```

[Trunk]: https://trunkrs.dev/
