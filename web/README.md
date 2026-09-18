# web

Server-rendered web application on axum: `/items` CRUD with HTML forms,
MiniJinja templates that reload as you edit them, cookie sessions
(`tower-sessions`), a CSRF token on every form, flash messages carried
across redirects, HTML error pages, `X-Request-Id` set and propagated on
every request, `tracing` logs (pretty or JSON), a store that is
in-memory, SQLite or PostgreSQL (sqlx, embedded migrations), optional
invite-only accounts with Argon2 passwords and Google sign in, and
Bootstrap styling compiled from SCSS with `sass`. Configuration comes
from the environment and `.env`. Ships a multi-stage Dockerfile that
builds the stylesheet in a node stage, a compose file, CI, pre-commit,
dprint, typos and cargo-deny setup.

## Usage

```sh
cargo generate ijanc/templates web --name bar
```

Produces crate `bar` with binary `bar`.

## Layout

```
template/
  cargo-generate.toml    placeholders, hooks, ignore and exclude lists
  hooks/pre.rhai         derives crate_name, ..., picks src/store.rs,
                         drops the auth files (deleted on generate)
  Cargo.toml             lib + one bin
  build.rs               <ENV_PREFIX>_GIT_HASH / _BUILD_DATE
  src/lib.rs             AppState, app() router with the middleware stack
  src/main.rs            .env, Config, telemetry, store, sessions, serve
  src/config.rs          Config::from_env()
  src/telemetry.rs       tracing subscriber, request span
  src/request_id.rs      X-Request-Id layers
  src/render.rs          MiniJinja environment, Renderer extractor
  src/session.rs         session store and cookie layer
  src/flash.rs           one-shot message kept in the session
  src/csrf.rs            per session token, verify middleware
  src/auth.rs            login, CurrentUser, guard, invites (auth != none)
  src/error.rs           Error -> HTML error page middleware
  src/extract.rs         Form/Path/Query with Error rejections
  src/model.rs           Item, CreateItem, UpdateItem, ListQuery, Page,
                         User, Invite and the form types
  src/routes/mod.rs      /, /healthz, /readyz, /static, nested routers
  src/routes/items.rs    item pages
  src/routes/auth.rs     login, register, invites, OAuth (auth != none)
  src/store_memory.rs    RwLock<HashMap>  -> src/store.rs (store=memory)
  src/store_sqlx.rs      sqlx pool        -> src/store.rs (store=sqlite|postgres)
  migrations/            sqlx migrations (store=sqlite|postgres)
  templates/             MiniJinja pages, copied through untouched
  assets/scss/app.scss   Bootstrap import and variable overrides
  static/robots.txt      the only committed static file; css and js are built
  package.json           bootstrap + sass, build:css and build:js
  tests/common/mod.rs    spawn() on an ephemeral port, with a cookie jar
  tests/items.rs         forms, redirects, flash, validation
  tests/auth.rs          invites, register, login, guard (auth != none)
  tests/infra.rs         probes, request id, static files, error pages, CSRF
  Dockerfile             node assets stage, rust builder, debian runtime
  compose.yaml           app (+ db for postgres)
  .env.example           every variable with its default
```

Both `src/store_*.rs` expose the same `Store` methods, so the handlers
are identical for every backend. Queries are plain strings with bind
parameters, no `DATABASE_URL` is needed at build time.

`templates/` is listed in `exclude`, so cargo-generate copies it without
running the Liquid engine over it: the `{{ }}` and `{% %}` in those files
belong to MiniJinja. Nothing there names the project; the templates read
`app_name`, `auth` and `auth_google` from the render context instead.

## Placeholders

Prompted:

| name                  | description                                     |
| --------------------- | ----------------------------------------------- |
| `project-name`        | crate and binary name, kebab-case (built-in)    |
| `project-description` | `Cargo.toml` description, README                |
| `gh-username`         | repository URL, `FUNDING.yml`                   |
| `domain`              | `security@`, `conduct@` contact addresses       |
| `store`               | `memory`, `sqlite` (default) or `postgres`      |
| `auth`                | `none`, `local` (default) or `google`           |

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
  `_LOG_FORMAT`, `_TEMPLATE_DIR`, `_TEMPLATE_RELOAD`, `_STATIC_DIR`,
  `_SESSION_TTL`, `_COOKIE_SECURE`, with sqlx `DATABASE_URL` and
  `_RUN_MIGRATIONS`, with accounts `_INVITE_TTL` and `_BOOTSTRAP_INVITE`,
  and with Google `_GOOGLE_CLIENT_ID`, `_GOOGLE_CLIENT_SECRET` and
  `_GOOGLE_REDIRECT_URL`.
- The same code path serves dev and release: `AutoReloader` builds the
  environment once and, when `_TEMPLATE_RELOAD` is on, watches the
  directory and clears the template cache on change. The Dockerfile
  copies `templates/` next to the binary and turns reloading off.
- `Renderer` is the extractor every page takes. It holds the session and
  merges `app_name`, `version`, `path`, `csrf_token`, `flash` and `user`
  into the context, so a template can rely on them being there.
  `renderer.redirect(path, flash)` answers `303` and leaves the message
  for the next request, which is what every successful post does.
- `csrf::verify` runs on `POST`, `PUT`, `PATCH` and `DELETE`. It buffers
  the body, compares the `csrf_token` field with the session value in
  constant time and rebuilds the request, so handlers still see the body.
  It sits inside the session layer and outside the error page middleware,
  so a mismatch renders the 403 page.
- Error responses are built without the request at hand, so
  `error::page_middleware` runs as middleware, renders
  `errors/<status>.html` when it exists and `errors/error.html`
  otherwise, and gives bodiless 404/405/408 a page too. Failing to render
  the error page falls back to a hardcoded line of HTML.
- Form validation is not an error page: handlers call `validate()`
  themselves and re-render the form with the messages beside the fields
  and status `422`.
- Accounts are invite only. `<PREFIX>_BOOTSTRAP_INVITE` is accepted as a
  token while the users table is empty, which is how the first account is
  made; every later one needs an invitation created at `/invites`. An
  invitation is claimed before the account is created and handed back if
  the account cannot be made, so it is never spent for nothing.
- `auth=google` is `auth=local` plus Google: the password form stays on
  the same `/login` page, the OAuth flow is authorization code with PKCE,
  and an unknown email still needs a valid invitation. The token exchange
  and the userinfo call go through the `reqwest` that `oauth2` bundles.
- `tower-sessions` is pinned to `0.14`, the version
  `tower-sessions-sqlx-store` is built against; the database stores share
  the application's connection pool and sweep expired rows in the
  background.
- `cargo test` needs no npm: the pages reference `/static/css/app.css`,
  which simply 404s until `npm run build` has run. The generated CI has
  its own `assets` job for that.
- Tests resolve the template and static directories from
  `CARGO_MANIFEST_DIR`, not the working directory, and drive the app with
  a `reqwest` cookie jar, so sessions, flash messages and form tokens
  behave the way they do in a browser. PostgreSQL tests give each spawn
  its own schema and skip entirely when `DATABASE_URL` is unset; CI
  provides a service container.
- `Justfile` and `.github/workflows/ci.yml` use `{% raw %}` around `{{...}}`
  that must survive generation.
- `SECURITY.md` ships with a `<!-- pgp-key -->` marker; `just security-key` in
  the generated project creates a key and fills it in.
- Name-bearing Rust lines are kept short or unbreakable, and the tests import
  crate items through `tests/common`, so `rustfmt` output does not depend on
  the project name.
- The Dockerfile builds with `--locked`, so commit `Cargo.lock`.
