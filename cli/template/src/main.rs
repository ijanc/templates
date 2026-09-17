// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

{% if config -%}
use {{crate_name}}::{args, config::Config, error::OrFatal};
{%- else -%}
use {{crate_name}}::{args, error::OrFatal};
{%- endif %}

/// Log level for the given number of `-v` flags.
fn level(verbose: u8) -> &'static str {
    match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    }
}

fn main() {
{%- if dotenv %}
    dotenvy::dotenv().ok();
{%- endif %}
    let opts = args::parse();
    env_logger::Builder::from_env(
        env_logger::Env::new()
            .filter("{{env_prefix}}_LOG")
            .default_filter_or(level(opts.verbose)),
    )
    .format_timestamp(None)
    .init();
    run(opts).or_fatal();
}

fn run(opts: args::Opts) -> anyhow::Result<()> {
{%- if config %}
    let conf_is_default = opts.conf.is_none();
    let conf_path = opts.conf.unwrap_or_else({{crate_name}}::conf_file);
    let cfg = Config::load(&conf_path, conf_is_default)?;
    log::debug!("config: {cfg:?}");
    println!("hello, {}", cfg.name);
{%- else %}
    println!("hello");
{%- endif %}
    for arg in &opts.args {
        log::info!("arg: {arg}");
    }
    log::trace!("done");
    Ok(())
}
