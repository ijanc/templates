# {{project-name}}

{{project-description}}

REST API on axum: `/v1/items` CRUD, OpenAPI document with Swagger UI,
`X-Request-Id` on every request, per client rate limiting, RFC 9457
error bodies, `tracing` logs and Prometheus metrics.

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
  "type": "urn:{{crate_name}}:error:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "request validation failed",
  "instance": "/v1/items",
  "code": "validation",
  "request_id": "6b2b...",
  "errors": [{ "field": "name", "code": "blank", "message": "must not be blank" }]
}
```

`code` is stable per error kind; `errors` is present on 422 only.

`/v1` is rate limited per client address: `X-RateLimit-Limit` and
`X-RateLimit-Remaining` on every response, `429` with `Retry-After` once
the burst is spent. Probes, metrics and docs are not limited.

Every item response carries an `ETag` and `Cache-Control: private,
no-cache`. `GET` with `If-None-Match` answers `304` when the tag still
matches; `PUT` and `DELETE` with `If-Match` answer `412` when it no
longer does, so concurrent edits cannot overwrite each other.
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
