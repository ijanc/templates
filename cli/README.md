# cli

Command line tool: argument parsing with `getopt` or `clap`, `-V` printing
version, git hash and build date, logging to stderr through `env_logger`,
optional TOML configuration file and optional `.env` loading. Comes with an
mdoc man page, CI, pre-commit, dprint, typos and cargo-deny setup.

## Usage

```sh
cargo generate ijanc/templates cli --name bar
```

Produces crate `bar` with binary `bar`.

## Layout

```
template/
  cargo-generate.toml    placeholders, hooks, ignore list
  hooks/pre.rhai         derives crate_name, ..., picks src/args.rs
                         (deleted on generate)
  Cargo.toml             lib + one bin
  build.rs               <ENV_PREFIX>_GIT_HASH / _BUILD_DATE
  src/lib.rs             program name, version(), default config path
  src/args_getopt.rs     getopt parser        -> src/args.rs (args=getopt)
  src/args_clap.rs       clap derive parser   -> src/args.rs (args=clap)
  src/config.rs          TOML config, validation (config=true)
  src/error.rs           fatal helpers, strerror
  src/main.rs            .env, args, env_logger, config, run()
  tests/cli.rs           runs the binary, checks status and output
  <name>.1               man page
  <name>.conf.5          config man page (config=true)
  contrib/               example config (config=true), gen-security-key.sh
  .env.example           (dotenv=true)
```

Both `src/args_*.rs` expose the same `Opts` struct and `parse()` function so
`src/main.rs` is identical for either parser.

## Placeholders

Prompted:

| name                  | description                                   |
| --------------------- | --------------------------------------------- |
| `project-name`        | crate and binary name, kebab-case (built-in)  |
| `project-description` | `Cargo.toml` description, README, man page    |
| `gh-username`         | repository URL, `FUNDING.yml`                 |
| `domain`              | `security@`, `conduct@` contact addresses     |
| `args`                | `getopt` (default) or `clap`                  |
| `config`              | TOML config file, `-f`, `<name>.conf(5)`      |
| `dotenv`              | load `.env` with `dotenvy`, ship `.env.example` |

Derived in `hooks/pre.rhai`:

| name           | value                             |
| -------------- | --------------------------------- |
| `crate_name`   | `project-name` in snake_case      |
| `env_prefix`   | `crate_name` in SHOUTY_SNAKE_CASE |
| `author_name`  | name part of `authors`            |
| `author_email` | email part of `authors`           |
| `year`         | current UTC year                  |
| `mdocdate`     | current UTC date, `Month D YYYY`  |

## Notes

- Logging is controlled by `<ENV_PREFIX>_LOG` (env_logger filter syntax);
  `-v` only sets the default level, the variable takes precedence.
- Default config file is `$XDG_CONFIG_HOME/<name>/config.toml`, falling
  back to `$HOME/.config`. A missing default file yields the defaults; a
  missing file given with `-f` is an error.
- `Justfile` and `.github/workflows/ci.yml` use `{% raw %}` around `{{...}}`
  that must survive generation.
- `SECURITY.md` ships with a `<!-- pgp-key -->` marker; `just security-key` in
  the generated project creates a key and fills it in.
- Name-bearing Rust lines are kept short or unbreakable so `rustfmt` output does
  not depend on the project name length.
