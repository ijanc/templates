// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Control socket: bound by the parent, served by the engine.

use std::{
    fs, io,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Context;

use crate::{
    daemon::{self, pollfd, set_nonblock_cloexec},
    error::{Result, strerror},
    imsg::Imsgbuf,
    ipc::{Request, Response},
};

const BACKLOG: libc::c_int = 5;
/// Pause accepting after EMFILE/ENFILE for this long.
const ACCEPT_BACKOFF: Duration = Duration::from_secs(1);

/// Fail when another instance answers on `path`.
pub fn check(path: &Path) -> Result<()> {
    match UnixStream::connect(path) {
        Ok(_) => anyhow::bail!("already running"),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("connect: {}: {}", path.display(), strerror(&e))
        }
    }
}

/// Remove a stale socket and bind `path` with mode 0660.
/// The socket is owned by the caller, normally root; the engine calls
/// [`Control::listen`] on it.
pub fn open(path: &Path) -> Result<OwnedFd> {
    check(path)?;
    if let Err(e) = fs::remove_file(path)
        && e.kind() != io::ErrorKind::NotFound
    {
        anyhow::bail!("unlink: {}: {}", path.display(), strerror(&e));
    }
    // SAFETY: umask cannot fail.
    let old = unsafe { libc::umask(0o117) };
    let bound = UnixListener::bind(path);
    // SAFETY: as above.
    unsafe { libc::umask(old) };
    let listener = bound.map_err(|e| {
        anyhow::anyhow!("bind: {}: {}", path.display(), strerror(&e))
    })?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o660)).map_err(
        |e| anyhow::anyhow!("chmod: {}: {}", path.display(), strerror(&e)),
    )?;
    Ok(listener.into())
}

pub struct Control {
    listener: UnixListener,
    conns: Vec<Conn>,
    next_peerid: u32,
    accept_paused_until: Option<Instant>,
}

struct Conn {
    ibuf: Imsgbuf,
    peerid: u32,
    euid: libc::uid_t,
    closed: bool,
}

impl Control {
    /// Start accepting on a socket from [`open`].
    pub fn listen(fd: OwnedFd) -> Result<Self> {
        // SAFETY: listen on a bound socket.
        if unsafe { libc::listen(fd.as_raw_fd(), BACKLOG) } == -1 {
            let e = io::Error::last_os_error();
            anyhow::bail!("listen: {}", strerror(&e));
        }
        set_nonblock_cloexec(fd.as_raw_fd()).context("fcntl")?;
        Ok(Self {
            listener: fd.into(),
            conns: Vec::new(),
            next_peerid: 1,
            accept_paused_until: None,
        })
    }

    /// Append the listener and every client to `fds`.
    /// Number of entries appended equals `1 + clients`.
    pub fn fill(&mut self, fds: &mut Vec<libc::pollfd>) {
        let paused =
            self.accept_paused_until.is_some_and(|t| Instant::now() < t);
        if !paused {
            self.accept_paused_until = None;
        }
        fds.push(pollfd(
            self.listener.as_raw_fd(),
            if paused { 0 } else { libc::POLLIN },
        ));
        for c in &self.conns {
            fds.push(pollfd(c.ibuf.fd(), c.ibuf.events()));
        }
    }

    /// Milliseconds until accepting resumes, if paused.
    pub fn backoff_ms(&self) -> Option<i32> {
        self.accept_paused_until.map(|t| {
            t.saturating_duration_since(Instant::now()).as_millis() as i32
        })
    }

    /// Process readiness for the entries [`fill`](Self::fill) appended,
    /// calling `dispatch` with the client's peer id for every request.
    /// A `None` from `dispatch` means the answer comes later through
    /// [`reply`](Self::reply).
    pub fn handle(
        &mut self,
        fds: &[libc::pollfd],
        dispatch: &mut dyn FnMut(u32, Request) -> Option<Response>,
    ) {
        if fds[0].revents & libc::POLLIN != 0 {
            self.accept();
        }
        for (c, fd) in self.conns.iter_mut().zip(&fds[1..]) {
            if fd.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
            {
                c.read(dispatch);
            }
            if !c.closed && fd.revents & libc::POLLOUT != 0 {
                c.write();
            }
        }
        let before = self.conns.len();
        self.conns.retain(|c| !c.closed);
        if self.conns.len() < before {
            // A descriptor was freed; try accepting again.
            self.accept_paused_until = None;
        }
    }

