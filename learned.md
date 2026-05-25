## 2026-05-25 — Phase 2, Session 1+2: TCP, HTTP, raw server

- TCP is a reliable, ordered byte stream between two programs. No message boundaries — just bytes in order.
- HTTP is a text protocol layered on TCP. Request = method+path+version, headers, blank line, optional body. Response = status line, headers, blank line, body. Lines end with \r\n.
- DNS resolves a name to IP(s). One name can map to many IPs — round-robin load balancing at the protocol level.
- A server is a program that: (1) listens on a port, (2) accepts connections, (3) reads bytes, (4) computes a response, (5) writes bytes back, (6) closes or keeps the connection alive.
- Built a raw HTTP server in ~30 lines using std::net::TcpListener and std::io::{Read, Write}. No framework. Handles GET, returns "hello from the server".
- HTTP/0.9 was the first version (1991): just body, no headers, no status line. Modern clients refuse it. Curl's "Received HTTP/0.9 when not allowed" was the symptom of returning raw bytes without a status line.
- Content-Length tells the client how many body bytes to expect. Required for clients to know where the response ends on a keep-alive connection.
- 127.0.0.1 binds to localhost only; 0.0.0.0 binds to all interfaces. Difference is whether your server is reachable from the network.
- The current server is unconditional (same response for every request) and sequential (one connection at a time). Routing and concurrency come next.
- std::net is synchronous and blocking. tokio's equivalents are async, which is how real Rust servers handle thousands of concurrent connections. We'll get there.

## Next: Phase 2, Session 3 — routing

- Parse the request line to extract method and path.
- Dispatch to different handlers based on path: /health, /users/:id, /.
- Still synchronous, still raw — last session before tokio.
- Open question: where does the request line end? How does the server know where headers stop and the body starts?
