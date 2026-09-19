# lib-generated

[![crates.io](https://img.shields.io/crates/v/lib-generated.svg)](https://crates.io/crates/lib-generated)
[![docs.rs](https://docs.rs/lib-generated/badge.svg)](https://docs.rs/lib-generated)

An example generated using the lib template

## Usage

```sh
cargo add lib-generated
```

```rust
use lib_generated::Greeting;

let g = Greeting::new("world")?;
assert_eq!(g.to_string(), "hello, world");
```

## Minimum supported Rust version

1.85. Bumping it is a minor version change.

## License

[ISC](LICENSE)
