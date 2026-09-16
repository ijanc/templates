// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Process plumbing: detaching, signals, privilege dropping, poll.

use std::{
    ffi::CString,
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    process,
    sync::atomic::{AtomicI32, Ordering},
};

/// Fork, detach from the controlling terminal, chdir to `/` and point
/// the standard descriptors at `/dev/null`.
/// The parent exits 0.
pub fn daemonize() -> io::Result<()> {
    // SAFETY: plain libc calls; the process is single-threaded here.
    unsafe {
        match libc::fork() {
            -1 => return Err(io::Error::last_os_error()),
            0 => {}
            _ => process::exit(0),
        }
        if libc::setsid() == -1 {
            return Err(io::Error::last_os_error());
        }
        if libc::chdir(c"/".as_ptr()) == -1 {
            return Err(io::Error::last_os_error());
        }
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
        if null == -1 {
            return Err(io::Error::last_os_error());
        }
        for fd in 0..3 {
            if libc::dup2(null, fd) == -1 {
                return Err(io::Error::last_os_error());
            }
        }
        if null > 2 {
            libc::close(null);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Signal {
    Hup,
    Term,
    Int,
}

/// Self-pipe: handlers write the signal number, the main loop reads it.
static PIPE_WR: AtomicI32 = AtomicI32::new(-1);

extern "C" fn handler(signo: libc::c_int) {
    let fd = PIPE_WR.load(Ordering::Relaxed);
    if fd != -1 {
        let b = signo as u8;
        // SAFETY: write(2) is async-signal-safe; errors are ignored.
        unsafe {
            libc::write(fd, (&raw const b).cast(), 1);
        }
    }
}

pub struct Signals {
    rd: OwnedFd,
    _wr: OwnedFd,
}

impl Signals {
    /// Install handlers for SIGHUP, SIGTERM and SIGINT; ignore SIGPIPE.
    pub fn install() -> io::Result<Self> {
        let (rd, wr) = pipe()?;
        PIPE_WR.store(wr.as_raw_fd(), Ordering::Relaxed);
        for signo in [libc::SIGHUP, libc::SIGTERM, libc::SIGINT] {
            sigaction(signo, handler as *const () as libc::sighandler_t)?;
        }
        sigaction(libc::SIGPIPE, libc::SIG_IGN)?;
        Ok(Self { rd, _wr: wr })
    }

    pub fn fd(&self) -> RawFd {
        self.rd.as_raw_fd()
    }

    /// Drain pending signals from the pipe.
    pub fn drain(&self) -> Vec<Signal> {
        let mut buf = [0u8; 64];
        let mut out = Vec::new();
        loop {
            // SAFETY: buf is valid for its length.
            let n = unsafe {
                libc::read(self.fd(), buf.as_mut_ptr().cast(), buf.len())
            };
            if n <= 0 {
                break;
            }
            for &b in &buf[..n as usize] {
                match libc::c_int::from(b) {
                    libc::SIGHUP => out.push(Signal::Hup),
                    libc::SIGTERM => out.push(Signal::Term),
                    libc::SIGINT => out.push(Signal::Int),
                    _ => {}
                }
            }
        }
        out
    }
}

fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: fds is a valid two-element array.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both descriptors were just returned by pipe(2).
    let (rd, wr) =
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [&rd, &wr] {
        set_nonblock_cloexec(fd.as_raw_fd())?;
    }
    Ok((rd, wr))
}

pub fn set_nonblock_cloexec(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl on a descriptor we own.
    unsafe {
        let fl = libc::fcntl(fd, libc::F_GETFL);
        if fl == -1
            || libc::fcntl(fd, libc::F_SETFL, fl | libc::O_NONBLOCK) == -1
        {
            return Err(io::Error::last_os_error());
        }
        let fd_fl = libc::fcntl(fd, libc::F_GETFD);
        if fd_fl == -1
            || libc::fcntl(fd, libc::F_SETFD, fd_fl | libc::FD_CLOEXEC) == -1
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn sigaction(signo: libc::c_int, action: libc::sighandler_t) -> io::Result<()> {
    // SAFETY: sigaction is zero-initialised then fully set up.
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = action;
        sa.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&raw mut sa.sa_mask);
        if libc::sigaction(signo, &sa, std::ptr::null_mut()) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct Passwd {
    pub uid: libc::uid_t,
    pub gid: libc::gid_t,
}

/// Look up `name` in the password database.
pub fn getpwnam(name: &str) -> Option<Passwd> {
    let c = CString::new(name).ok()?;
    // SAFETY: getpwnam returns NULL or a pointer to a static struct.
    let pw = unsafe { libc::getpwnam(c.as_ptr()) };
    if pw.is_null() {
        return None;
    }
    // SAFETY: non-null, points to a valid passwd.
    unsafe {
        Some(Passwd {
            uid: (*pw).pw_uid,
            gid: (*pw).pw_gid,
        })
    }
}

pub fn is_root() -> bool {
    // SAFETY: geteuid cannot fail.
    unsafe { libc::geteuid() == 0 }
}

/// Drop to `pw`: supplementary groups, gid, uid.
pub fn drop_privs(pw: Passwd) -> io::Result<()> {
    // SAFETY: plain libc calls with valid arguments.
    unsafe {
        if libc::setgroups(1, &raw const pw.gid) == -1
            || libc::setgid(pw.gid) == -1
            || libc::setuid(pw.uid) == -1
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// `chown(2)` on a path.
pub fn chown(
    path: &std::path::Path,
    uid: libc::uid_t,
    gid: libc::gid_t,
) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    // SAFETY: c is a valid NUL-terminated path.
    if unsafe { libc::chown(c.as_ptr(), uid, gid) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn pollfd(fd: RawFd, events: libc::c_short) -> libc::pollfd {
    libc::pollfd {
        fd,
        events,
        revents: 0,
    }
}

/// `poll(2)` retrying on EINTR.
/// `timeout_ms` of `-1` blocks.
pub fn poll(fds: &mut [libc::pollfd], timeout_ms: i32) -> io::Result<usize> {
    loop {
        // SAFETY: fds is a valid slice of pollfd.
        let n = unsafe {
            libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout_ms)
        };
        if n >= 0 {
            return Ok(n as usize);
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}
