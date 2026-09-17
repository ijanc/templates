// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use cli_generated::{args, config::Config, error::OrFatal};

/// Log level for the given number of `-v` flags.
fn level(verbose: u8) -> &'static str {
    match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    }
}

fn main() {
    dotenvy::dotenv().ok();
    let opts = args::parse();
    env_logger::Builder::from_env(
        env_logger::Env::new()
            .filter("CLI_GENERATED_LOG")
            .default_filter_or(level(opts.verbose)),
    )
    .format_timestamp(None)
    .init();
    run(opts).or_fatal();
}

fn run(opts: args::Opts) -> anyhow::Result<()> {
    let conf_is_default = opts.conf.is_none();
    let conf_path = opts.conf.unwrap_or_else(cli_generated::conf_file);
    let cfg = Config::load(&conf_path, conf_is_default)?;
    log::debug!("config: {cfg:?}");
    println!("hello, {}", cfg.name);
    for arg in &opts.args {
        log::info!("arg: {arg}");
    }
    log::trace!("done");
    Ok(())
}
