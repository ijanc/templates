// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::{
    env,
    path::PathBuf,
    process,
    time::{Duration, Instant},
};

use daemon_generated::{
    CONF_FILE, DAEMON,
    config::Config,
    control::Control,
    daemon::{self, Passwd, Signal, Signals},
    error::{OrFatal, fatal, strerror},
    ipc::{Request, Response, Status},
    log as flog,
};

fn usage() -> ! {
    eprintln!("usage: {DAEMON} [-dnv] [-f file] [-s socket]");
    process::exit(1);
}

struct Opts {
    debug: bool,
    check: bool,
    verbose: u8,
    conf: Option<PathBuf>,
    socket: Option<PathBuf>,
}

fn parse_args() -> Opts {
    let args: Vec<String> = env::args().collect();
    let mut p = getopt::Parser::new(&args, "df:ns:v");
    let mut o = Opts {
        debug: false,
        check: false,
        verbose: 0,
        conf: None,
        socket: None,
    };
    loop {
        match p.next().transpose() {
            Ok(None) => break,
            Ok(Some(getopt::Opt('d', _))) => o.debug = true,
            Ok(Some(getopt::Opt('f', Some(f)))) => o.conf = Some(f.into()),
            Ok(Some(getopt::Opt('n', _))) => o.check = true,
            Ok(Some(getopt::Opt('s', Some(s)))) => o.socket = Some(s.into()),
            Ok(Some(getopt::Opt('v', _))) => o.verbose += 1,
            Ok(Some(_)) => unreachable!(),
            Err(e) => {
                eprintln!("{DAEMON}: {e}");
                usage();
            }
        }
    }
    if p.index() < args.len() {
        usage();
    }
    o
}

/// Daemon state shared with the control dispatcher.
struct State {
    config: Config,
    conf_path: PathBuf,
    conf_is_default: bool,
    socket_override: Option<PathBuf>,
    started: Instant,
    reloads: u32,
    quit: bool,
}

impl State {
    fn load(&self) -> anyhow::Result<Config> {
        let mut cfg = Config::load(&self.conf_path, self.conf_is_default)?;
        if let Some(s) = &self.socket_override {
            cfg.socket = s.clone();
        }
        Ok(cfg)
    }

    /// Re-read the configuration; keep the old one on failure.
    fn reload(&mut self) -> Result<(), String> {
        match self.load() {
            Ok(cfg) => {
                if cfg.socket != self.config.socket {
                    log::warn!("socket change requires restart");
                }
                if cfg.user != self.config.user {
                    log::warn!("user change requires restart");
                }
                self.config = cfg;
                self.reloads += 1;
                log::info!("configuration reloaded");
                Ok(())
            }
            Err(e) => {
                log::error!("{e:#}");
                log::error!("configuration reload failed");
                Err(e.to_string())
            }
        }
    }

    fn status(&self) -> Status {
        Status {
            pid: process::id(),
            uptime_secs: self.started.elapsed().as_secs(),
            config: self.config.clone(),
            verbose: flog::is_verbose(),
            reloads: self.reloads,
        }
    }

    fn dispatch(&mut self, req: Request) -> Response {
        match req {
            Request::ShowStatus => Response::Status(self.status()),
            Request::LogVerbose(on) => {
                flog::set_verbose(on);
                log::info!(
                    "log verbosity {}",
                    if on { "verbose" } else { "brief" }
                );
                Response::Ok
            }
            Request::Reload => match self.reload() {
                Ok(()) => Response::Ok,
                Err(e) => Response::Fail(e),
            },
            Request::Shutdown => {
                self.quit = true;
                Response::Ok
            }
        }
    }
}

fn main() {
    flog::init_stderr(DAEMON, 0);
    let opts = parse_args();
    log::set_max_level(flog::level(opts.verbose));

    let conf_is_default = opts.conf.is_none();
    let conf_path = opts.conf.unwrap_or_else(|| CONF_FILE.into());
    let mut state = State {
        config: Config::default(),
        conf_path,
        conf_is_default,
        socket_override: opts.socket,
        started: Instant::now(),
        reloads: 0,
        quit: false,
    };
    state.config = state.load().or_fatal();

    if opts.check {
        eprintln!("configuration OK");
        process::exit(0);
    }

    Control::check(&state.config.socket).or_fatal();

    let pw = state.config.user.as_deref().map(lookup_user);

    flog::init(DAEMON, opts.debug, opts.verbose);
    if !opts.debug {
        daemon::daemonize().or_fatal();
    }
    log::info!("startup");

    let signals = Signals::install().or_fatal();
    let mut ctl = Control::init(&state.config.socket).or_fatal();

    if let Some(pw) = pw {
        ctl.chown(pw).or_fatal();
        daemon::drop_privs(pw)
            .map_err(|e| format!("can't drop privileges: {}", strerror(&e)))
            .or_fatal();
    }

    run(&mut state, &signals, &mut ctl);

    drop(ctl);
    log::info!("terminating");
    process::exit(0);
}

fn lookup_user(name: &str) -> Passwd {
    if !daemon::is_root() {
        fatal("need root privileges");
    }
    daemon::getpwnam(name)
        .unwrap_or_else(|| fatal(format!("unknown user {name}")))
}

fn run(state: &mut State, signals: &Signals, ctl: &mut Control) {
    let mut next_tick = Instant::now() + interval(state);
    let mut fds = Vec::new();
    while !state.quit {
        fds.clear();
        fds.push(daemon::pollfd(signals.fd(), libc::POLLIN));
        ctl.fill(&mut fds);

        let now = Instant::now();
        let mut timeout =
            next_tick.saturating_duration_since(now).as_millis() as i32;
        if let Some(b) = ctl.backoff_ms() {
            timeout = timeout.min(b);
        }
        if let Err(e) = daemon::poll(&mut fds, timeout) {
            fatal(format!("poll: {}", strerror(&e)));
        }

        if fds[0].revents & libc::POLLIN != 0 {
            for sig in signals.drain() {
                match sig {
                    Signal::Hup => {
                        let _ = state.reload();
                        next_tick = Instant::now() + interval(state);
                    }
                    Signal::Term | Signal::Int => {
                        log::debug!("{sig:?} received");
                        state.quit = true;
                    }
                }
            }
        }

        ctl.handle(&fds[1..], &mut |req| state.dispatch(req));

        if Instant::now() >= next_tick {
            tick(state);
            next_tick += interval(state);
        }
    }
}

fn interval(state: &State) -> Duration {
    Duration::from_secs(state.config.interval)
}

/// Example periodic workload; replace with the real one.
fn tick(state: &State) {
    log::debug!("tick, uptime {}s", state.started.elapsed().as_secs());
}
