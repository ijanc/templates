// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Spawn the daemon on a temporary socket and drive it with the control
//! program.

use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicU32, Ordering},
    thread,
    time::{Duration, Instant},
};

use daemon_generated::{CTL, DAEMON};

const DAEMON_EXE: &str = env!("CARGO_BIN_EXE_daemon-generatedd");
const CTL_EXE: &str = env!("CARGO_BIN_EXE_daemon-generatedctl");

struct Daemon {
    child: Child,
    dir: PathBuf,
    sock: PathBuf,
    conf: PathBuf,
}

impl Daemon {
    fn spawn() -> Self {
        static SEQ: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "{DAEMON}-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("ctl.sock");
        let conf = dir.join("ctl.conf");
        fs::write(&conf, "interval = 1\n").unwrap();
        let child = Command::new(DAEMON_EXE)
            .args(["-dv", "-s"])
            .arg(&sock)
            .arg("-f")
            .arg(&conf)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !sock.exists() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "socket never appeared"
            );
            thread::sleep(Duration::from_millis(20));
        }
        Self {
            child,
            dir,
            sock,
            conf,
        }
    }

    fn ctl(&self, args: &[&str]) -> Output {
        Command::new(CTL_EXE)
            .arg("-s")
            .arg(&self.sock)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn lifecycle() {
    let mut d = Daemon::spawn();

    let o = d.ctl(&["show", "status"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(
        out.contains(&format!("pid:       {}", d.child.id())),
        "{out}"
    );
    assert!(out.contains("verbose:   yes"), "{out}");
    assert!(out.contains("reloads:   0"), "{out}");
    assert!(out.contains("interval:  1s"), "{out}");

    let o = d.ctl(&["log", "brief"]);
    assert!(o.status.success());
    assert_eq!(stdout(&o), "command succeeded\n");
    let out = stdout(&d.ctl(&["sh", "st"]));
    assert!(out.contains("verbose:   no"), "{out}");

    let o = d.ctl(&["log", "verbose"]);
    assert!(o.status.success());
    let out = stdout(&d.ctl(&["show", "status"]));
    assert!(out.contains("verbose:   yes"), "{out}");

    fs::write(&d.conf, "interval = 7\n").unwrap();
    let o = d.ctl(&["reload"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&d.ctl(&["show", "status"]));
    assert!(out.contains("reloads:   1"), "{out}");
    assert!(out.contains("interval:  7s"), "{out}");

    fs::write(&d.conf, "interval = 0\n").unwrap();
    let o = d.ctl(&["reload"]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("interval"), "{}", stderr(&o));
    let out = stdout(&d.ctl(&["show", "status"]));
    assert!(out.contains("interval:  7s"), "{out}");

    let o = d.ctl(&["stop"]);
    assert!(o.status.success());
    let status = d.child.wait().unwrap();
    assert!(status.success(), "{status}");
    assert!(!d.sock.exists());
}

#[test]
fn already_running() {
    let d = Daemon::spawn();
    let o = Command::new(DAEMON_EXE)
        .args(["-d", "-s"])
        .arg(&d.sock)
        .arg("-f")
        .arg(&d.conf)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(stderr(&o), format!("{DAEMON}: already running\n"));
}

#[test]
fn ctl_argument_errors() {
    let d = Daemon::spawn();
    let o = d.ctl(&["s"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).starts_with(&format!("{CTL}: ambiguous argument: s\n")));

    let o = d.ctl(&["log"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).starts_with(&format!("{CTL}: missing argument\n")));

    let o = d.ctl(&["bogus"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).starts_with(&format!("{CTL}: unknown argument: bogus\n"))
    );

    let o = d.ctl(&[]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        stderr(&o),
        format!("usage: {CTL} [-s socket] command [argument ...]\n")
    );
}

#[test]
fn ctl_without_daemon() {
    let sock = std::env::temp_dir().join(format!("{DAEMON}-none.sock"));
    let o = Command::new(CTL_EXE)
        .arg("-s")
        .arg(&sock)
        .args(["show", "status"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).starts_with(&format!("{CTL}: connect: ")),
        "{}",
        stderr(&o)
    );
}

#[test]
fn config_check() {
    let o = Command::new(DAEMON_EXE)
        .args(["-n", "-f", "/nonexistent/ctl.conf"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        stderr(&o),
        format!("{DAEMON}: /nonexistent/ctl.conf: No such file or directory\n")
    );

    let o = Command::new(DAEMON_EXE).args(["extra"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        stderr(&o),
        format!("usage: {DAEMON} [-dnv] [-f file] [-s socket]\n")
    );
}
