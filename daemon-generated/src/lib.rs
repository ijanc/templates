// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

pub mod config;
pub mod control;
pub mod daemon;
pub mod engine;
pub mod error;
pub mod imsg;
pub mod ipc;
pub mod log;
pub mod proc;

/// Daemon program name, used for log identity and diagnostics.
pub const DAEMON: &str = "daemon-generatedd";
/// Control program name.
pub const CTL: &str = "daemon-generatedctl";

pub const CONF_FILE: &str = "/etc/daemon-generatedd.conf";
pub const SOCKET: &str = "/var/run/daemon-generatedd.sock";
