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

## 2026-05-27 — Phase 2, Session 5: axum (the framework)

- axum is sugar on top of tokio. Same concurrency, far less code per route.
- Handler = async fn whose return type implements IntoResponse. Strings, JSON, tuples of (StatusCode, body), Results — all map to HTTP responses.
- Extractor = parameter type that implements FromRequest. Path<T>, Json<T>, State<T> are extractors. axum runs them against the request to populate handler parameters; failure → automatic 4xx without the handler running.
- The declarative-binding pattern strikes again: clap for args, serde for JSON, axum for HTTP requests. Same shape — declare the type, library does the parsing and validation.
- Routes: Router::new().route("/path", get(handler)). Same path can chain methods: .route("/users", get(list).post(create)).
- axum 0.7.9 wants `{id}` for path params, not `:id` (that was the 0.6 syntax). Read the panic message — it told us exactly what to do.
- Shared mutable state across async handlers: Arc<Mutex<T>>. Arc lets many owners share; Mutex provides exclusive access; tokio::sync::Mutex (not std) is the async-aware version that yields the task instead of blocking the thread.
- type AppState = Arc<Mutex<HashMap<u32, User>>> — type aliases keep the long type readable everywhere.
- State extractor: handlers get the shared state via State<AppState>. State created in main, attached via .with_state(state) on the router, extracted in handlers.
- Mutex guards release on drop (RAII). Forgetting to unlock is impossible — the unlock happens automatically when the guard goes out of scope.
- Result<Json<T>, StatusCode> as a return type lets handlers cleanly express "200 with body OR 4xx status." `?` works inside handlers if the return type allows.
- HashMap doesn't preserve insertion order — iteration order is determined by hashing. BTreeMap sorts by key; Vec preserves insertion order; real APIs usually sort explicitly before returning.
- Gotcha: axum's panic at runtime if route syntax is wrong is a good design choice — better to fail loudly at startup than to silently 404 every request.

## Phase 2 — complete

- raw_server: HTTP/1.1 server with routing, JSON, shared state, concurrent connections.
- Built bottom-up: TCP listener → manual HTTP parsing → routing → async with tokio → axum.
- Every abstraction layer is now visible. axum doesn't feel magic because we wrote what it replaces.
- Architectural shape: pure logic / async glue / HTTP transport. Same as loggrep's shape but transport is HTTP not stdin.

## Next: Phase 3 — Persistence (Postgres via sqlx)

- Replace the HashMap with a real database.
- Connection pooling, schema, migrations, transactions.
- The Mutex disappears (the DB handles concurrency).
- Open question: when the server holds 100 concurrent requests all needing the DB, why isn't a single connection enough?

## 2026-05-31 — Phase 3, Session 1: Database concepts, Postgres setup

- A database is a separate server process that listens on a port (5432 for Postgres) and speaks its own wire protocol over TCP. Mechanically the same shape as your HTTP server.
- Connections are EXPENSIVE: TCP handshake + auth + per-connection memory on the server. ~50-200ms each. Postgres caps total connections (default 100).
- Connection pool: app maintains a small pool of long-lived connections, hands them out to handlers, takes them back. 10-20 connections can serve hundreds of concurrent requests because each query is microseconds.
- This is the answer to the open question — Mutex disappears because (1) Postgres handles concurrency via MVCC internally, (2) the pool itself is the concurrency primitive that handlers share via Arc.
- Pattern is universal: connection pools to DBs, HTTP client pools to upstreams, worker pools for jobs. Finite resource, transient borrowers, bounded concurrency.
- Postgres in Docker: clean isolation, version-pinned (postgres:16), recreatable. Real production runs either Docker or managed services (RDS/Supabase).
- Gotcha: port 5432 was occupied by another project's stale container. Moved our DB to port 5433 to avoid the fight. Non-default ports are also a small security-hygiene habit.
- Schema basics: CREATE TABLE with columns, types, NOT NULL, UNIQUE, PRIMARY KEY. SERIAL is shorthand for auto-incrementing integer backed by a sequence. TIMESTAMPTZ stores UTC and converts on output — always use it, never plain TIMESTAMP.
- PRIMARY KEY and UNIQUE create backing indexes automatically. Other indexes are explicit. btree is the default index type — O(log n) lookups, ranges, ordering.
- Sequences live outside transactions: a rolled-back insert still consumes its id. Postgres ids can have gaps. Don't rely on contiguity.

