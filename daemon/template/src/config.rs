// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::error::{Result, strerror};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Control socket path.
    pub socket: PathBuf,
    /// User the engine runs as.
    /// Requires root when set.
    pub user: Option<String>,
    /// Directory the engine is confined to with `chroot(2)`.
    /// Defaults to the home directory of `user`.
    pub chroot: Option<PathBuf>,
    /// Seconds between main loop ticks.
    pub interval: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket: crate::SOCKET.into(),
            user: None,
            chroot: None,
            interval: 60,
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
        anyhow::ensure!(self.interval >= 1, "interval must be at least 1");
        anyhow::ensure!(
            self.socket.is_absolute(),
            "socket must be an absolute path"
        );
        if let Some(u) = &self.user {
            anyhow::ensure!(!u.is_empty(), "user must not be empty");
        }
        if let Some(c) = &self.chroot {
            anyhow::ensure!(c.is_absolute(), "chroot must be an absolute path");
        }
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
        assert_eq!(cfg.socket, PathBuf::from(crate::SOCKET));
        assert_eq!(cfg.user, None);
        assert_eq!(cfg.chroot, None);
        assert_eq!(cfg.interval, 60);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn parses_all_keys() {
        let cfg = parse(
            "socket = \"/tmp/x.sock\"\nuser = \"_x\"\nchroot = \"/var/empty\"\ninterval = 5\n",
        )
        .unwrap();
        assert_eq!(cfg.socket, PathBuf::from("/tmp/x.sock"));
        assert_eq!(cfg.user.as_deref(), Some("_x"));
        assert_eq!(cfg.chroot, Some(PathBuf::from("/var/empty")));
        assert_eq!(cfg.interval, 5);
    }

    #[test]
    fn rejects_zero_interval() {
        let e = parse("interval = 0\n").unwrap_err();
        assert!(e.to_string().contains("interval"));
    }

    #[test]
    fn rejects_relative_socket() {
        let e = parse("socket = \"x.sock\"\n").unwrap_err();
        assert!(e.to_string().contains("socket"));
    }

    #[test]
    fn rejects_relative_chroot() {
        let e = parse("chroot = \"empty\"\n").unwrap_err();
        assert!(e.to_string().contains("chroot"));
    }

    #[test]
    fn rejects_empty_user() {
        let e = parse("user = \"\"\n").unwrap_err();
        assert!(e.to_string().contains("user"));
    }

    #[test]
    fn rejects_unknown_key() {
        let e = parse("bogus = 1\n").unwrap_err();
        assert!(e.to_string().contains("bogus"));
    }

    #[test]
    fn missing_default_file_is_empty_config() {
        let cfg = Config::load(Path::new("/nonexistent/x.conf"), true).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn missing_explicit_file_is_error() {
        let e =
            Config::load(Path::new("/nonexistent/x.conf"), false).unwrap_err();
        assert!(e.to_string().starts_with("/nonexistent/x.conf: "));
    }
}
