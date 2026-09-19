# lib-serde-generated

[![crates.io](https://img.shields.io/crates/v/lib-serde-generated.svg)](https://crates.io/crates/lib-serde-generated)
[![docs.rs](https://docs.rs/lib-serde-generated/badge.svg)](https://docs.rs/lib-serde-generated)

An example generated using the lib template

## Usage

```sh
cargo add lib-serde-generated
```

```rust
use lib_serde_generated::Greeting;

let g = Greeting::new("world")?;
assert_eq!(g.to_string(), "hello, world");
```

## Features

- `serde`: `Serialize`/`Deserialize` for public types.

## Minimum supported Rust version

1.85. Bumping it is a minor version change.

## License

[ISC](LICENSE)
