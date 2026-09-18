# templates

Project templates for [cargo-generate](https://cargo-generate.github.io/cargo-generate/).

## Usage

```sh
cargo install cargo-generate
cargo generate ijanc/templates
```

Or pick a template directly:

```sh
cargo generate ijanc/templates daemon --name bar
```

## Templates

| name                | description                                                         |
| ------------------- | ------------------------------------------------------------------- |
| [daemon](./daemon/) | Unix daemon with control socket, man pages and CI                   |
| [cli](./cli/)       | Command line tool with getopt/clap, env_logger, optional TOML/.env   |
| [api](./api/)       | axum REST API with OpenAPI, metrics, memory/SQLite/PostgreSQL store |
| [web](./web/)       | axum web app with MiniJinja, sessions, CSRF, Bootstrap, invites     |

Each `<name>-generated/` directory is the committed output of
`just generate-<name>`, so changes to a template show up as a diff.

## License

[ISC](LICENSE)
