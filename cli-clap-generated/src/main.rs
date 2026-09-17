// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use cli_clap_generated::{args, error::OrFatal};

/// Log level for the given number of `-v` flags.
fn level(verbose: u8) -> &'static str {
    match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    }
}

fn main() {
    let opts = args::parse();
    env_logger::Builder::from_env(
        env_logger::Env::new()
            .filter("CLI_CLAP_GENERATED_LOG")
            .default_filter_or(level(opts.verbose)),
    )
    .format_timestamp(None)
    .init();
    run(opts).or_fatal();
}

fn run(opts: args::Opts) -> anyhow::Result<()> {
    println!("hello");
    for arg in &opts.args {
        log::info!("arg: {arg}");
    }
    log::trace!("done");
    Ok(())
}
