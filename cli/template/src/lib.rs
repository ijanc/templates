// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

pub mod args;
{%- if config %}
pub mod config;
{%- endif %}
pub mod error;

/// Program name, used in diagnostics.
pub const PROG: &str = "{{project-name}}";

{%- if config %}

/// Default configuration file, relative to `$XDG_CONFIG_HOME`
/// (or `$HOME/.config`).
pub const CONF_FILE: &str = "{{project-name}}/config.toml";

/// Absolute path of the default configuration file.
pub fn conf_file() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = std::path::PathBuf::from(h);
                p.push(".config");
                p
            })
        })
        .unwrap_or_default();
    base.join(CONF_FILE)
}
{%- endif %}

/// Package version followed by the git hash and build date recorded by
/// `build.rs`, when available: `0.1.0 (abc1234 2026-01-31)`.
pub fn version() -> String {
    let mut v = env!("CARGO_PKG_VERSION").to_string();
    let hash = env!("{{env_prefix}}_GIT_HASH");
    let date = env!("{{env_prefix}}_BUILD_DATE");
    let extra: Vec<&str> =
        [hash, date].into_iter().filter(|s| !s.is_empty()).collect();
    if !extra.is_empty() {
        v.push_str(&format!(" ({})", extra.join(" ")));
    }
    v
}