## Next: Phase 3, Session 2 — sqlx, first query from Rust

- Pull sqlx into Cargo.toml with the postgres + tokio features.
- Connect to the DB from Rust. PgPool as shared state instead of Arc<Mutex<HashMap>>.
- Compile-time-checked queries: sqlx::query! / query_as! macros validate SQL against the live DB at build time.
- Open question: how can a Rust macro check SQL against a real database during compilation? What's it actually doing?

## 2026-06-01 — Phase 3, Session 2: sqlx, real database-backed CRUD

- sqlx is the standard async DB library. Its signature feature: compile-time SQL checking (query!/query_as! macros connect to the live DB at build time, run PREPARE, and type-check the query). We used the runtime query_as (no !) this session; macros next.
- PgPool::connect(&url) opens the connection pool. PgPool is Clone + internally reference-counted — no Arc/Mutex wrapper needed. It IS the shared state now.
- DATABASE_URL env var (postgres://user:pass@host:port/db) tells sqlx where the DB is. Stored in .env, gitignored. Exported in shell for cargo build.
- #[derive(sqlx::FromRow)] maps a query result row into a struct by matching column names to field names.
- query*as::<*, User>("SQL").bind(x).bind(y).<fetch>(&pool).await is the core pattern.
- Three fetch shapes: fetch_all -> Vec<T>; fetch_optional -> Option<T> (maps cleanly to 404); fetch_one -> T (errors on 0 or >1 rows). execute -> rows-affected count for writes without RETURNING.
- $1, $2 placeholders + .bind() = parameterized queries = SQL-injection-safe. NEVER string-interpolate user input into SQL.
- RETURNING (Postgres) makes INSERT return the created row including auto-generated id and created_at — no separate SELECT needed.
- Error mapping: sqlx::Error is an enum. The Database(db_err) variant has helpers like is_unique_violation(). Map unique violations to 409 Conflict (client error), everything else to 500 (server error). Returning the right status code matters — clients build retry logic on it.
- Confirmed: failed insert (duplicate email) still consumed an id from the sequence — ids went 1,2,3,5 (4 was the rejected insert). Sequences don't recycle. Gaps are normal.
- TIMESTAMPTZ -> chrono::DateTime<Utc> -> ISO-8601 JSON string, all via derives. Five layers of conversion, zero hand-written code.

## Next: Phase 3, Session 3 — compile-time checked queries + migrations

- Switch from query_as (runtime) to query_as! (compile-time checked). Feel SQL errors become build errors.
- Install sqlx-cli. Set up migrations (versioned schema changes) instead of hand-running CREATE TABLE in psql.
- Open question: if sqlx checks queries against the live DB at compile time, how does CI build the project without a running database? (Answer involves `cargo sqlx prepare` and offline mode.)

## 2026-06-01 — Phase 3, Session 3: migrations + compile-time checked queries

### Migrations (sqlx-cli)

- Migrations = versioned SQL scripts committed to the repo that evolve the schema step by step. Each runs exactly once, in timestamp order, tracked in the \_sqlx_migrations table.
- Why: hand-typed CREATE TABLE in psql lives only on your laptop. Migrations let teammates, CI, and production all reach an identical schema from code.
- sqlx-cli: `cargo install sqlx-cli --no-default-features --features postgres`. Reads DATABASE_URL.
- `sqlx migrate add <name>` creates migrations/<timestamp>\_<name>.sql. Put schema SQL in it. `sqlx migrate run` applies all pending.
- Idempotent: re-running migrate run does nothing if all are applied (sees them in \_sqlx_migrations). Safe to run on every deploy.
- Debugged a failed migration: pre-existing hand-made `users` table blocked CREATE TABLE ("already exists"). Diagnosed via \dt and SELECT from \_sqlx_migrations (0 rows = nothing applied = clean failure, nothing to roll back). Fix: DROP the conflicting table, re-run.
- Hard-won: `sqlx`/`docker`/`cargo` are SHELL commands (prompt %). SQL and \dt are psql commands (prompt #) — run inside psql or via psql -c "...". Pasting a shell command into psql just hangs it waiting for a semicolon.

### Compile-time checked queries (query_as! macro)

- query_as!(StructType, "SQL", bind1, bind2) — struct is first macro arg, binds are TRAILING macro args (not .bind() chains). The `!` is the difference.
- At COMPILE time the macro connects to the live DB (via DATABASE_URL), runs PREPARE, gets the real column names + types from Postgres, and generates type-checked code. This is the answer to the old open question: the macro delegates to Postgres, the authority on what the SQL means.
- Proved it: typo'd `email`->`eail` in the Rust source → `cargo build` FAILED with Postgres's "column does not exist" error, attributed to the line in main.rs. No binary produced. Same bug that would've been a runtime 500 with the non-macro form, caught at build time instead.
- The macro generates row mapping itself — #[derive(sqlx::FromRow)] no longer needed (harmless to keep).
- TRADEOFF: build now depends on a reachable DB with current schema. Teammates/CI without a DB can't build. Fix is `cargo sqlx prepare` → caches query metadata into .sqlx/ (committed to repo) for offline builds. NOT YET DONE.
- Detail: a single multi-row INSERT evaluates NOW() once, so all rows share an identical created_at timestamp.

## Next: Phase 3, Session 4 — offline query cache + maybe UPDATE/DELETE

- `cargo sqlx prepare` for offline/CI builds (.sqlx cache). Understand why CI can't hit the dev DB.
- Add UPDATE (PUT/PATCH) and DELETE handlers to complete CRUD — .execute() and rows-affected.
- Then Session 5: indexes, EXPLAIN/query plans, the N+1 problem.

## 2026-06-01 — Phase 3, Session 4: offline query cache + full CRUD (UPDATE/DELETE)

### Offline query cache (cargo sqlx prepare)

- query_as!/query! need a live DB at compile time. That breaks teammates without a DB, CI containers, offline builds.
- `cargo sqlx prepare` runs all macros against the live DB once, caches the metadata (columns, types, param types, nullability) as JSON in .sqlx/. COMMIT .sqlx/ to the repo (it's not secret — .env is secret and stays gitignored; .sqlx travels with the code).
- Build logic: if DATABASE_URL is set → validate against live DB. If not → fall back to .sqlx/ cache. Either way still type-checked, just against a snapshot.
- Proved offline mode: `unset DATABASE_URL; touch src/main.rs; cargo build` → compiled fine using the cache.
- Tradeoff: change a query or schema → must re-run `cargo sqlx prepare` and commit the new cache. Teams enforce with `cargo sqlx prepare --check` in CI (fails if cache is stale).
- The cached JSON demystifies the "magic": it's literally Postgres's description of each query. nullability in the schema (NOT NULL) flows into whether the macro generates String vs Option<String>. Schema constraints become Rust types.

### Full CRUD (UPDATE + DELETE)

- query! (no struct arg) for statements that don't return rows. query_as! when you map rows to a struct.
- .execute(&pool) for non-returning statements → returns a result with .rows_affected() (u64).
- DELETE pattern: execute, then rows_affected() == 0 → 404 (nothing was there), else → 204 No Content (success, no body). Without the rows-affected check you'd falsely report success for deleting a nonexistent row.
- UPDATE pattern: UPDATE ... SET ... WHERE id = $n RETURNING ... + .fetch_optional. If WHERE matches no row, RETURNING returns nothing → None → 404. If it matches → updated row → 200. Option encodes existence again.
- UPDATE can also violate UNIQUE (changing a row's email to one another row owns) → same is_unique_violation() → 409. Hit this for real in testing.
- Confirmed an update keeps id and created_at constant, changes only name/email. An update is not a new row.
- Routes chain methods per path: get(get_user).put(update_user).delete(delete_user). PUT = full replace, PATCH = partial (we did PUT).
- Final CRUD status-code table: GET list 200 / GET one 200|404 / POST 201|409 / PUT 200|404|409 / DELETE 204|404, all with 500 as the genuine-server-error fallback.

## Next: Phase 3, Session 5 — indexes, query plans, N+1

- EXPLAIN / EXPLAIN ANALYZE: read a query plan, see seq scan vs index scan.
- Add an index, watch the plan change, measure the difference.
- The N+1 problem: why "list users then fetch each one's orders in a loop" collapses, and how a JOIN or batched query fixes it. Needs a second table (orders) with a foreign key — so this session also introduces relations.
