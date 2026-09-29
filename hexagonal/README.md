# hexagonal

Hexagonal architecture as a Cargo workspace. A domain crate holds the
entities, their rules and the ports; an application crate holds the use
cases; adapters sit on either side: an axum REST API and a command line
client driving them, an in-memory store and a SQLite or PostgreSQL store
(sqlx, embedded migrations) driven by them; a server binary wires it all.
Every layer is a crate, so the dependency direction is enforced by
`Cargo.toml` and checked by a script. The HTTP adapter is the
[api](../api/) template's stack: `/v1/items` CRUD, OpenAPI with Swagger
UI, `X-Request-Id`, per client rate limiting, `ETag` conditional
requests, RFC 9457 Problem Details, `tracing` and Prometheus metrics.
Configuration comes from the environment and `.env`. Ships a multi-stage
Dockerfile on cargo-chef, a compose file with Prometheus, CI, pre-commit,
dprint, typos and cargo-deny setup.

## Usage

```sh
cargo generate ijanc/templates hexagonal --name bar
```

Produces workspace `bar` with binary `bar`, and `bar-cli` with `cli=true`.

## Layout

```
template/
  cargo-generate.toml        placeholders, hooks, ignore list
  hooks/pre.rhai             derives crate_name, env_prefix, sqlx, year;
                             drops adapters/store-sqlx (store=memory) and
                             apps/cli (cli=false)
  Cargo.toml                 workspace: members, package, dependencies
  crates/domain/             <name>-domain
    src/item.rs              Item, Name and its rules
    src/ports.rs             ItemRepository (driven port), RepositoryError
  crates/application/        <name>-application
    src/ports.rs             ItemUseCases (driving port), inputs, Page
    src/service.rs           ItemService over Arc<dyn ItemRepository>
    src/error.rs             Error: NotFound, Invalid(fields), Repository
    tests/items.rs           the use cases over store-memory
  crates/testkit/            <name>-testkit
    src/lib.rs               repository_contract_tests! macro
    src/contract.rs          the checks, run one at a time
  adapters/http/             <name>-http, the api template's axum stack
    src/lib.rs               AppState, app() with the middleware stack
    src/dto.rs               Item, CreateItem, UpdateItem, ListQuery, Page
    src/error.rs             ApiError, Problem, From<application::Error>
    src/extract.rs           Json/Path/Query/Header with ApiError rejections
    src/http_cache.rs        ETag and Cache-Control
    src/metrics.rs           Prometheus layer, GET /metrics
    src/openapi.rs           base OpenAPI document
    src/rate_limit.rs        RateLimit, governor layer, 429 as ApiError
    src/request_id.rs        X-Request-Id layers
    src/trace.rs             request span
    src/routes/              /healthz, /readyz, /v1/items
    tests/                   spawn() over store-memory; items.rs, infra.rs
  adapters/store-memory/     <name>-store-memory: RwLock<HashMap>
    tests/contract.rs        the contract
  adapters/store-sqlx/       <name>-store-sqlx (store=sqlite|postgres)
    migrations/              sqlx migrations
    tests/contract.rs        the contract on sqlite::memory: or DATABASE_URL
  apps/server/               <name>: Config, telemetry, wiring, serve
    build.rs                 <ENV_PREFIX>_GIT_HASH / _BUILD_DATE
  apps/cli/                  <name>-cli (cli=true): migrate, items CRUD
    tests/cli.rs             runs the binary against a temporary database
  contrib/arch.sh            dependency allowlist of domain and application
  Dockerfile                 cargo-chef planner and builder, debian runtime
  compose.yaml               app + prometheus (+ db for postgres)
  .env.example               every variable with its default
```

## Placeholders

Prompted:

| name                  | description                                       |
| --------------------- | ------------------------------------------------- |
| `project-name`        | workspace and binary name, kebab-case (built-in)  |
| `project-description` | `Cargo.toml` descriptions, README, OpenAPI        |
| `gh-username`         | repository URL, `FUNDING.yml`                     |
| `domain`              | `security@`, `conduct@` contact addresses         |
| `store`               | `memory`, `sqlite` (default) or `postgres`        |
| `cli`                 | `apps/cli`, default `false`; database stores only |

Derived in `hooks/pre.rhai`:

| name         | value                             |
| ------------ | --------------------------------- |
| `crate_name` | `project-name` in snake_case      |
| `env_prefix` | `crate_name` in SHOUTY_SNAKE_CASE |
| `sqlx`       | `store != "memory"`               |
| `year`       | current UTC year                  |

## Notes

- Dependencies point inwards, enforced by each crate's `Cargo.toml`:
  `domain` depends on no other member, `application` on `domain`, the
  stores on `domain`, `http` on `domain` and `application`, the apps on
  everything. `contrib/arch.sh` (`just arch`, the `Layers` CI job) fails
  when `domain` or `application` gain a dependency outside their
  allowlist.
- Members name their library target after their layer (`[lib] name`:
  `domain`, `application`, `http_adapter`, `store_memory`, `store_sqlx`,
  `testkit`) and `[workspace.dependencies]` aliases them the same way,
  so code, tests included, says `use domain::Item` and no source file
  carries the project name; `rustfmt` output does not depend on it
  either.
- Ports are `async-trait` traits used as `Arc<dyn ...>`: `async fn` in
  traits is not dyn compatible.
- Rules live in the domain: `Name::new` is the only way to get a name,
  so the HTTP body types check shape only and a broken rule is a 422
  built from the use case's `Error::Invalid`. Field codes (`blank`,
  `length`, `range`) are stable.
- The domain truncates timestamps to microseconds, the coarsest
  precision any store keeps, so an item reads back equal to the one
  written on every store and the `ETag` can be `updated_at` as is.
- `store-memory` always exists: it is the store for `store=memory` and
  the test double of `application` and `http` otherwise, so `cargo test`
  needs no database beyond the sqlx contract tests (`sqlite::memory:`, or
  `DATABASE_URL` for PostgreSQL, skipped when unset).
- `testkit::repository_contract_tests!(make)` stamps the same tests out
  for every store; `make` is an `async fn() -> Option<R>` and `None`
  skips. The tests hold a lock while they run, so a shared database is
  cleared per test.
- `apps/cli` is a second driving adapter over the same use cases:
  `<name>-cli migrate` applies the migrations, `<name>-cli items ...` is
  CRUD. Its tests run the binary against a temporary SQLite file, or
  `DATABASE_URL` for PostgreSQL.
- The Dockerfile uses cargo-chef: the dependency layer is keyed on the
  manifests, the dummy `main.rs` trick does not scale to a workspace.
- The generated project is a workspace of its own, so it is `exclude`d
  from this repository's workspace and `just check` runs it through
  `--manifest-path`; its `Cargo.lock` and `target` are ignored here.
- `Justfile` and `.github/workflows/ci.yml` use `{% raw %}` around `{{...}}`
  that must survive generation.
- `SECURITY.md` ships with a `<!-- pgp-key -->` marker; `just security-key` in
  the generated project creates a key and fills it in.
- The Dockerfile builds with `--locked`, so commit `Cargo.lock`.
