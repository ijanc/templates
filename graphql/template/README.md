# {{project-name}}

{{project-description}}

GraphQL API on axum with async-graphql: item queries and mutations, GraphiQL,
the schema in SDL, depth and complexity limits, `X-Request-Id` on every request,
per client rate limiting, `tracing` logs and Prometheus metrics.

{% if store == "memory" -%}
Items live in memory and are lost on exit.
{%- elsif store == "sqlite" -%}
Items are stored in SQLite through sqlx; schema in `migrations/`.
{%- else -%}
Items are stored in PostgreSQL through sqlx; schema in `migrations/`.
{%- endif %}

## Running

```sh
cp .env.example .env
{%- if store == "postgres" %}
just up          # docker compose: database and prometheus
just migrate-run # needs sqlx-cli
{%- endif %}
just run
```

`just dev` serves under bacon and restarts on every change; systemfd holds
the socket open in between, so requests made during a rebuild wait instead of
being refused. Needs `cargo install bacon systemfd`.

| path              | description                             |
| ----------------- | --------------------------------------- |
| `/graphql`        | `POST` GraphQL requests, `GET` GraphiQL |
| `/schema.graphql` | the schema in SDL                       |
| `/healthz`        | liveness                                |
| `/readyz`         | readiness, checks the store             |
| `/metrics`        | Prometheus text format                  |

```graphql
mutation {
  createItem(input: { name: "widget" }) { id name createdAt }
}

query {
  items(limit: 20, offset: 0) { items { id name } total }
  item(id: "6b2b...") { name description updatedAt }
}
```

`updateItem(id, input)` replaces every field, `deleteItem(id)` returns the id.
`item` is `null` when there is no such id; updating or deleting one is a
`NOT_FOUND` error.

Errors inside a GraphQL request come back with status 200 in `errors`, each with
a stable `extensions.code` and the request id:

```json
{
  "data": null,
  "errors": [{
    "message": "input validation failed",
    "path": ["createItem"],
    "extensions": {
      "code": "BAD_USER_INPUT",
      "request_id": "6b2b...",
      "errors": [{ "field": "name", "code": "blank", "message": "must not be blank" }]
    }
  }]
}
```

| code                    | meaning                                          |
| ----------------------- | ------------------------------------------------ |
| `BAD_REQUEST`           | parse, schema validation, limits, argument types |
| `BAD_USER_INPUT`        | an input check failed; details in `errors`       |
| `NOT_FOUND`             | no item with that id                             |
| `INTERNAL_SERVER_ERROR` | logged, the cause is not sent                    |

Everything outside a GraphQL request (unknown route, a body that is not a
GraphQL request, rate limit, timeout) is an RFC 9457 Problem Details response,
`application/problem+json`, with `code`, `instance` and `request_id`.

Queries nested more than 10 levels deep or above a complexity of 200 are
rejected before they run.

`POST /graphql` is rate limited per client address: `X-RateLimit-Limit` and
`X-RateLimit-Remaining` on every response, `429` with `Retry-After` once the
burst is spent. Probes, metrics, GraphiQL and the SDL are not limited.
{%- if cache %}

Reads by id are served from an in-process cache in front of the
database; writes update or drop the entry, so a single instance never
serves stale items. Hits and misses are counted in
`{{crate_name}}_cache_hits_total` and `{{crate_name}}_cache_misses_total`.
{%- endif %}

## Configuration

Read from the environment; `.env` is loaded first.

- `{{env_prefix}}_ADDR`: listen address, default `127.0.0.1:8080`
- `{{env_prefix}}_LOG`: `tracing` filter, default `info`
- `{{env_prefix}}_LOG_FORMAT`: `pretty` (default) or `json`
- `{{env_prefix}}_RATE_LIMIT_RPS`: sustained requests per second per
  client, default `10`; `0` disables rate limiting
- `{{env_prefix}}_RATE_LIMIT_BURST`: requests allowed at once, default `50`
- `{{env_prefix}}_TRUST_PROXY`: take the client address from
  `X-Forwarded-For`, `X-Real-Ip` or `Forwarded`, default `false`; only
  enable behind a proxy that overwrites those headers
{%- if store == "sqlite" %}
- `DATABASE_URL`: default `sqlite://{{project-name}}.db?mode=rwc`
- `{{env_prefix}}_RUN_MIGRATIONS`: apply migrations at startup, default
  `true`
{%- endif %}
{%- if store == "postgres" %}
- `DATABASE_URL`: required
- `{{env_prefix}}_RUN_MIGRATIONS`: apply migrations at startup, default
  `true`
{%- endif %}
{%- if cache %}
- `{{env_prefix}}_CACHE_TTL`: seconds an item stays cached, default
  `60`; `0` disables the cache
- `{{env_prefix}}_CACHE_CAPACITY`: most items cached, default `10000`
{%- endif %}

## Docker

```sh
just docker-build                        # linux/amd64, loaded locally
just docker-build PLATFORM=linux/arm64
just docker-buildx ghcr.io/{{gh-username}}/{{project-name}}:latest  # multi-arch, pushed
just up                                  # docker compose with prometheus on :9090
```

## License

[ISC](LICENSE)
