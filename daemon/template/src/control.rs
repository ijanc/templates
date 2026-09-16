// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Control socket: a non-blocking Unix listener and its clients.

use std::{
    fs, io,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Context;

use crate::{
    daemon::{self, pollfd, set_nonblock_cloexec},
    error::{Result, strerror},
    ipc::{self, Request, Response},
};

const BACKLOG: libc::c_int = 5;
/// Pause accepting after EMFILE/ENFILE for this long.
const ACCEPT_BACKOFF: Duration = Duration::from_secs(1);
const READ_CHUNK: usize = 4096;

pub struct Control {
    listener: UnixListener,
    path: PathBuf,
    conns: Vec<Conn>,
    accept_paused_until: Option<Instant>,
}

struct Conn {
    stream: UnixStream,
    rbuf: Vec<u8>,
    wbuf: Vec<u8>,
    closed: bool,
}

impl Control {
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

    /// Remove a stale socket and listen on `path` with mode 0660.
    pub fn init(path: &Path) -> Result<Self> {
        Self::check(path)?;
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
        // SAFETY: listen on a bound socket.
        if unsafe { libc::listen(listener.as_raw_fd(), BACKLOG) } == -1 {
            let e = io::Error::last_os_error();
            anyhow::bail!("listen: {}: {}", path.display(), strerror(&e));
        }
        set_nonblock_cloexec(listener.as_raw_fd()).context("fcntl")?;
        Ok(Self {
            listener,
            path: path.to_path_buf(),
            conns: Vec::new(),
            accept_paused_until: None,
        })
    }

    /// Hand the socket to the unprivileged user before dropping privileges.
    pub fn chown(&self, pw: daemon::Passwd) -> Result<()> {
        daemon::chown(&self.path, pw.uid, pw.gid).map_err(|e| {
            anyhow::anyhow!("chown: {}: {}", self.path.display(), strerror(&e))
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
            let mut ev = libc::POLLIN;
            if !c.wbuf.is_empty() {
                ev |= libc::POLLOUT;
            }
            fds.push(pollfd(c.stream.as_raw_fd(), ev));
        }
    }

    /// Milliseconds until accepting resumes, if paused.
    pub fn backoff_ms(&self) -> Option<i32> {
        self.accept_paused_until.map(|t| {
            t.saturating_duration_since(Instant::now()).as_millis() as i32
        })
    }

    /// Process readiness for the entries [`fill`](Self::fill) appended,
    /// calling `dispatch` for every complete request.
    pub fn handle(
        &mut self,
        fds: &[libc::pollfd],
        dispatch: &mut dyn FnMut(Request) -> Response,
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
        self.conns.retain(|c| !c.closed);
    }

    fn accept(&mut self) {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Err(e) = set_nonblock_cloexec(stream.as_raw_fd()) {
                        log::warn!("fcntl: {}", strerror(&e));
                        continue;
                    }
                    log::trace!("control connection accepted");
                    self.conns.push(Conn {
                        stream,
                        rbuf: Vec::new(),
                        wbuf: Vec::new(),
                        closed: false,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
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
    fn read(&mut self, dispatch: &mut dyn FnMut(Request) -> Response) {
        let mut chunk = [0u8; READ_CHUNK];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    log::trace!("control connection closed");
                    self.closed = true;
                    return;
                }
                Ok(n) => self.rbuf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    log::debug!("control read: {}", strerror(&e));
                    self.closed = true;
                    return;
                }
            }
        }
        loop {
            match ipc::decode::<Request>(&self.rbuf) {
                Ok(Some((req, used))) => {
                    self.rbuf.drain(..used);
                    log::debug!("control request: {req:?}");
                    let resp = dispatch(req);
                    match ipc::encode(&resp) {
                        Ok(frame) => self.wbuf.extend_from_slice(&frame),
                        Err(e) => {
                            log::warn!("control encode: {e}");
                            self.closed = true;
                            return;
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    log::debug!("control bad frame: {e}");
                    self.closed = true;
                    return;
                }
            }
        }
        if !self.wbuf.is_empty() {
            self.write();
        }
    }

    fn write(&mut self) {
        while !self.wbuf.is_empty() {
            match self.stream.write(&self.wbuf) {
                Ok(0) => {
                    self.closed = true;
                    return;
                }
                Ok(n) => {
                    self.wbuf.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    log::debug!("control write: {}", strerror(&e));
                    self.closed = true;
                    return;
                }
            }
        }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
