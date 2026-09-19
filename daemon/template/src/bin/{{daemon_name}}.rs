// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! The privileged parent: reads the configuration, spawns the engine,
//! hands it the control socket and supervises it.

use std::{
    env, fs,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process,
};

use {{crate_name}}::{
    CONF_FILE, DAEMON,
    config::Config,
    control, daemon,
    daemon::{Passwd, Signal, Signals},
    engine,
    error::{OrFatal, errx, fatal, strerror},
    imsg, log as flog,
    proc::{self, Child, ChildOpts, ProcId},
    sandbox,
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
    /// Hidden: run as this child process.
    proc_id: Option<ProcId>,
    /// Hidden: child drops to this user.
    user: Option<String>,
    /// Hidden: child chroots here.
    chroot: Option<PathBuf>,
}

fn parse_args() -> Opts {
    let args: Vec<String> = env::args().collect();
    let mut p = getopt::Parser::new(&args, "df:ns:vP:u:C:");
    let mut o = Opts {
        debug: false,
        check: false,
        verbose: 0,
        conf: None,
        socket: None,
        proc_id: None,
        user: None,
        chroot: None,
    };
    loop {
        match p.next().transpose() {
            Ok(None) => break,
            Ok(Some(getopt::Opt('d', _))) => o.debug = true,
            Ok(Some(getopt::Opt('f', Some(f)))) => o.conf = Some(f.into()),
            Ok(Some(getopt::Opt('n', _))) => o.check = true,
            Ok(Some(getopt::Opt('s', Some(s)))) => o.socket = Some(s.into()),
            Ok(Some(getopt::Opt('v', _))) => o.verbose += 1,
            Ok(Some(getopt::Opt('P', Some(t)))) => {
                o.proc_id =
                    Some(ProcId::from_title(&t).unwrap_or_else(|| usage()));
            }
            Ok(Some(getopt::Opt('u', Some(u)))) => o.user = Some(u),
            Ok(Some(getopt::Opt('C', Some(c)))) => o.chroot = Some(c.into()),
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

/// Parent state.
struct State {
    config: Config,
    conf_path: PathBuf,
    conf_is_default: bool,
    socket_override: Option<PathBuf>,
    engine: Child,
}

impl State {
    fn load(
        conf_path: &Path,
        conf_is_default: bool,
        socket_override: Option<&Path>,
    ) -> anyhow::Result<Config> {
        let mut cfg = Config::load(conf_path, conf_is_default)?;
        if let Some(s) = socket_override {
            cfg.socket = s.to_path_buf();
        }
        Ok(cfg)
    }

    /// Re-read the configuration and push it to the engine; keep the
    /// old one on failure.
    /// `peerid` identifies the control client to answer, if any.
    fn reload(&mut self, peerid: Option<u32>) {
        let r = Self::load(
            &self.conf_path,
            self.conf_is_default,
            self.socket_override.as_deref(),
        );
        match r {
            Ok(cfg) => {
                if cfg.socket != self.config.socket {
                    log::warn!("socket change requires restart");
                }
                if cfg.user != self.config.user
                    || cfg.chroot != self.config.chroot
                {
                    log::warn!("user or chroot change requires restart");
                }
                self.config = cfg;
                self.send_config();
                if let Some(id) = peerid {
                    self.ctl_ok(id);
                }
                log::info!("configuration reloaded");
            }
            Err(e) => {
                log::error!("{e:#}");
                log::error!("configuration reload failed");
                if let Some(id) = peerid {
                    self.compose(imsg::IMSG_CTL_FAIL, id, &e.to_string());
                }
            }
        }
    }

    fn send_config(&mut self) {
        let cfg = self.config.clone();
        self.compose(imsg::IMSG_RECONF_CONF, 0, &cfg);
        self.compose(imsg::IMSG_RECONF_END, 0, &());
    }

    fn ctl_ok(&mut self, peerid: u32) {
        self.compose(imsg::IMSG_CTL_OK, peerid, &());
    }

    fn compose<T: serde::Serialize>(
        &mut self,
        typ: u32,
        peerid: u32,
        data: &T,
    ) {
        self.engine
            .ibuf
            .compose(typ, peerid, None, None, data)
            .or_fatal();
    }

    /// Handle a message from the engine.
    fn engine_msg(&mut self, m: &imsg::Imsg) -> Option<i32> {
        match m.hdr.typ {
            imsg::IMSG_CTL_RELOAD => self.reload(Some(m.hdr.peerid)),
            imsg::IMSG_CTL_VERBOSE => {
                let on: bool = m.get().or_fatal();
                flog::set_verbose(on);
            }
            imsg::IMSG_CTL_SHUTDOWN => {
                log::debug!("shutdown requested");
                return Some(0);
            }
            t => log::warn!("unexpected imsg {t} from engine"),
        }
        None
    }

    /// Stop the engine, wait for it and exit with `code`.
    fn shutdown(mut self, code: i32) -> ! {
        self.engine.terminate();
        match self.engine.wait() {
            Ok(st) if st.success() => log::debug!("engine exited"),
            Ok(st) => match st.signal() {
                Some(sig) => log::warn!("engine terminated; signal {sig}"),
                None => log::warn!("engine exited abnormally"),
            },
            Err(e) => log::warn!("wait: {}", strerror(&e)),
        }
        let _ = fs::remove_file(&self.config.socket);
        log::info!("terminating");
        process::exit(code);
    }
}

fn main() {
    flog::init_stderr(DAEMON, 0);
    let argv0 = env::args().next().unwrap_or_default();
    let opts = parse_args();
    log::set_max_level(flog::level(opts.verbose));

    if let Some(ProcId::Engine) = opts.proc_id {
        let privdrop = match (opts.user, opts.chroot) {
            (Some(u), Some(c)) => Some((u, c)),
            (None, None) => None,
            _ => usage(),
        };
        engine::main(engine::Opts {
            debug: opts.debug,
            verbose: opts.verbose,
            privdrop,
        });
    }
    if opts.proc_id.is_some() || opts.user.is_some() || opts.chroot.is_some() {
        usage();
    }
    flog::procinit(ProcId::Parent.title());
    proc::init_exec_path(&argv0).or_errx();

    let conf_is_default = opts.conf.is_none();
    let conf_path = opts.conf.unwrap_or_else(|| CONF_FILE.into());
    let config =
        State::load(&conf_path, conf_is_default, opts.socket.as_deref())
            .or_errx();

    if opts.check {
        eprintln!("configuration OK");
        process::exit(0);
    }

    control::check(&config.socket).or_errx();

    let privdrop = config.user.as_deref().map(|u| {
        let pw = lookup_user(u);
        let dir = config.chroot.clone().unwrap_or(pw.dir);
        (u.to_string(), dir)
    });

    flog::init(DAEMON, opts.debug, opts.verbose);
    if !opts.debug {
        daemon::daemonize().or_fatal();
    }
    log::info!("startup");

    let signals = Signals::install(&[
        Signal::Hup,
        Signal::Term,
        Signal::Int,
        Signal::Chld,
    ])
    .or_fatal();

    let engine = proc::spawn(
        ProcId::Engine,
        &ChildOpts {
            debug: opts.debug,
            verbose: opts.verbose,
            privdrop,
        },
    )
    .map_err(|e| format!("spawn engine: {}", strerror(&e)))
    .or_fatal();

    let mut state = State {
        config,
        conf_path,
        conf_is_default,
        socket_override: opts.socket,
        engine,
    };

    let ctl_fd = control::open(&state.config.socket).or_fatal();
    state
        .engine
        .ibuf
        .compose(imsg::IMSG_CONTROLFD, 0, None, Some(ctl_fd), &())
        .or_fatal();
    state.send_config();
    // The descriptor must be out before giving up the right to send it.
    state.engine.ibuf.flush().or_fatal();

    // Reads the configuration file, unlinks the socket, signals and
    // waits for the engine.
    if state.conf_path.exists() {
        sandbox::unveil(&state.conf_path, "r").or_fatal();
    }
    sandbox::unveil(&state.config.socket, "c").or_fatal();
    sandbox::unveil_lock().or_fatal();
    sandbox::pledge("stdio rpath cpath proc").or_fatal();

    let code = run(&mut state, &signals);
    state.shutdown(code);
}

fn lookup_user(name: &str) -> Passwd {
    if !daemon::is_root() {
        errx("need root privileges");
    }
    daemon::getpwnam(name)
        .unwrap_or_else(|| errx(format!("unknown user {name}")))
}

/// Main loop; returns the exit code.
fn run(state: &mut State, signals: &Signals) -> i32 {
    let mut fds = Vec::new();
    loop {
        fds.clear();
        fds.push(daemon::pollfd(signals.fd(), libc::POLLIN));
        fds.push(daemon::pollfd(
            state.engine.ibuf.fd(),
            state.engine.ibuf.events(),
        ));
        if let Err(e) = daemon::poll(&mut fds, -1) {
            fatal(format!("poll: {}", strerror(&e)));
        }

        if fds[0].revents & libc::POLLIN != 0 {
            for sig in signals.drain() {
                match sig {
                    Signal::Hup => state.reload(None),
                    Signal::Term | Signal::Int => {
                        log::debug!("{sig:?} received");
                        return 0;
                    }
                    Signal::Chld => {
                        if let Ok(Some(_)) = state.engine.try_wait() {
                            log::warn!("lost child: engine");
                            return 1;
                        }
                    }
                }
            }
        }

        if fds[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
        {
            match state.engine.ibuf.read() {
                Ok(true) => {}
                Ok(false) => {
                    log::warn!("lost child: engine");
                    return 1;
                }
                Err(e) => fatal(format!("engine read: {}", strerror(&e))),
            }
            loop {
                match state.engine.ibuf.get() {
                    Ok(Some(m)) => {
                        if let Some(code) = state.engine_msg(&m) {
                            return code;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => fatal(format!("engine imsg: {e}")),
                }
            }
        }
        if let Err(e) = state.engine.ibuf.write() {
            fatal(format!("engine write: {}", strerror(&e)));
        }
    }
}
