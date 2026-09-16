# daemon

Unix daemon with a control socket: `<name>d` daemonizes, drops privileges,
handles signals, reloads its TOML config and answers requests on a Unix socket;
`<name>ctl` is the command-line client. Comes with mdoc man pages, rc.d,
systemd and launchd units, CI, pre-commit, dprint, typos and cargo-deny setup.

## Usage

```sh
cargo generate ijanc/templates daemon --name bar
```

Produces crate `bar` with binaries `bard` and `barctl`.

## Layout

```
template/
  cargo-generate.toml    placeholders, hooks, ignore list
  hooks/pre.rhai         derives daemon_name, ctl_name, ... (deleted on generate)
  Cargo.toml             lib + two bins
  build.rs               <ENV_PREFIX>_GIT_HASH / _BUILD_DATE
  src/lib.rs             program names, default paths
  src/config.rs          TOML config, validation
  src/control.rs         control socket, client handling
  src/daemon.rs          daemonize, privdrop, signals, poll
  src/error.rs           fatal helpers, strerror
  src/ipc.rs             request/response framing
  src/log.rs             syslog/stderr logger
  src/bin/<daemon>.rs    daemon main loop
  src/bin/<ctl>.rs       control client
  tests/ctl.rs           end-to-end test through the socket
  <daemon>.8, <ctl>.8, <daemon>.conf.5
  contrib/               example config, rc.d, systemd, launchd,
                         gen-security-key.sh
```

## Placeholders

Prompted:

| name                  | description                              |
| --------------------- | ---------------------------------------- |
| `project-name`        | crate name, kebab-case (built-in)        |
| `project-description` | `Cargo.toml` description, README         |
| `gh-username`         | repository URL, `FUNDING.yml`            |
| `domain`              | `security@`, `conduct@` contact addresses |

Derived in `hooks/pre.rhai`:

| name           | value                              |
| -------------- | ---------------------------------- |
| `crate_name`   | `project-name` in snake_case       |
| `daemon_name`  | `project-name` + `d`               |
| `ctl_name`     | `project-name` + `ctl`             |
| `env_prefix`   | `crate_name` in SHOUTY_SNAKE_CASE  |
| `plist_id`     | `org.<project-name>.<daemon_name>` |
| `author_name`  | name part of `authors`             |
| `author_email` | email part of `authors`            |
| `year`         | current UTC year                   |
| `mdocdate`     | current UTC date, `Month D YYYY`   |

## Notes

- `Justfile` and `.github/workflows/ci.yml` use `{% raw %}` around `{{...}}`
  that must survive generation.
- `SECURITY.md` ships with a `<!-- pgp-key -->` marker; `just security-key` in
  the generated project creates a key and fills it in.
- Name-bearing Rust lines are kept short or unbreakable so `rustfmt` output does
  not depend on the project name length.
