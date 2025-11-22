# {{project-name}}

`{{project-name}}` {{project-description}}

## Why

## Features

- Feature 1
- Feature 2
- Feature 3

## Requirements

- Rust toolchain (Rust 1.89 or newer recommended)

To install Rust:

```sh
curl https://sh.rustup.rs -sSf | sh
```

Verify installation:

```sh
rustc --version
cargo --version
```

## Building

Clone the repository and build the binary:

```sh
git clone ssh://anon@ijanc.org/{{project-name}}
cd {{project-name}}
cargo build --release
```

The binary will be located at:

```
target/release/{{project-name}}
```

You can add it to your PATH, move it to `/usr/local/bin`.

or

```
cargo install --path .
```

## Command Overview

`{{project-name}}` currently supports one primary operation:

### Command 1

## Logging and verbosity

`{{project-name}}` uses `tracing` for structured logging.

By default, logs are shown at the INFO level.\
Use `-v` to enable DEBUG logs:

```
{{project-name}} -v ...
```

Or configure via `RUST_LOG`:

```
RUST_LOG=debug {{project-name}} ...
```

## License

This project is licensed under the ISC license ([LICENSE](LICENSE) or\
http://opensource.org/licenses/ISC)
