# lib

Library crate: typed errors with `thiserror`, optional `serde` feature,
crate-level docs with a doctest, `#![warn(missing_docs)]`, MSRV, changelog,
docs.rs metadata. Comes with CI (fmt, clippy, test on Linux/macOS/Windows,
docs, MSRV, cargo-deny licenses and advisories), a tag-triggered publish
workflow, pre-commit, dprint and typos setup.

## Usage

```sh
cargo generate ijanc/templates lib --name bar
```

Produces crate `bar`, library `bar`.

## Layout

```
template/
  cargo-generate.toml            placeholders, hooks, ignore list
  hooks/pre.rhai                 derives crate_name, year (deleted on generate)
  Cargo.toml                     features, docs.rs metadata, rust-version
  src/lib.rs                     crate docs, lints, example type
  src/error.rs                   thiserror enum, Result alias
  tests/integration.rs           public API tests, serde roundtrip (serde=true)
  CHANGELOG.md                   Keep a Changelog skeleton
  .github/workflows/ci.yml       fmt, typos, clippy, test, docs, msrv, deny
  .github/workflows/release.yml  cargo publish on v* tags
  contrib/                       gen-security-key.sh
```

## Placeholders

Prompted:

| name                  | description                                  |
| --------------------- | -------------------------------------------- |
| `project-name`        | crate name, kebab-case (built-in)            |
| `project-description` | `Cargo.toml` description, README, crate docs |
| `gh-username`         | repository URL, `FUNDING.yml`, changelog     |
| `domain`              | `security@`, `conduct@` contact addresses    |
| `serde`               | optional `serde` feature with derives        |

Derived in `hooks/pre.rhai`:

| name         | value                        |
| ------------ | ---------------------------- |
| `crate_name` | `project-name` in snake_case |
| `year`       | current UTC year             |

## Notes

- MSRV is `rust-version` in `Cargo.toml`, checked by the `msrv` CI job and
  `just msrv`; bump both together.
- `release.yml` refuses to publish when the tag does not match the crate
  version and needs a `CARGO_REGISTRY_TOKEN` secret in the `crates-io`
  environment.
- `Justfile` and `.github/workflows/*.yml` use `{% raw %}` around `{{...}}`
  that must survive generation.
- `SECURITY.md` ships with a `<!-- pgp-key -->` marker; `just security-key` in
  the generated project creates a key and fills it in.