    /// Answer a request deferred by `dispatch`.
    /// A client that went away is silently skipped.
    pub fn reply(&mut self, peerid: u32, resp: &Response) {
        if let Some(c) = self.conns.iter_mut().find(|c| c.peerid == peerid) {
            c.send(resp);
        }
    }

    fn accept(&mut self) {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Err(e) = set_nonblock_cloexec(stream.as_raw_fd()) {
                        log::warn!("fcntl: {}", strerror(&e));
                        continue;
                    }
                    let euid = match daemon::peer_euid(stream.as_raw_fd()) {
                        Ok(uid) => uid,
                        Err(e) => {
                            log::warn!("getpeereid: {}", strerror(&e));
                            continue;
                        }
                    };
                    let peerid = self.next_peerid;
                    self.next_peerid = self.next_peerid.wrapping_add(1).max(1);
                    log::trace!("control connection {peerid} accepted");
                    self.conns.push(Conn {
                        ibuf: Imsgbuf::new(stream.into()),
                        peerid,
                        euid,
                        closed: false,
                    });
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::ConnectionAborted
                    ) =>
                {
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e)
                    if matches!(
                        e.raw_os_error(),
                        Some(libc::EMFILE | libc::ENFILE)
                    ) =>
                {
                    log::warn!("accept: {}", strerror(&e));
                    self.accept_paused_until =
                        Some(Instant::now() + ACCEPT_BACKOFF);
                    break;
                }
                Err(e) => {
                    log::warn!("accept: {}", strerror(&e));
                    break;
                }
            }
        }
    }
}

impl Conn {
    /// Root and the daemon's own user may issue privileged requests.
    fn allowed(&self, req: &Request) -> bool {
        // SAFETY: geteuid cannot fail.
        !req.privileged()
            || self.euid == 0
            || self.euid == unsafe { libc::geteuid() }
    }

    fn read(
        &mut self,
        dispatch: &mut dyn FnMut(u32, Request) -> Option<Response>,
    ) {
        match self.ibuf.read() {
            Ok(true) => {}
            Ok(false) => {
                log::trace!("control connection {} closed", self.peerid);
                self.closed = true;
                return;
            }
            Err(e) => {
                log::debug!("control read: {}", strerror(&e));
                self.closed = true;
                return;
            }
        }
        loop {
            let m = match self.ibuf.get() {
                Ok(Some(m)) => m,
                Ok(None) => break,
                Err(e) => {
                    log::debug!("control bad message: {e}");
                    self.closed = true;
                    return;
                }
            };
            let req = match Request::from_imsg(&m) {
                Ok(r) => r,
                Err(e) => {
                    log::debug!("control bad request: {e}");
                    self.closed = true;
                    return;
                }
            };
            log::debug!("control request: {req:?}");
            if !self.allowed(&req) {
                log::warn!(
                    "control request {req:?} from uid {} denied",
                    self.euid
                );
                self.send(&Response::Fail("permission denied".into()));
                continue;
            }
            if let Some(resp) = dispatch(self.peerid, req) {
                self.send(&resp);
            }
        }
        if self.ibuf.queuelen() > 0 {
            self.write();
        }
    }

    fn send(&mut self, resp: &Response) {
        let r = resp.parts().and_then(|(typ, body)| {
            self.ibuf.compose_raw(typ, self.peerid, None, None, &body)
        });
        if let Err(e) = r {
            log::warn!("control encode: {e}");
            self.closed = true;
        }
    }

    fn write(&mut self) {
        if let Err(e) = self.ibuf.write() {
            log::debug!("control write: {}", strerror(&e));
            self.closed = true;
        }
    }
}
