// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! The unprivileged process: serves the control socket and runs the
//! periodic task with the configuration the parent sends over.

use std::{
    path::{Path, PathBuf},
    process,
    time::{Duration, Instant},
};

use crate::{
    DAEMON,
    config::Config,
    control::Control,
    daemon::{self, Signal, Signals},
    error::{OrFatal, fatal, strerror},
    imsg::{self, Imsgbuf},
    ipc::{Request, Response, Status},
    log as flog,
    proc::{self, ProcId},
    sandbox,
};

pub struct Opts {
    pub debug: bool,
    pub verbose: u8,
    /// User to drop to, and the directory to chroot into.
    pub privdrop: Option<(String, PathBuf)>,
}

struct State {
    config: Option<Config>,
    /// Configuration being received, applied at `IMSG_RECONF_END`.
    pending: Option<Config>,
    ctl: Option<Control>,
    /// Set when a configuration was applied, cleared by the main loop.
    reconfigured: bool,
    started: Instant,
    reloads: u32,
    quit: bool,
}

impl State {
    fn status(&self, config: &Config) -> Status {
        Status {
            pid: process::id(),
            uptime_secs: self.started.elapsed().as_secs(),
            config: config.clone(),
            verbose: flog::is_verbose(),
            reloads: self.reloads,
        }
    }

    /// Handle a control request; `None` defers the answer to the
    /// parent, which replies with the same peer id.
    fn dispatch(
        &mut self,
        parent: &mut Imsgbuf,
        peerid: u32,
        req: Request,
    ) -> Option<Response> {
        let config = self.config.as_ref()?;
        match req {
            Request::ShowStatus => Some(Response::Status(self.status(config))),
            Request::LogVerbose(on) => {
                flog::set_verbose(on);
                log::info!(
                    "log verbosity {}",
                    if on { "verbose" } else { "brief" }
                );
                parent
                    .compose(imsg::IMSG_CTL_VERBOSE, peerid, None, None, &on)
                    .or_fatal();
                Some(Response::Ok)
            }
            Request::Reload => {
                parent
                    .compose(imsg::IMSG_CTL_RELOAD, peerid, None, None, &())
                    .or_fatal();
                None
            }
            Request::Shutdown => {
                // Answer first: the parent stops this process on receipt.
                parent
                    .compose(imsg::IMSG_CTL_SHUTDOWN, peerid, None, None, &())
                    .or_fatal();
                Some(Response::Ok)
            }
        }
    }

    /// Handle a message from the parent.
    fn parent_msg(&mut self, m: &mut imsg::Imsg) {
        match m.hdr.typ {
            imsg::IMSG_CONTROLFD => {
                let fd = m.take_fd().unwrap_or_else(|| {
                    fatal("control socket descriptor missing")
                });
                if self.ctl.is_some() {
                    fatal("control socket already received");
                }
                self.ctl = Some(Control::listen(fd).or_fatal());
            }
            imsg::IMSG_RECONF_CONF => {
                self.pending = Some(m.get().or_fatal());
            }
            imsg::IMSG_RECONF_END => {
                let Some(cfg) = self.pending.take() else {
                    fatal("configuration end without configuration");
                };
                if self.config.is_some() {
                    self.reloads += 1;
                    log::info!("configuration reloaded");
                } else if self.ctl.is_some() {
                    // Everything the parent hands over has arrived.
                    sandbox::pledge("stdio unix").or_fatal();
                }
                self.config = Some(cfg);
                self.reconfigured = true;
            }
            imsg::IMSG_CTL_OK => {
                self.reply(m.hdr.peerid, &Response::Ok);
            }
            imsg::IMSG_CTL_FAIL => {
                let e: String = m.get().or_fatal();
                self.reply(m.hdr.peerid, &Response::Fail(e));
            }
            t => log::warn!("unexpected imsg {t} from parent"),
        }
    }

    fn reply(&mut self, peerid: u32, resp: &Response) {
        if let Some(ctl) = &mut self.ctl {
            ctl.reply(peerid, resp);
        }
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(self.config.as_ref().map_or(60, |c| c.interval))
    }
}

