// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use std::{fs, io, path::Path};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::error::{Result, strerror};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Example string setting.
    pub name: String,
    /// Example numeric setting.
    pub retries: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "world".into(),
            retries: 3,
        }
    }
}

impl Config {
    /// Read and validate `path`.
    /// When `is_default` is set a missing file yields the defaults;
    /// otherwise it is an error.
    pub fn load(path: &Path, is_default: bool) -> Result<Self> {
        let s = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if is_default && e.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => {
                anyhow::bail!("{}: {}", path.display(), strerror(&e))
            }
        };
        let cfg: Self = toml::from_str(&s)
            .with_context(|| format!("{}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(!self.name.is_empty(), "name must not be empty");
        anyhow::ensure!(self.retries <= 10, "retries must be at most 10");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Config> {
        let cfg: Config = toml::from_str(s)?;
        cfg.validate()?;
        Ok(cfg)
    }

    #[test]
    fn defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.name, "world");
        assert_eq!(cfg.retries, 3);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn parses_all_keys() {
        let cfg = parse("name = \"x\"\nretries = 5\n").unwrap();
        assert_eq!(cfg.name, "x");
        assert_eq!(cfg.retries, 5);
    }

    #[test]
    fn rejects_empty_name() {
        let e = parse("name = \"\"\n").unwrap_err();
        assert!(e.to_string().contains("name"));
    }

    #[test]
    fn rejects_too_many_retries() {
        let e = parse("retries = 11\n").unwrap_err();
        assert!(e.to_string().contains("retries"));
    }

    #[test]
    fn rejects_unknown_key() {
        let e = parse("bogus = 1\n").unwrap_err();
        assert!(e.to_string().contains("bogus"));
    }

    #[test]
    fn missing_default_file_is_empty_config() {
        let cfg = Config::load(Path::new("/nonexistent/x.toml"), true).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn missing_explicit_file_is_error() {
        let e =
            Config::load(Path::new("/nonexistent/x.toml"), false).unwrap_err();
        assert!(e.to_string().starts_with("/nonexistent/x.toml: "));
    }
}
