// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::Context;
{% if auth == "none" %}
use crate::{render, session};
{%- else %}
use crate::{auth, render, session};
{%- endif %}

/// Environment variable prefix.
pub const ENV_PREFIX: &str = "{{env_prefix}}";

/// Log output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

/// Runtime configuration, read from the environment (and `.env`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// `<PREFIX>_ADDR`: listen address, default `127.0.0.1:8080`.
    pub addr: SocketAddr,
    /// `<PREFIX>_LOG`: `tracing` filter directives, default `info`.
    pub log: String,
    /// `<PREFIX>_LOG_FORMAT`: `pretty` (default) or `json`.
    pub log_format: LogFormat,
    /// `<PREFIX>_TEMPLATE_DIR` (default `templates`) and
    /// `<PREFIX>_TEMPLATE_RELOAD` (default on in debug builds).
    pub templates: render::Config,
    /// `<PREFIX>_STATIC_DIR`: files served under `/static`, default
    /// `static`.
    pub static_dir: PathBuf,
    /// `<PREFIX>_SESSION_TTL` (seconds, default `86400`) and
    /// `<PREFIX>_COOKIE_SECURE` (default `false`).
    pub session: session::Config,
{%- if sqlx %}
    /// `DATABASE_URL`: connection string.
    pub database_url: String,
    /// `<PREFIX>_RUN_MIGRATIONS`: apply pending migrations at startup,
    /// default `true`.
    pub run_migrations: bool,
{%- endif %}
{%- if auth != "none" %}
    /// `<PREFIX>_INVITE_TTL` (seconds, default `604800`) and
    /// `<PREFIX>_BOOTSTRAP_INVITE` (unset by default){% if auth == "google" %}, plus the
    /// `<PREFIX>_GOOGLE_*` credentials, which are required{% endif %}.
    pub auth: auth::Config,
{%- endif %}
}

/// Read a `true`/`false` variable, or `default` when it is unset.
fn flag(
    key: &str,
    value: Option<String>,
    default: bool,
) -> anyhow::Result<bool> {
    match value.as_deref() {
        None => Ok(default),
        Some("true" | "1" | "yes") => Ok(true),
        Some("false" | "0" | "no") => Ok(false),
        Some(other) => {
            anyhow::bail!("{key}: expected true or false, got {other}")
        }
    }
}

