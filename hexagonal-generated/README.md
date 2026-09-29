# hexagonal-generated

An example generated using the hexagonal template

A hexagonal workspace: the domain and the use cases are crates that know nothing
about HTTP or databases, adapters on either side of them do the talking. The
REST API is `/v1/items` CRUD on axum with OpenAPI, request ids, rate limiting,
RFC 9457 errors, `tracing` logs and Prometheus metrics.

Items are stored in SQLite through sqlx; schema in
`adapters/store-sqlx/migrations/`.

## Layout

```
crates/domain          Item, Name and its rules; ItemRepository, the driven port
crates/application     ItemUseCases, the driving port, and ItemService behind it
crates/testkit         the repository contract every store has to pass
adapters/http          axum: routes, wire types, errors, OpenAPI, middleware
adapters/store-memory  ItemRepository in a map, also the test double
adapters/store-sqlx    ItemRepository on SQLite, migrations
apps/server            hexagonal-generated: configuration and wiring
apps/cli               hexagonal-generated-cli: the same use cases from a terminal
```

Dependencies point inwards only: `apps` see everything, `adapters` see `domain`
and `application`, `application` sees `domain`, `domain` sees no other crate.
`just arch` checks the two inner crates against an allowlist; the `Layers` CI
job runs the same script.

A new use case is a method on `ItemUseCases` and `ItemService` in
`crates/application`, a route and its wire types in `adapters/http` and a
subcommand in `apps/cli`. A new port is a trait in `crates/domain/src/ports.rs`,
one implementation per store and a line in `apps/server/src/main.rs`.

## Running

```sh
cp .env.example .env
just run
```

`just dev` serves under bacon and restarts on every change; systemfd holds
the socket open in between, so requests made during a rebuild wait instead of
being refused. Needs `cargo install bacon systemfd`.

| path                     | description                   |
| ------------------------ | ----------------------------- |
| `/healthz`               | liveness                      |
| `/readyz`                | readiness, checks the store   |
| `/metrics`               | Prometheus text format        |
| `/swagger-ui`            | interactive API docs          |
| `/api-docs/openapi.json` | OpenAPI document              |
| `/v1/items`              | `POST`, `GET ?limit=&offset=` |
| `/v1/items/{id}`         | `GET`, `PUT`, `DELETE`        |

Errors are RFC 9457 Problem Details, `application/problem+json`:

```json
{
  "type": "urn:hexagonal_generated:error:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "request validation failed",
  "instance": "/v1/items",
  "code": "validation",
  "request_id": "6b2b...",
  "errors": [{ "field": "name", "code": "blank", "message": "must not be blank" }]
}
```

`code` is stable per error kind; `errors` is present on 422 only and comes from
the rules in `crates/domain` and `crates/application`.

`/v1` is rate limited per client address: `X-RateLimit-Limit` and
`X-RateLimit-Remaining` on every response, `429` with `Retry-After` once the
burst is spent. Probes, metrics and docs are not limited.

Every item response carries an `ETag` and `Cache-Control: private, no-cache`.
`GET` with `If-None-Match` answers `304` when the tag still matches; `PUT` and
`DELETE` with `If-Match` answer `412` when it no longer does, so concurrent
edits cannot overwrite each other.

## Command line

```sh
just cli migrate
just cli items create widget -d "the first one"
just cli items list
just cli items get <id>
just cli items update <id> gadget
just cli items delete <id>
```

`DATABASE_URL` comes from the environment or `.env`; `--database-url` overrides
it. Failures print one line on stderr and exit with status 1.

## Tests

```sh
just test
```

`crates/application` and `adapters/http` run over the in-memory store, so no
database is needed. `adapters/store-memory` and `adapters/store-sqlx` run the
same contract from `crates/testkit`, the latter on `sqlite::memory:`.

## Configuration

Read from the environment; `.env` is loaded first.

- `HEXAGONAL_GENERATED_ADDR`: listen address, default `127.0.0.1:8080`
- `HEXAGONAL_GENERATED_LOG`: `tracing` filter, default `info`
- `HEXAGONAL_GENERATED_LOG_FORMAT`: `pretty` (default) or `json`
- `HEXAGONAL_GENERATED_RATE_LIMIT_RPS`: sustained requests per second per
  client, default `10`; `0` disables rate limiting
- `HEXAGONAL_GENERATED_RATE_LIMIT_BURST`: requests allowed at once, default `50`
- `HEXAGONAL_GENERATED_TRUST_PROXY`: take the client address from
  `X-Forwarded-For`, `X-Real-Ip` or `Forwarded`, default `false`; only enable
  behind a proxy that overwrites those headers
- `DATABASE_URL`: default `sqlite://hexagonal-generated.db?mode=rwc`
- `HEXAGONAL_GENERATED_RUN_MIGRATIONS`: apply migrations at startup, default
  `true`

## Docker

```sh
just docker-build                        # linux/amd64, loaded locally
just docker-build PLATFORM=linux/arm64
just docker-buildx ghcr.io/ijanc/hexagonal-generated:latest  # multi-arch, pushed
just up                                  # docker compose with prometheus on :9090
```

## License

[ISC](LICENSE)
