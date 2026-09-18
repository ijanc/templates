# {{project-name}}

{{project-description}}

Server-rendered web application on axum and MiniJinja: `/items` CRUD with
forms, flash messages, session cookies, CSRF protected posts,
`X-Request-Id` on every request, HTML error pages, `tracing` logs
(pretty or JSON) and Bootstrap styling built with `sass`.

{% if store == "memory" -%}
Items live in memory and are lost on exit.
{%- elsif store == "sqlite" -%}
Items are stored in SQLite through sqlx; schema in `migrations/`.
{%- else -%}
Items are stored in PostgreSQL through sqlx; schema in `migrations/`.
{%- endif %}
{%- if auth == "local" %}

Accounts are invite only, with passwords hashed using Argon2.
{%- endif %}
{%- if auth == "google" %}

Accounts are invite only. They sign in with a password (Argon2) or with
Google.
{%- endif %}

## Running

```sh
cp .env.example .env
npm install
npm run build   # static/css/app.css and static/js
{%- if store == "postgres" %}
just up          # docker compose: database
just migrate-run # needs sqlx-cli
{%- endif %}
just run
```

Templates are watched while `{{env_prefix}}_TEMPLATE_RELOAD` is on, so
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
{%- if auth != "none" %}
| `/login`             | sign in                              |
| `/logout`            | `POST` signs out                     |
| `/register`          | create an account, `?token=`         |
| `/invites`           | list and create invitations          |
{%- endif %}
{%- if auth == "google" %}
| `/auth/google`       | start the Google sign in             |
{%- endif %}
| `/healthz`           | liveness, JSON                       |
| `/readyz`            | readiness, checks the store          |
| `/static`            | stylesheet and script                |

Every form carries a `csrf_token` matching the one in the session;
`POST`, `PUT`, `PATCH` and `DELETE` without it answer `403`. Writes
redirect afterwards and leave a flash message, which the next page shows
once.
{%- if auth != "none" %}

## Accounts

Registration needs an invitation. To create the first account, set
`{{env_prefix}}_BOOTSTRAP_INVITE` to a secret of your choosing
(`just bootstrap-invite` prints one), start the app and open
`/register?token=<that secret>`. It is refused as soon as an account
exists, so unset it afterwards.

Any signed in user can hand out further invitations at `/invites` and
share the `/register?token=...` link. An invitation works once and
expires after `{{env_prefix}}_INVITE_TTL`.

Reading items is public; creating, editing and deleting them needs an
account.
{%- endif %}
{%- if auth == "google" %}

For the Google button, create an OAuth client of type "Web application"
in the Google Cloud console, add `{{env_prefix}}_GOOGLE_REDIRECT_URL`
to its authorized redirect URIs, and set the three
`{{env_prefix}}_GOOGLE_*` variables. The flow uses the authorization
code with PKCE and reads the `email profile` scopes. A Google account
whose email is unknown still needs a valid invitation.
{%- endif %}

## Configuration

Read from the environment; `.env` is loaded first.

- `{{env_prefix}}_ADDR`: listen address, default `127.0.0.1:8080`
- `{{env_prefix}}_LOG`: `tracing` filter, default `info`
- `{{env_prefix}}_LOG_FORMAT`: `pretty` (default) or `json`
- `{{env_prefix}}_TEMPLATE_DIR`: default `templates`
- `{{env_prefix}}_TEMPLATE_RELOAD`: watch the template directory,
  default on in debug builds
- `{{env_prefix}}_STATIC_DIR`: served under `/static`, default `static`
- `{{env_prefix}}_SESSION_TTL`: seconds of inactivity before a session
  is dropped, default `86400`
- `{{env_prefix}}_COOKIE_SECURE`: send the session cookie over HTTPS
  only, default `false`; turn it on in production
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
{%- if auth != "none" %}
- `{{env_prefix}}_INVITE_TTL`: seconds an invitation stays usable,
  default `604800`
- `{{env_prefix}}_BOOTSTRAP_INVITE`: token accepted while no account
  exists, unset by default
{%- endif %}
{%- if auth == "google" %}
- `{{env_prefix}}_GOOGLE_CLIENT_ID`: required
- `{{env_prefix}}_GOOGLE_CLIENT_SECRET`: required
- `{{env_prefix}}_GOOGLE_REDIRECT_URL`: required, must match the client
{%- endif %}

## Assets

`assets/scss/app.scss` imports Bootstrap; override its variables in the
`with (...)` block. `npm run build` compiles it to `static/css/app.css`
and copies the Bootstrap bundle to `static/js/`. Both directories are
build output and are not committed.

## Docker

```sh
just docker-build                        # linux/amd64, loaded locally
just docker-build PLATFORM=linux/arm64
just docker-buildx ghcr.io/{{gh-username}}/{{project-name}}:latest  # multi-arch, pushed
just up                                  # docker compose
```

The image builds the stylesheet in a node stage and ships `templates/`
and `static/` next to the binary.

## License

[ISC](LICENSE)
