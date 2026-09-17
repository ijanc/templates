# api

Versioned REST API on axum: `/v1/items` CRUD, OpenAPI document with Swagger
UI, `X-Request-Id` set and propagated on every request, per client rate
limiting on `/v1` with `X-RateLimit-*` headers, `ETag` with
`If-None-Match`/`If-Match` conditional requests, RFC 9457 Problem Details
error bodies, `validator` derived input checks, `tracing` logs (pretty or
JSON), Prometheus metrics, a store that is in-memory, SQLite or
PostgreSQL (sqlx, embedded migrations) and an optional in-process read
cache (moka) for the database stores. Configuration comes from the
environment and `.env`. Ships a multi-stage Dockerfile, a compose file with
Prometheus, CI, pre-commit, dprint, typos and cargo-deny setup.

## Usage

```sh
cargo generate ijanc/templates api --name bar
```

Produces crate `bar` with binary `bar`.

## Layout

```
template/
  cargo-generate.toml    placeholders, hooks, ignore list
  hooks/pre.rhai         derives crate_name, ..., picks src/store.rs,
                         drops src/cache.rs (deleted on generate)
  Cargo.toml             lib + one bin
  build.rs               <ENV_PREFIX>_GIT_HASH / _BUILD_DATE
  src/lib.rs             AppState, app() router with the middleware stack
  src/main.rs            .env, Config, telemetry, store, serve, shutdown
  src/config.rs          Config::from_env()
  src/telemetry.rs       tracing subscriber, request span
  src/request_id.rs      X-Request-Id layers
  src/metrics.rs         Prometheus layer, GET /metrics
  src/rate_limit.rs      per client governor layer, 429 as ApiError
  src/http_cache.rs      ETag and Cache-Control for items
  src/cache.rs           moka read cache in front of the store (cache=true)
  src/error.rs           ApiError -> Problem (application/problem+json)
  src/extract.rs         Json/Path/Query/Header with ApiError rejections
  src/model.rs           Item, CreateItem, UpdateItem, ListQuery, Page
  src/openapi.rs         base OpenAPI document
  src/routes/mod.rs      /healthz, /readyz, nest /v1
  src/routes/v1/items.rs POST, GET, PUT, DELETE /v1/items
  src/store_memory.rs    RwLock<HashMap>  -> src/store.rs (store=memory)
  src/store_sqlx.rs      sqlx pool        -> src/store.rs (store=sqlite|postgres)
  migrations/            sqlx migrations (store=sqlite|postgres)
  tests/common/mod.rs    spawn() the app on an ephemeral port
  tests/items.rs         CRUD over HTTP with reqwest
  tests/infra.rs         probes, request id, metrics, OpenAPI, Swagger UI
  Dockerfile             rust:1-bookworm builder, debian:bookworm-slim runtime
  compose.yaml           app + prometheus (+ db for postgres)
  contrib/prometheus.yml scrape config for compose
  .env.example           every variable with its default
```

Both `src/store_*.rs` expose the same `Store` methods (`ping`, `create`,
`list`, `get`, `update`, `delete`), so the routes are identical for every
backend. Queries are plain strings with bind parameters, no `DATABASE_URL`
is needed at build time.

## Placeholders

Prompted:

| name                  | description                                  |
| --------------------- | -------------------------------------------- |
| `project-name`        | crate and binary name, kebab-case (built-in) |
| `project-description` | `Cargo.toml` description, README, OpenAPI    |
| `gh-username`         | repository URL, `FUNDING.yml`                |
| `domain`              | `security@`, `conduct@` contact addresses    |
| `store`               | `memory` (default), `sqlite` or `postgres`   |
| `cache`               | moka read cache, default `false`; sqlx only  |

Derived in `hooks/pre.rhai`:

| name           | value                             |
| -------------- | --------------------------------- |
| `crate_name`   | `project-name` in snake_case      |
| `env_prefix`   | `crate_name` in SHOUTY_SNAKE_CASE |
| `sqlx`         | `store != "memory"`               |
| `author_name`  | name part of `authors`            |
| `author_email` | email part of `authors`           |
| `year`         | current UTC year                  |

## Notes

- Configuration is environment only: `<ENV_PREFIX>_ADDR`, `_LOG`,
  `_LOG_FORMAT`, `_RATE_LIMIT_RPS`, `_RATE_LIMIT_BURST`, `_TRUST_PROXY`,
  with sqlx `DATABASE_URL` and `_RUN_MIGRATIONS`, and with the cache
  `_CACHE_TTL` and `_CACHE_CAPACITY`.
- The `ETag` is `updated_at` in microseconds: `Utc::now()` has
  nanoseconds but the databases keep less, so a finer tag would differ
  between the `POST` response and the next `GET`. `If-Match` is checked
  with a read before the write; the window between them is accepted.
- The cache is an `Option<Cache>` inside the sqlx `Store`: `get` fills
  it, `create`/`update` write through, `delete` drops the entry. `list`
  bypasses it. Another replica would serve stale items until the TTL
  passes; that is the documented trade-off of `cache=true`.
- Error responses are built without the request at hand, so
  `error::problem_middleware` runs as middleware and fills `request_id`
  and `instance` in. It also gives bodiless 4xx/5xx responses (405 from
  the router, 408 from the timeout) a Problem body.
- The rate limiter keys on the TCP peer address, so the app is served with
  `into_make_service_with_connect_info`. Proxy headers are ignored unless
  `_TRUST_PROXY=true`; per address state lives in memory and is swept
  every minute.
- Input DTOs derive `validator::Validate`; the `Json` and `Query`
  extractors call it, so handlers never see invalid input. Failures are a
  422 with an `errors[]` list of `{field, code, message}`.
- The Prometheus recorder is process global; `metrics::pair()` creates it
  once so `app()` can be called repeatedly by tests.
- SQLite tests use `sqlite::memory:`; `Store::connect` keeps a single
  connection for in-memory URLs. PostgreSQL tests skip when `DATABASE_URL`
  is unset; CI provides a service container.
- `Justfile` and `.github/workflows/ci.yml` use `{% raw %}` around `{{...}}`
  that must survive generation.
- `SECURITY.md` ships with a `<!-- pgp-key -->` marker; `just security-key` in
  the generated project creates a key and fills it in.
- Name-bearing Rust lines are kept short or unbreakable, and the tests import
  crate items through `tests/common`, so `rustfmt` output does not depend on
  the project name.
- The Dockerfile builds with `--locked`, so commit `Cargo.lock`.
