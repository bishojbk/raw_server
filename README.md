# raw_server

A from-scratch HTTP server in Rust, built using only the standard library
(`std::net`). No `hyper`, no `axum`, no `tokio` — the goal is to learn the
HTTP wire format by reading and writing raw bytes over TCP.

## What it does today

`src/main.rs` opens a `TcpListener` on `127.0.0.1:8080`, accepts connections
one at a time, reads up to 1024 bytes of the incoming request, prints those
bytes to stdout, and writes back a hardcoded `HTTP/1.1 200 OK` response with
a small text body.

There is no request parsing, no routing, and no concurrency yet — each of
those will be added as the project grows.

## Run it

```sh
cargo run
```

The server logs `listening on http://127.0.0.1:8080` and then blocks waiting
for connections.

## Try it

From another terminal:

```sh
curl -v http://127.0.0.1:8080/
```

The server prints the raw request it received between `---` markers, which
is the point of the exercise — seeing what an HTTP request actually looks
like on the wire.

## Layout

- `src/main.rs` — the server loop
- `Cargo.toml` — package manifest (edition 2024, no dependencies)
- `learned.md` — running notes on what's been learned along the way
