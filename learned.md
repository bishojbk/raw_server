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

## 2026-05-25 — Phase 2, Session 3: Routing

- HTTP frames the byte stream with \r\n line endings and \r\n\r\n to terminate headers. The first line is the request line: METHOD PATH HTTP/VERSION.
- Parsed the request line by-hand: take the first line, split on spaces, second piece is the path.
- Router = match on the path. Express's app.get, axum's Router, everyone's router does this same dispatch under different syntax.
- Built a build_response(status, body) helper so the format!() block isn't repeated three times. DRY pattern same as parse_level reuse last phase.
- HTTP status codes: 2xx success, 3xx redirect, 4xx client error, 5xx server error. Used 200 and 404 today.
- Server is now route-aware but still single-threaded and sequential. One slow handler blocks every other client. This is the bug async fixes.
- Blocking I/O: thread parks waiting for bytes. Non-blocking + async: thread does other work while waiting. tokio is the runtime that schedules tasks across threads when they would otherwise block.

## Next: Phase 2, Session 4 — async with tokio

- Pull in tokio, rewrite the server using async fn and .await.
- Spawn a task per connection so the server handles many at once on one or a few threads.
- Conceptually the hardest session of Phase 2. Fresh brain required.
- Open question: what's the runtime actually doing under the hood when you write `.await`? It's not a thread switch — what is it?

## 2026-05-26 — Phase 2, Session 4: Async, tokio, and what .await actually does

- An async fn does NOT run when called. Calling it builds and returns a Future (a state machine struct). Nothing executes until something polls it.
- A Future is a value. It sits in memory doing nothing on its own. .await polls it; tokio::spawn hands it to the runtime to poll concurrently with other tasks.
- Demonstrated: calling slow_task("A") three times without .await printed nothing because no future was polled. The compiler even warns "unused must_use Future" — building a future and dropping it is a real bug.
- .await is the yield point. The compiler converts the async fn into a state machine where each .await is a place the function can pause, save its state into the struct, and return Pending to the runtime. When the awaited future becomes ready, the runtime resumes the task.
- A task that's awaiting is FROZEN, not running. No background thread is churning through it. It's just memory until the runtime polls it again.
- tokio runs a small pool of worker threads (default: one per CPU core). Each thread polls tasks. Many tasks per thread; ~hundreds of bytes per task vs ~8KB per OS thread stack.
- tokio::spawn(future) hands a future to the runtime as an independent task. In the server: the accept loop spawns one task per connection so the loop can immediately go back to accepting.
- async move {} is an inline async block; move means it takes ownership of captured variables (necessary because spawned tasks outlive the loop iteration).
- Sync vs async I/O: tokio::time::sleep yields the task and frees the thread; std::thread::sleep blocks the thread and stalls every other task on it. The same applies to file reads, network reads, locks. NEVER call a thread-blocking function inside an async fn — it collapses concurrency silently. tokio::spawn_blocking exists for unavoidable blocking calls.
- Proved the model: /slow parked for 3s while /health returned in 9ms on the same server. Sync version would have made /health wait the full 3s.
- The async ecosystem is contagious: every dependency needs to be async-aware or isolated. This is the "async colors functions" criticism — async fns can only call other async fns easily. The win in concurrency is paid for in code structure.

## Next: Phase 2, Session 5 — axum

- Replace the hand-rolled HTTP parsing and routing with axum.
- Feel the ergonomic win: declarative routes, typed extractors, automatic JSON serialization.
- The capability is the same (we're still on tokio). What changes is the abstraction level.
- Open question: when we write `async fn handler(Json(body): Json<User>) -> ...` in axum, what is the framework doing for us that we'd otherwise hand-roll?
