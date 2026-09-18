# web-generated

An example generated using the web template

Server-rendered web application on axum and MiniJinja: `/items` CRUD with
forms, flash messages, session cookies, CSRF protected posts,
`X-Request-Id` on every request, HTML error pages, `tracing` logs
(pretty or JSON) and Bootstrap styling built with `sass`.

Items are stored in SQLite through sqlx; schema in `migrations/`.

Accounts are invite only, with passwords hashed using Argon2.

## Running

```sh
cp .env.example .env
npm install
npm run build   # static/css/app.css and static/js
just run
```

Templates are watched while `WEB_GENERATED_TEMPLATE_RELOAD` is on, so
editing `templates/` shows up on the next request without a restart.
Pages render without the stylesheet, so `cargo test` needs no npm.

| path                 | description                          |
| -------------------- | ------------------------------------ |
| `/`                  | redirects to `/items`                |
| `/items`             | list, `?limit=&offset=`              |
| `/items/new`         | create form                          |
| `/items/{id}`        | one item; `POST` replaces it         |
| `/items/{id}/edit`   | edit form                            |
| `/items/{id}/delete` | `POST` deletes                       |
| `/login`             | sign in                              |
| `/logout`            | `POST` signs out                     |
| `/register`          | create an account, `?token=`         |
| `/invites`           | list and create invitations          |
| `/healthz`           | liveness, JSON                       |
| `/readyz`            | readiness, checks the store          |
| `/static`            | stylesheet and script                |

Every form carries a `csrf_token` matching the one in the session;
`POST`, `PUT`, `PATCH` and `DELETE` without it answer `403`. Writes
redirect afterwards and leave a flash message, which the next page shows
once.

## Accounts

Registration needs an invitation. To create the first account, set
`WEB_GENERATED_BOOTSTRAP_INVITE` to a secret of your choosing
(`just bootstrap-invite` prints one), start the app and open
`/register?token=<that secret>`. It is refused as soon as an account
exists, so unset it afterwards.

Any signed in user can hand out further invitations at `/invites` and
share the `/register?token=...` link. An invitation works once and
expires after `WEB_GENERATED_INVITE_TTL`.

Reading items is public; creating, editing and deleting them needs an
account.

## Configuration

Read from the environment; `.env` is loaded first.

- `WEB_GENERATED_ADDR`: listen address, default `127.0.0.1:8080`
- `WEB_GENERATED_LOG`: `tracing` filter, default `info`
- `WEB_GENERATED_LOG_FORMAT`: `pretty` (default) or `json`
- `WEB_GENERATED_TEMPLATE_DIR`: default `templates`
- `WEB_GENERATED_TEMPLATE_RELOAD`: watch the template directory,
  default on in debug builds
- `WEB_GENERATED_STATIC_DIR`: served under `/static`, default `static`
- `WEB_GENERATED_SESSION_TTL`: seconds of inactivity before a session
  is dropped, default `86400`
- `WEB_GENERATED_COOKIE_SECURE`: send the session cookie over HTTPS
  only, default `false`; turn it on in production
- `DATABASE_URL`: default `sqlite://web-generated.db?mode=rwc`
- `WEB_GENERATED_RUN_MIGRATIONS`: apply migrations at startup, default
  `true`
- `WEB_GENERATED_INVITE_TTL`: seconds an invitation stays usable,
  default `604800`
- `WEB_GENERATED_BOOTSTRAP_INVITE`: token accepted while no account
  exists, unset by default

## Assets

`assets/scss/app.scss` imports Bootstrap; override its variables in the
`with (...)` block. `npm run build` compiles it to `static/css/app.css`
and copies the Bootstrap bundle to `static/js/`. Both directories are
build output and are not committed.

## Docker

```sh
just docker-build                        # linux/amd64, loaded locally
just docker-build PLATFORM=linux/arm64
just docker-buildx ghcr.io/ijanc/web-generated:latest  # multi-arch, pushed
just up                                  # docker compose
```

The image builds the stylesheet in a node stage and ships `templates/`
and `static/` next to the binary.

## License

[ISC](LICENSE)
