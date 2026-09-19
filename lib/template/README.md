# {{project-name}}

[![crates.io](https://img.shields.io/crates/v/{{project-name}}.svg)](https://crates.io/crates/{{project-name}})
[![docs.rs](https://docs.rs/{{project-name}}/badge.svg)](https://docs.rs/{{project-name}})

{{project-description}}

## Usage

```sh
cargo add {{project-name}}
```

```rust
use {{crate_name}}::Greeting;

let g = Greeting::new("world")?;
assert_eq!(g.to_string(), "hello, world");
```
{%- if serde %}

## Features

- `serde`: `Serialize`/`Deserialize` for public types.
{%- endif %}

## Minimum supported Rust version

1.85. Bumping it is a minor version change.

## License

[ISC](LICENSE)
