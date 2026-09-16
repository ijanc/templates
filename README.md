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

| name                | description                                            |
| ------------------- | ------------------------------------------------------ |
| [daemon](./daemon/) | Unix daemon with control socket, man pages and CI     |

Each `<name>-generated/` directory is the committed output of
`just generate-<name>`, so changes to a template show up as a diff.

## License

[ISC](LICENSE)
