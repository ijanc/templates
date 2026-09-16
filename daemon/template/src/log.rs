// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Logging to standard error while debugging, to syslog otherwise.

use std::{
    io::Write,
    os::unix::net::UnixDatagram,
    path::Path,
    process,
    sync::{Mutex, OnceLock},
};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Where syslogd may listen, in probe order.
const SYSLOG_PATHS: &[&str] = &["/dev/log", "/var/run/syslog", "/var/run/log"];

/// `LOG_DAEMON` from syslog(3).
const FACILITY: u8 = 3 << 3;

static IDENT: OnceLock<&'static str> = OnceLock::new();

/// Log level for a `-v` count.
pub fn level(verbose: u8) -> LevelFilter {
    match verbose {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    }
}

/// Log to standard error.
/// Called once at startup so errors before option parsing have somewhere
/// to go; [`init`] replaces the sink later.
pub fn init_stderr(ident: &'static str, verbose: u8) {
    let _ = IDENT.set(ident);
    set_sink(Sink::Stderr);
    log::set_max_level(level(verbose));
}

/// Choose the final sink: standard error when `debug`, syslog otherwise.
/// Falls back to standard error when no syslog socket is reachable.
pub fn init(ident: &'static str, debug: bool, verbose: u8) {
    let _ = IDENT.set(ident);
    if !debug && let Some(s) = Syslog::open(ident) {
        set_sink(Sink::Syslog(s));
    } else {
        set_sink(Sink::Stderr);
    }
    log::set_max_level(level(verbose));
}

/// Toggle debug logging at runtime.
pub fn set_verbose(on: bool) {
    log::set_max_level(if on {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
}

pub fn is_verbose() -> bool {
    log::max_level() > LevelFilter::Info
}

enum Sink {
    Stderr,
    Syslog(Syslog),
}

static SINK: Mutex<Sink> = Mutex::new(Sink::Stderr);

fn set_sink(sink: Sink) {
    *SINK.lock().unwrap_or_else(|e| e.into_inner()) = sink;
    // The global logger can only be installed once; [`init_stderr`] may
    // already have done it.
    let _ = log::set_boxed_logger(Box::new(Dispatch));
}

/// Routes records to the current [`Sink`].
struct Dispatch;

impl Log for Dispatch {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= log::max_level()
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let mut sink = SINK.lock().unwrap_or_else(|e| e.into_inner());
        match &mut *sink {
            Sink::Stderr => Stderr.log(r),
            Sink::Syslog(s) => s.log(r),
        }
    }

    fn flush(&self) {}
}

/// OpenBSD `log.c` style: warnings and errors carry the program name,
/// everything else is bare.
struct Stderr;

impl Log for Stderr {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= log::max_level()
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = match r.level() {
            Level::Error | Level::Warn => {
                let ident = IDENT.get().copied().unwrap_or("");
                writeln!(err, "{ident}: {}", r.args())
            }
            _ => writeln!(err, "{}", r.args()),
        };
    }

    fn flush(&self) {}
}

/// RFC 3164 datagrams to the local syslogd.
struct Syslog {
    ident: &'static str,
    sock: UnixDatagram,
    path: &'static str,
}

impl Syslog {
    fn open(ident: &'static str) -> Option<Self> {
        for path in SYSLOG_PATHS {
            if !Path::new(path).exists() {
                continue;
            }
            if let Some(sock) = Self::connect(path) {
                return Some(Self { ident, sock, path });
            }
        }
        None
    }

    fn connect(path: &str) -> Option<UnixDatagram> {
        let sock = UnixDatagram::unbound().ok()?;
        sock.connect(path).ok()?;
        Some(sock)
    }

    fn log(&mut self, r: &Record) {
        let severity = match r.level() {
            Level::Error => 3,
            Level::Warn => 4,
            Level::Info => 6,
            Level::Debug | Level::Trace => 7,
        };
        let msg = format!(
            "<{}>{}[{}]: {}",
            FACILITY | severity,
            self.ident,
            process::id(),
            r.args()
        );
        if self.sock.send(msg.as_bytes()).is_err() {
            // syslogd may have restarted; reconnect once.
            if let Some(sock) = Self::connect(self.path) {
                self.sock = sock;
                let _ = self.sock.send(msg.as_bytes());
            }
        }
    }
}
