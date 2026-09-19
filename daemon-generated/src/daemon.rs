// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Process plumbing: detaching, signals, privilege dropping, poll.

use std::{
    ffi::CString,
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::ffi::OsStrExt,
    },
    path::{Path, PathBuf},
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
    Chld,
}

impl Signal {
    fn signo(self) -> libc::c_int {
        match self {
            Self::Hup => libc::SIGHUP,
            Self::Term => libc::SIGTERM,
            Self::Int => libc::SIGINT,
            Self::Chld => libc::SIGCHLD,
        }
    }

    fn from_signo(signo: libc::c_int) -> Option<Self> {
        match signo {
            libc::SIGHUP => Some(Self::Hup),
            libc::SIGTERM => Some(Self::Term),
            libc::SIGINT => Some(Self::Int),
            libc::SIGCHLD => Some(Self::Chld),
            _ => None,
        }
    }
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
    /// Route `handled` through the pipe; ignore SIGPIPE.
    pub fn install(handled: &[Signal]) -> io::Result<Self> {
        let (rd, wr) = pipe()?;
        PIPE_WR.store(wr.as_raw_fd(), Ordering::Relaxed);
        for sig in handled {
            sigaction(sig.signo(), handler as *const () as libc::sighandler_t)?;
        }
        ignore(libc::SIGPIPE)?;
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
                if let Some(sig) = Signal::from_signo(libc::c_int::from(b)) {
                    out.push(sig);
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
    }
    set_cloexec(fd)
}

pub fn set_cloexec(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl on a descriptor we own.
    unsafe {
        let fl = libc::fcntl(fd, libc::F_GETFD);
        if fl == -1
            || libc::fcntl(fd, libc::F_SETFD, fl | libc::FD_CLOEXEC) == -1
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Clear close-on-exec so `fd` survives `execve(2)`.
pub fn clear_cloexec(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl on a descriptor we own.
    unsafe {
        let fl = libc::fcntl(fd, libc::F_GETFD);
        if fl == -1
            || libc::fcntl(fd, libc::F_SETFD, fl & !libc::FD_CLOEXEC) == -1
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Ignore a signal.
pub fn ignore(signo: libc::c_int) -> io::Result<()> {
    sigaction(signo, libc::SIG_IGN)
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

#[derive(Clone, Debug, PartialEq)]
pub struct Passwd {
    pub uid: libc::uid_t,
    pub gid: libc::gid_t,
    /// Home directory, the default chroot.
    pub dir: PathBuf,
}

/// Look up `name` in the password database.
pub fn getpwnam(name: &str) -> Option<Passwd> {
    let c = CString::new(name).ok()?;
    // SAFETY: getpwnam returns NULL or a pointer to a static struct.
    let pw = unsafe { libc::getpwnam(c.as_ptr()) };
    if pw.is_null() {
        return None;
    }
    // SAFETY: non-null, points to a valid passwd whose pw_dir is a
    // NUL-terminated string.
    unsafe {
        let dir = std::ffi::CStr::from_ptr((*pw).pw_dir);
        Some(Passwd {
            uid: (*pw).pw_uid,
            gid: (*pw).pw_gid,
            dir: PathBuf::from(std::ffi::OsStr::from_bytes(dir.to_bytes())),
        })
    }
}

/// Raise the soft open file limit to the hard one; children inherit it.
pub fn raise_nofile() -> io::Result<()> {
    let mut rl = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: rl is a valid out pointer, then a valid in pointer.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut rl) == -1 {
            return Err(io::Error::last_os_error());
        }
        if rl.rlim_cur == rl.rlim_max {
            return Ok(());
        }
        rl.rlim_cur = rl.rlim_max;
        if libc::setrlimit(libc::RLIMIT_NOFILE, &rl) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub fn is_root() -> bool {
    // SAFETY: geteuid cannot fail.
    unsafe { libc::geteuid() == 0 }
}

/// Confine the process to `dir` and drop to `pw`: chroot, chdir to `/`,
/// supplementary groups, gid, uid.
/// `dir` must be owned by root and not writable by group or others.
pub fn drop_privs(pw: &Passwd, dir: &Path) -> io::Result<()> {
    let c = cpath(dir)?;
    // SAFETY: plain libc calls with valid arguments; st is only read
    // after stat succeeds.
    unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::stat(c.as_ptr(), &raw mut st) == -1 {
            return Err(io::Error::last_os_error());
        }
        if st.st_uid != 0 || st.st_mode & (libc::S_IWGRP | libc::S_IWOTH) != 0 {
            return Err(io::Error::other(format!(
                "bad privsep dir permissions: {}",
                dir.display()
            )));
        }
        if libc::chroot(c.as_ptr()) == -1 || libc::chdir(c"/".as_ptr()) == -1 {
            return Err(io::Error::last_os_error());
        }
        if libc::setgroups(1, &raw const pw.gid) == -1
            || setresgid(pw.gid) == -1
            || setresuid(pw.uid) == -1
        {
            return Err(io::Error::last_os_error());
        }
        if libc::setuid(0) != -1 {
            return Err(io::Error::other("able to regain privileges"));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
unsafe fn setresgid(gid: libc::gid_t) -> libc::c_int {
    // SAFETY: plain libc call.
    unsafe { libc::setresgid(gid, gid, gid) }
}

#[cfg(not(target_os = "macos"))]
unsafe fn setresuid(uid: libc::uid_t) -> libc::c_int {
    // SAFETY: plain libc call.
    unsafe { libc::setresuid(uid, uid, uid) }
}

/// macOS lacks setres*; setgid/setuid set all three ids when root.
#[cfg(target_os = "macos")]
unsafe fn setresgid(gid: libc::gid_t) -> libc::c_int {
    // SAFETY: plain libc call.
    unsafe { libc::setgid(gid) }
}

#[cfg(target_os = "macos")]
unsafe fn setresuid(uid: libc::uid_t) -> libc::c_int {
    // SAFETY: plain libc call.
    unsafe { libc::setuid(uid) }
}

/// Effective uid of the peer on a connected Unix socket.
pub fn peer_euid(fd: RawFd) -> io::Result<libc::uid_t> {
    #[cfg(target_os = "linux")]
    // SAFETY: getsockopt fills a ucred of the length passed.
    unsafe {
        let mut cred: libc::ucred = std::mem::zeroed();
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        if libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &raw mut len,
        ) == -1
        {
            return Err(io::Error::last_os_error());
        }
        Ok(cred.uid)
    }
    #[cfg(not(target_os = "linux"))]
    // SAFETY: getpeereid writes both ids on success.
    unsafe {
        let (mut uid, mut gid) = (0 as libc::uid_t, 0 as libc::gid_t);
        if libc::getpeereid(fd, &raw mut uid, &raw mut gid) == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(uid)
    }
}

fn cpath(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
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
