// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::net::SocketAddr;

use anyhow::Context;

use crate::cache;

/// Environment variable prefix.
pub const ENV_PREFIX: &str = "API_SQLITE_GENERATED";

/// Log output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

/// Per client rate limit applied to `/v1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimit {
    /// Sustained requests per second.
    pub per_second: u32,
    /// Requests allowed at once before the sustained rate applies.
    pub burst: u32,
    /// Take the client address from `X-Forwarded-For`, `X-Real-Ip` or
    /// `Forwarded` instead of the peer address.
    pub trust_proxy: bool,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            per_second: 10,
            burst: 50,
            trust_proxy: false,
        }
    }
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
    /// `<PREFIX>_RATE_LIMIT_RPS` (default `10`, `0` disables),
    /// `<PREFIX>_RATE_LIMIT_BURST` (default `50`) and
    /// `<PREFIX>_TRUST_PROXY` (default `false`).
    pub rate_limit: Option<RateLimit>,
    /// `DATABASE_URL`: connection string.
    pub database_url: String,
    /// `<PREFIX>_RUN_MIGRATIONS`: apply pending migrations at startup,
    /// default `true`.
    pub run_migrations: bool,
    /// `<PREFIX>_CACHE_TTL` (seconds, default `60`, `0` disables) and
    /// `<PREFIX>_CACHE_CAPACITY` (entries, default `10000`).
    pub cache: Option<cache::Config>,
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

        let defaults = RateLimit::default();
        let (key, rps) = var("RATE_LIMIT_RPS");
        let per_second: u32 = match rps {
            Some(v) => v
                .parse()
                .with_context(|| format!("{key}: expected a number"))?,
            None => defaults.per_second,
        };
        let (key, burst) = var("RATE_LIMIT_BURST");
        let burst: u32 = match burst {
            Some(v) => v
                .parse()
                .ok()
                .filter(|b| *b > 0)
                .with_context(|| format!("{key}: expected a number > 0"))?,
            None => defaults.burst,
        };
        let (key, trust_proxy) = var("TRUST_PROXY");
        let trust_proxy = match trust_proxy.as_deref().unwrap_or("false") {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" => false,
            other => {
                anyhow::bail!("{key}: expected true or false, got {other}")
            }
        };
        let rate_limit = (per_second > 0).then_some(RateLimit {
            per_second,
            burst,
            trust_proxy,
        });

        let database_url = get("DATABASE_URL")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| format!("sqlite://{}.db?mode=rwc", crate::PROG));

        let (key, run_migrations) = var("RUN_MIGRATIONS");
        let run_migrations = match run_migrations.as_deref().unwrap_or("true") {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" => false,
            other => {
                anyhow::bail!("{key}: expected true or false, got {other}")
            }
        };

        let defaults = cache::Config::default();
        let (key, ttl) = var("CACHE_TTL");
        let ttl: u64 = match ttl {
            Some(v) => v
                .parse()
                .with_context(|| format!("{key}: expected a number"))?,
            None => defaults.ttl.as_secs(),
        };
        let (key, capacity) = var("CACHE_CAPACITY");
        let capacity: u64 = match capacity {
            Some(v) => v
                .parse()
                .ok()
                .filter(|c| *c > 0)
                .with_context(|| format!("{key}: expected a number > 0"))?,
            None => defaults.capacity,
        };
        let cache = (ttl > 0).then_some(cache::Config {
            ttl: std::time::Duration::from_secs(ttl),
            capacity,
        });

        Ok(Self {
            addr,
            log,
            log_format,
            rate_limit,
            database_url,
            run_migrations,
            cache,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn parse(vars: &[(&str, &str)]) -> anyhow::Result<Config> {
        let map: HashMap<String, String> = vars
            .iter()
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
        assert_eq!(cfg.rate_limit, Some(RateLimit::default()));
        assert!(cfg.run_migrations);
        assert_eq!(cfg.cache, Some(cache::Config::default()));
    }

    #[test]
    fn parses_all_keys() {
        let cfg = parse(&[
            ("X_ADDR", "0.0.0.0:9000"),
            ("X_LOG", "debug"),
            ("X_LOG_FORMAT", "json"),
            ("X_RATE_LIMIT_RPS", "2"),
            ("X_RATE_LIMIT_BURST", "5"),
            ("X_TRUST_PROXY", "true"),
            ("DATABASE_URL", "sqlite://x"),
            ("X_RUN_MIGRATIONS", "false"),
            ("X_CACHE_TTL", "5"),
            ("X_CACHE_CAPACITY", "7"),
        ])
        .unwrap();
        assert_eq!(cfg.addr, "0.0.0.0:9000".parse().unwrap());
        assert_eq!(cfg.log, "debug");
        assert_eq!(cfg.log_format, LogFormat::Json);
        assert_eq!(
            cfg.rate_limit,
            Some(RateLimit {
                per_second: 2,
                burst: 5,
                trust_proxy: true,
            })
        );
        assert_eq!(cfg.database_url, "sqlite://x");
        assert!(!cfg.run_migrations);
        assert_eq!(
            cfg.cache,
            Some(cache::Config {
                ttl: std::time::Duration::from_secs(5),
                capacity: 7,
            })
        );
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
    fn rejects_bad_rate_limit() {
        let e = parse(&[("X_RATE_LIMIT_RPS", "fast")]).unwrap_err();
        assert!(e.to_string().contains("_RATE_LIMIT_RPS"), "{e}");
        let e = parse(&[("X_RATE_LIMIT_BURST", "0")]).unwrap_err();
        assert!(e.to_string().contains("_RATE_LIMIT_BURST"), "{e}");
        let e = parse(&[("X_TRUST_PROXY", "maybe")]).unwrap_err();
        assert!(e.to_string().contains("_TRUST_PROXY"), "{e}");
    }

    #[test]
    fn rate_limit_off() {
        let cfg = parse(&[
            ("X_RATE_LIMIT_RPS", "0"),
            ("X_RATE_LIMIT_BURST", "1"),
            ("X_TRUST_PROXY", "true"),
        ])
        .unwrap();
        assert_eq!(cfg.rate_limit, None);
    }

    #[test]
    fn cache_off() {
        let cfg = parse(&[("X_CACHE_TTL", "0")]).unwrap();
        assert_eq!(cfg.cache, None);
    }

    #[test]
    fn rejects_bad_cache() {
        let e = parse(&[("X_CACHE_TTL", "soon")]).unwrap_err();
        assert!(e.to_string().contains("_CACHE_TTL"), "{e}");
        let e = parse(&[("X_CACHE_CAPACITY", "0")]).unwrap_err();
        assert!(e.to_string().contains("_CACHE_CAPACITY"), "{e}");
    }
}
