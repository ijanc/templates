// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use anyhow::Context;
use tracing_subscriber::EnvFilter;

use crate::config::{Config, ENV_PREFIX, LogFormat};

/// Install the global `tracing` subscriber.
pub fn init(cfg: &Config) -> anyhow::Result<()> {
    let filter = EnvFilter::try_new(&cfg.log)
        .with_context(|| format!("{ENV_PREFIX}_LOG: invalid filter"))?;
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match cfg.log_format {
        LogFormat::Pretty => builder.init(),
        LogFormat::Json => builder.json().flatten_event(true).init(),
    }
    Ok(())
}
