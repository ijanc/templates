//
// Copyright (c) {{ "today" | date: "%Y" }} {{ authors }}
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//

use anyhow::Result;
use clap::{ArgAction, Parser};
{% if project-diagnosis == "log" -%}
use log::{debug, info};
{% else -%}
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;
{% endif %}

const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_HASH", "unknown"),
    " ",
    env!("BUILD_DATE", "unknown"),
    ")",
);

#[derive(Debug, Parser)]
#[command(
    name = "{{project-name}}",
    about = "{{project-description}}",
    version = VERSION,
    author,
    propagate_version = true
)]
struct Args {
    /// Increase verbosity (use -v, -vv, ...).
    ///
    /// When no RUST_LOG is set, a single -v switches the log level to DEBUG.
    #[arg(short, long, global = true, action = ArgAction::Count)]
    verbose: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();

    init_logger(args.verbose);

    info!("hello world cli");
    debug!("hello world cli");

    Ok(())
}

{% if project-diagnosis == "log" -%}
/// Initialize log based on RUST_LOG and the CLI verbosity.
///
/// Rules:
/// - If RUST_LOG is set, it is fully respected.
/// - If RUST_LOG is not set and verbose == 0 -> INFO level.
/// - If RUST_LOG is not set and verbose  > 0 -> DEBUG level.
fn init_logger(verbose: u8) {
    use std::io::Write;
    use log::LevelFilter;

    if std::env::var_os("RUST_LOG").is_some() {
        env_logger::builder()
            .format(|buf, record| {
                writeln!(buf, "[{}]: {}", record.level(), record.args())
            })
            .init();
        return;
    }

    let level =
        if verbose > 0 { LevelFilter::Debug } else { LevelFilter::Info };

    env_logger::builder()
        .filter(None, level)
        .format(|buf, record| {
            writeln!(buf, "[{}]: {}", record.level(), record.args())
        })
        .init();
}
{% else -%}
/// Initialize tracing based on RUST_LOG and the CLI verbosity.
///
/// Rules:
/// - If RUST_LOG is set, it is fully respected.
/// - If RUST_LOG is not set and verbose == 0 -> INFO level.
/// - If RUST_LOG is not set and verbose  > 0 -> DEBUG level.
fn init_logger(verbose: u8) {
    if std::env::var_os("RUST_LOG").is_some() {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
        return;
    }

    let filter = if verbose > 0 {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
{% endif %}
