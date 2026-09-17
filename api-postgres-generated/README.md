# api-postgres-generated

An example generated using the api template

REST API on axum: `/v1/items` CRUD, OpenAPI document with Swagger UI,
`X-Request-Id` on every request, per client rate limiting, RFC 9457
error bodies, `tracing` logs and Prometheus metrics.

Items are stored in PostgreSQL through sqlx; schema in `migrations/`.

## Running

```sh
cp .env.example .env
just up          # docker compose: database and prometheus
just migrate-run # needs sqlx-cli
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
  "type": "urn:api_postgres_generated:error:validation",
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

## Configuration

Read from the environment; `.env` is loaded first.

- `API_POSTGRES_GENERATED_ADDR`: listen address, default `127.0.0.1:8080`
- `API_POSTGRES_GENERATED_LOG`: `tracing` filter, default `info`
- `API_POSTGRES_GENERATED_LOG_FORMAT`: `pretty` (default) or `json`
- `API_POSTGRES_GENERATED_RATE_LIMIT_RPS`: sustained requests per second per
  client, default `10`; `0` disables rate limiting
- `API_POSTGRES_GENERATED_RATE_LIMIT_BURST`: requests allowed at once, default `50`
- `API_POSTGRES_GENERATED_TRUST_PROXY`: take the client address from
  `X-Forwarded-For`, `X-Real-Ip` or `Forwarded`, default `false`; only
  enable behind a proxy that overwrites those headers
- `DATABASE_URL`: required
- `API_POSTGRES_GENERATED_RUN_MIGRATIONS`: apply migrations at startup, default
  `true`

## Docker

```sh
just docker-build                        # linux/amd64, loaded locally
just docker-build PLATFORM=linux/arm64
just docker-buildx ghcr.io/ijanc/api-postgres-generated:latest  # multi-arch, pushed
just up                                  # docker compose with prometheus on :9090
```

## License

[ISC](LICENSE)