impl Config {
    /// Read the process environment.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_vars(|k| std::env::var(k).ok())
    }

    /// Read from `get`, which returns the value of a variable or `None`.
    pub fn from_vars(
        get: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let var = |name: &str| {
            let key = format!("{ENV_PREFIX}_{name}");
            (key.clone(), get(&key).filter(|v| !v.is_empty()))
        };

        let (key, addr) = var("ADDR");
        let addr = addr
            .as_deref()
            .unwrap_or("127.0.0.1:8080")
            .parse()
            .with_context(|| format!("{key}: invalid socket address"))?;

        let (_, log) = var("LOG");
        let log = log.unwrap_or_else(|| "info".into());

        let (key, log_format) = var("LOG_FORMAT");
        let log_format = match log_format.as_deref().unwrap_or("pretty") {
            "pretty" => LogFormat::Pretty,
            "json" => LogFormat::Json,
            other => {
                anyhow::bail!("{key}: expected pretty or json, got {other}")
            }
        };

        let defaults = render::Config::default();
        let (_, dir) = var("TEMPLATE_DIR");
        let dir = dir.map_or(defaults.dir, PathBuf::from);
        let (key, reload) = var("TEMPLATE_RELOAD");
        let reload = flag(&key, reload, defaults.reload)?;
        let templates = render::Config { dir, reload };

        let (_, static_dir) = var("STATIC_DIR");
        let static_dir =
            static_dir.map_or_else(|| PathBuf::from("static"), PathBuf::from);

        let defaults = session::Config::default();
        let (key, ttl) = var("SESSION_TTL");
        let ttl: u64 = match ttl {
            Some(v) => v
                .parse()
                .ok()
                .filter(|s| *s > 0)
                .with_context(|| format!("{key}: expected a number > 0"))?,
            None => defaults.ttl.as_secs(),
        };
        let (key, secure) = var("COOKIE_SECURE");
        let secure = flag(&key, secure, defaults.secure)?;
        let session = session::Config {
            ttl: Duration::from_secs(ttl),
            secure,
        };
{%- if sqlx %}

        let database_url = get("DATABASE_URL")
            .filter(|v| !v.is_empty())
{%- if store == "sqlite" %}
            .unwrap_or_else(|| format!("sqlite://{}.db?mode=rwc", crate::PROG));
{%- else %}
            .context("DATABASE_URL: not set")?;
{%- endif %}

        let (key, run_migrations) = var("RUN_MIGRATIONS");
        let run_migrations = flag(&key, run_migrations, true)?;
{%- endif %}
{%- if auth != "none" %}

        let (key, invite_ttl) = var("INVITE_TTL");
        let invite_ttl: u64 = match invite_ttl {
            Some(v) => v
                .parse()
                .ok()
                .filter(|s| *s > 0)
                .with_context(|| format!("{key}: expected a number > 0"))?,
            None => crate::INVITE_TTL.as_secs(),
        };
        let invite_ttl = Duration::from_secs(invite_ttl);

        let (_, bootstrap_invite) = var("BOOTSTRAP_INVITE");
{%- endif %}
{%- if auth == "google" %}

        let (key, client_id) = var("GOOGLE_CLIENT_ID");
        let client_id = client_id.with_context(|| format!("{key}: not set"))?;
        let (key, secret) = var("GOOGLE_CLIENT_SECRET");
        let client_secret =
            secret.with_context(|| format!("{key}: not set"))?;
        let (key, redirect) = var("GOOGLE_REDIRECT_URL");
        let redirect_url =
            redirect.with_context(|| format!("{key}: not set"))?;
        let google = auth::GoogleConfig {
            client_id,
            client_secret,
            redirect_url,
        };
{%- endif %}
{%- if auth != "none" %}

        let auth = auth::Config {
            invite_ttl,
            bootstrap_invite,
{%- if auth == "google" %}
            google,
{%- endif %}
        };
{%- endif %}

        Ok(Self {
            addr,
            log,
            log_format,
            templates,
            static_dir,
            session,
{%- if sqlx %}
            database_url,
            run_migrations,
{%- endif %}
{%- if auth != "none" %}
            auth,
{%- endif %}
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// Variables every variant needs before it parses at all.
{%- if store == "postgres" and auth == "google" %}
    const REQUIRED: &[(&str, &str)] = &[
        ("DATABASE_URL", "postgres://localhost/x"),
        ("X_GOOGLE_CLIENT_ID", "id"),
        ("X_GOOGLE_CLIENT_SECRET", "secret"),
        ("X_GOOGLE_REDIRECT_URL", "http://localhost/cb"),
    ];
{%- elsif store == "postgres" %}
    const REQUIRED: &[(&str, &str)] =
        &[("DATABASE_URL", "postgres://localhost/x")];
{%- elsif auth == "google" %}
    const REQUIRED: &[(&str, &str)] = &[
        ("X_GOOGLE_CLIENT_ID", "id"),
        ("X_GOOGLE_CLIENT_SECRET", "secret"),
        ("X_GOOGLE_REDIRECT_URL", "http://localhost/cb"),
    ];
{%- else %}
    const REQUIRED: &[(&str, &str)] = &[];
{%- endif %}

    fn parse(vars: &[(&str, &str)]) -> anyhow::Result<Config> {
        let map: HashMap<String, String> = REQUIRED
            .iter()
            .chain(vars)
            .map(|(k, v)| {
                (k.replace("X_", &format!("{ENV_PREFIX}_")), v.to_string())
            })
            .collect();
        Config::from_vars(|k| map.get(k).cloned())
    }

    #[test]
    fn defaults() {
        let cfg = parse(&[]).unwrap();
        assert_eq!(cfg.addr, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(cfg.log, "info");
        assert_eq!(cfg.log_format, LogFormat::Pretty);
        assert_eq!(cfg.templates, render::Config::default());
        assert_eq!(cfg.static_dir, PathBuf::from("static"));
        assert_eq!(cfg.session, session::Config::default());
{%- if sqlx %}
        assert!(cfg.run_migrations);
{%- endif %}
{%- if auth != "none" %}
        assert_eq!(cfg.auth.invite_ttl, crate::INVITE_TTL);
        assert_eq!(cfg.auth.bootstrap_invite, None);
{%- endif %}
    }

    #[test]
    fn parses_all_keys() {
        let cfg = parse(&[
            ("X_ADDR", "0.0.0.0:9000"),
            ("X_LOG", "debug"),
            ("X_LOG_FORMAT", "json"),
            ("X_TEMPLATE_DIR", "views"),
            ("X_TEMPLATE_RELOAD", "true"),
            ("X_STATIC_DIR", "public"),
            ("X_SESSION_TTL", "60"),
            ("X_COOKIE_SECURE", "true"),
{%- if sqlx %}
            ("DATABASE_URL", "{{store}}://x"),
            ("X_RUN_MIGRATIONS", "false"),
{%- endif %}
{%- if auth != "none" %}
            ("X_INVITE_TTL", "90"),
            ("X_BOOTSTRAP_INVITE", "open-sesame"),
{%- endif %}
        ])
        .unwrap();
        assert_eq!(cfg.addr, "0.0.0.0:9000".parse().unwrap());
        assert_eq!(cfg.log, "debug");
        assert_eq!(cfg.log_format, LogFormat::Json);
        assert_eq!(cfg.templates.dir, PathBuf::from("views"));
        assert!(cfg.templates.reload);
        assert_eq!(cfg.static_dir, PathBuf::from("public"));
        assert_eq!(cfg.session.ttl, Duration::from_secs(60));
        assert!(cfg.session.secure);
{%- if sqlx %}
        assert_eq!(cfg.database_url, "{{store}}://x");
        assert!(!cfg.run_migrations);
{%- endif %}
{%- if auth != "none" %}
        assert_eq!(cfg.auth.invite_ttl, Duration::from_secs(90));
        let bootstrap = cfg.auth.bootstrap_invite.as_deref();
        assert_eq!(bootstrap, Some("open-sesame"));
{%- endif %}
    }

    #[test]
    fn rejects_bad_addr() {
        let e = parse(&[("X_ADDR", "nope")]).unwrap_err();
        assert!(e.to_string().contains("_ADDR"), "{e}");
    }

    #[test]
    fn rejects_bad_log_format() {
        let e = parse(&[("X_LOG_FORMAT", "xml")]).unwrap_err();
        assert!(e.to_string().contains("_LOG_FORMAT"), "{e}");
    }

    #[test]
    fn rejects_bad_session() {
        let e = parse(&[("X_SESSION_TTL", "0")]).unwrap_err();
        assert!(e.to_string().contains("_SESSION_TTL"), "{e}");
        let e = parse(&[("X_COOKIE_SECURE", "maybe")]).unwrap_err();
        assert!(e.to_string().contains("_COOKIE_SECURE"), "{e}");
    }

    #[test]
    fn rejects_bad_template_reload() {
        let e = parse(&[("X_TEMPLATE_RELOAD", "sometimes")]).unwrap_err();
        assert!(e.to_string().contains("_TEMPLATE_RELOAD"), "{e}");
    }
{%- if auth != "none" %}

    #[test]
    fn rejects_bad_invite_ttl() {
        let e = parse(&[("X_INVITE_TTL", "soon")]).unwrap_err();
        assert!(e.to_string().contains("_INVITE_TTL"), "{e}");
    }
{%- endif %}
{%- if store == "postgres" %}

    #[test]
    fn requires_database_url() {
        let map: HashMap<String, String> = HashMap::new();
        let e = Config::from_vars(|k| map.get(k).cloned()).unwrap_err();
        assert!(e.to_string().contains("DATABASE_URL"), "{e}");
    }
{%- endif %}
{%- if auth == "google" %}

    #[test]
    fn requires_google_credentials() {
        let map: HashMap<String, String> = REQUIRED
            .iter()
            .filter(|(k, _)| *k != "X_GOOGLE_CLIENT_SECRET")
            .map(|(k, v)| {
                (k.replace("X_", &format!("{ENV_PREFIX}_")), v.to_string())
            })
            .collect();
        let e = Config::from_vars(|k| map.get(k).cloned()).unwrap_err();
        assert!(e.to_string().contains("_GOOGLE_CLIENT_SECRET"), "{e}");
    }
{%- endif %}
}