/// Entry point of the engine process; never returns.
pub fn main(opts: Opts) -> ! {
    flog::procinit(ProcId::Engine.title());
    flog::init(DAEMON, opts.debug, opts.verbose);

    if let Some((user, dir)) = &opts.privdrop {
        let pw = daemon::getpwnam(user)
            .unwrap_or_else(|| fatal(format!("unknown user {user}")));
        daemon::drop_privs(&pw, dir)
            .map_err(|e| format!("can't drop privileges: {}", strerror(&e)))
            .or_fatal();
    }
    // No file system at all; the control socket arrives as a descriptor.
    sandbox::unveil(Path::new("/"), "").or_fatal();
    sandbox::unveil_lock().or_fatal();
    sandbox::pledge("stdio unix recvfd").or_fatal();

    // SAFETY: first use of descriptor 3 in this process.
    let mut parent = unsafe { proc::parent_socket() }.or_fatal();
    parent.allow_fdpass(true);
    daemon::ignore(libc::SIGHUP).or_fatal();
    let signals = Signals::install(&[Signal::Term, Signal::Int]).or_fatal();
    let mut state = State {
        config: None,
        pending: None,
        ctl: None,
        reconfigured: false,
        started: Instant::now(),
        reloads: 0,
        quit: false,
    };
    log::debug!("engine started, pid {}", process::id());

    run(&mut state, &signals, &mut parent);

    log::debug!("engine exiting, pid {}", process::id());
    process::exit(0);
}

fn run(state: &mut State, signals: &Signals, parent: &mut Imsgbuf) {
    let mut next_tick = Instant::now() + state.interval();
    let mut fds = Vec::new();
    while !state.quit {
        fds.clear();
        fds.push(daemon::pollfd(signals.fd(), libc::POLLIN));
        fds.push(daemon::pollfd(parent.fd(), parent.events()));
        let ctl_polled = state.ctl.is_some();
        if let Some(ctl) = &mut state.ctl {
            ctl.fill(&mut fds);
        }

        let now = Instant::now();
        let mut timeout =
            next_tick.saturating_duration_since(now).as_millis() as i32;
        if let Some(b) = state.ctl.as_ref().and_then(Control::backoff_ms) {
            timeout = timeout.min(b);
        }
        if let Err(e) = daemon::poll(&mut fds, timeout) {
            fatal(format!("poll: {}", strerror(&e)));
        }

        if fds[0].revents & libc::POLLIN != 0 {
            for sig in signals.drain() {
                log::debug!("{sig:?} received");
                state.quit = true;
            }
        }

        if fds[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
        {
            match parent.read() {
                Ok(true) => {}
                Ok(false) => {
                    log::debug!("parent pipe closed");
                    state.quit = true;
                    break;
                }
                Err(e) => fatal(format!("parent read: {}", strerror(&e))),
            }
            loop {
                match parent.get() {
                    Ok(Some(mut m)) => state.parent_msg(&mut m),
                    Ok(None) => break,
                    Err(e) => fatal(format!("parent imsg: {e}")),
                }
            }
        }
        if fds[1].revents & libc::POLLOUT != 0
            && let Err(e) = parent.write()
        {
            fatal(format!("parent write: {}", strerror(&e)));
        }

        if state.reconfigured {
            state.reconfigured = false;
            next_tick = Instant::now() + state.interval();
        }

        if ctl_polled && let Some(mut ctl) = state.ctl.take() {
            ctl.handle(&fds[2..], &mut |peerid, req| {
                state.dispatch(parent, peerid, req)
            });
            state.ctl = Some(ctl);
        }
        // Messages queued for the parent go out now rather than after
        // the next poll.
        if let Err(e) = parent.write() {
            fatal(format!("parent write: {}", strerror(&e)));
        }

        if Instant::now() >= next_tick {
            tick(state);
            next_tick += state.interval();
        }
    }
}

/// Example periodic workload; replace with the real one.
fn tick(state: &State) {
    log::debug!("tick, uptime {}s", state.started.elapsed().as_secs());
}
