# graphql

GraphQL API on axum with async-graphql: item queries and mutations, GraphiQL on
`GET /graphql`, the schema in SDL on `/schema.graphql`, depth and complexity
limits, resolver errors with a stable `extensions.code`, `validator` derived
input checks, `X-Request-Id` set, propagated and copied into every GraphQL
error, per client rate limiting on GraphQL requests with `X-RateLimit-*`
headers, RFC 9457 Problem Details for errors outside GraphQL, `tracing` logs
(pretty or JSON), Prometheus metrics, a store that is in-memory, SQLite or
PostgreSQL (sqlx, embedded migrations) and an optional in-process read cache
(moka) for the database stores. Configuration comes from the environment and
`.env`. Ships a multi-stage Dockerfile, a compose file with Prometheus, CI,
pre-commit, dprint, typos and cargo-deny setup.

## Usage

```sh
cargo generate ijanc/templates graphql --name bar
```

Produces crate `bar` with binary `bar`.

## Layout

```
template/
  cargo-generate.toml     placeholders, hooks, ignore list
  hooks/pre.rhai          derives crate_name, ..., picks src/store.rs,
                          drops src/cache.rs (deleted on generate)
  Cargo.toml              lib + one bin
  build.rs                <ENV_PREFIX>_GIT_HASH / _BUILD_DATE
  src/lib.rs              AppState, app() router with the middleware stack
  src/main.rs             .env, Config, telemetry, store, serve, shutdown
  src/config.rs           Config::from_env()
  src/telemetry.rs        tracing subscriber, request span
  src/request_id.rs       X-Request-Id layers
  src/metrics.rs          Prometheus layer, GET /metrics
  src/rate_limit.rs       per client governor layer, 429 as ApiError
  src/cache.rs            moka read cache in front of the store (cache=true)
  src/error.rs            ApiError -> Problem (application/problem+json)
  src/model.rs            Item, CreateItem, UpdateItem, ListQuery, Page
  src/routes.rs           /graphql, /schema.graphql, /healthz, /readyz
  src/graphql/mod.rs      Schema, merged Query and Mutation, limits
  src/graphql/items.rs    item, items, createItem, updateItem, deleteItem
  src/graphql/error.rs    resolver Error -> extensions.code
  src/store_memory.rs     RwLock<HashMap>  -> src/store.rs (store=memory)
  src/store_sqlx.rs       sqlx pool        -> src/store.rs (store=sqlite|postgres)
  migrations/             sqlx migrations (store=sqlite|postgres)
  tests/common/mod.rs     spawn() the app on an ephemeral port, gql()
  tests/items.rs          queries and mutations over HTTP with reqwest
  tests/infra.rs          probes, request id, metrics, GraphiQL, SDL
  Dockerfile              rust:1-bookworm builder, debian:bookworm-slim runtime
  compose.yaml            app + prometheus (+ db for postgres)
  contrib/prometheus.yml  scrape config for compose
  .env.example            every variable with its default
```

Both `src/store_*.rs` expose the same `Store` methods (`ping`, `create`, `list`,
`get`, `update`, `delete`), so the resolvers are identical for every backend.
Queries are plain strings with bind parameters, no `DATABASE_URL` is needed at
build time.

## Placeholders

Prompted:

| name                  | description                                  |
| --------------------- | -------------------------------------------- |
| `project-name`        | crate and binary name, kebab-case (built-in) |
| `project-description` | `Cargo.toml` description, README             |
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

- Configuration is environment only: `<ENV_PREFIX>_ADDR`, `_LOG`, `_LOG_FORMAT`,
  `_RATE_LIMIT_RPS`, `_RATE_LIMIT_BURST`, `_TRUST_PROXY`, with sqlx
  `DATABASE_URL` and `_RUN_MIGRATIONS`, and with the cache `_CACHE_TTL` and
  `_CACHE_CAPACITY`.
- Two error paths. Inside a GraphQL request, errors are in the response body
  with status 200; resolvers return `graphql::error::Error`, which sets
  `extensions.code`, and `graphql::error::finish` gives every other error
  (parse, validation, limits, argument coercion) `BAD_REQUEST` and adds
  `request_id`. Outside it (unknown route, 405, a body that is not a GraphQL
  request, 408, 429), `ApiError` and `error::problem_middleware` build Problem
  Details as usual.
- `async-graphql` is built with `custom-error-conversion`, which removes its
  blanket `From<T: Display>` for `async_graphql::Error`. A `?` on a store error
  in a resolver goes through `Error::Internal`, which logs the cause and sends
  `internal server error`, instead of leaking the message.
- `GraphQLRequest<ApiError>` is the extractor, so a body that is not a GraphQL
  request is a 400 Problem, and one over the size limit a 413.
- A new resource adds its own `Query` and `Mutation` structs in `src/graphql/`
  and one field to the `MergedObject`s in `src/graphql/mod.rs`.
- Input objects derive `validator::Validate` next to `InputObject`; the
  resolvers call `validate()` first. Failures are `BAD_USER_INPUT` with an
  `extensions.errors` list of `{field, code, message}`; `field` is the Rust
  field name.
- `Item` and `Page` serialize in camelCase so the tests deserialize GraphQL
  responses straight into them.
- The sqlx store truncates timestamps to microseconds before writing, the
  precision both databases keep, so an item returned by a mutation equals the
  same item read back.
- The cache is an `Option<Cache>` inside the sqlx `Store`: `get` fills it,
  `create`/`update` write through, `delete` drops the entry. `list` bypasses it.
  Another replica would serve stale items until the TTL passes; that is the
  documented trade-off of `cache=true`.
- The rate limiter is a layer on the `POST` method of `/graphql` only and keys
  on the TCP peer address, so the app is served with
  `into_make_service_with_connect_info`. Proxy headers are ignored unless
  `_TRUST_PROXY=true`; per address state lives in memory and is swept every
  minute.
- GraphiQL loads its scripts from a CDN.
- The Prometheus recorder is process global; `metrics::pair()` creates it once
  so `app()` can be called repeatedly by tests. Every GraphQL request is
  recorded under `endpoint="/graphql"`.
- SQLite tests use `sqlite::memory:`; `Store::connect` keeps a single connection
  for in-memory URLs. PostgreSQL tests skip when `DATABASE_URL` is unset; CI
  provides a service container.
- Test queries are string constants, not `format!` strings: liquid would read
  their `{{`/`}}` escapes as placeholders.
- `Justfile` and `.github/workflows/ci.yml` use `{% raw %}` around `{{...}}`
  that must survive generation.
- `SECURITY.md` ships with a `<!-- pgp-key -->` marker; `just security-key` in
  the generated project creates a key and fills it in.
- Name-bearing Rust lines are kept short or unbreakable, and the tests import
  crate items through `tests/common`, so `rustfmt` output does not depend on the
  project name.
- The Dockerfile builds with `--locked`, so commit `Cargo.lock`.
