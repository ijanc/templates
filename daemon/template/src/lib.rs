// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

pub mod config;
pub mod control;
pub mod daemon;
pub mod error;
pub mod ipc;
pub mod log;

/// Daemon program name, used for log identity and diagnostics.
pub const DAEMON: &str = "{{daemon_name}}";
/// Control program name.
pub const CTL: &str = "{{ctl_name}}";

pub const CONF_FILE: &str = "/etc/{{daemon_name}}.conf";
pub const SOCKET: &str = "/var/run/{{daemon_name}}.sock";
