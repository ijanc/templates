// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Inter-process messages over a Unix stream socket.
//!
//! Every message starts with a fixed 16-byte little-endian header
//! ([`Hdr`]): type, total length, peer id and sender pid.
//! The payload, when present, is postcard-encoded.
//! At most one file descriptor rides along per message, sent as
//! `SCM_RIGHTS` ancillary data with the first byte of the message.

use std::{
    collections::VecDeque,
    io,
    mem::size_of,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    process,
};

use serde::{Serialize, de::DeserializeOwned};

use crate::daemon::set_cloexec;

/// Largest message in bytes, header included.
pub const MAX_IMSGSIZE: usize = 16384;
/// Header size in bytes.
pub const HEADER: usize = 16;
/// Bytes read from the socket per call.
const READ_SIZE: usize = 65535;
/// Set in the on-wire length when a descriptor accompanies the message.
const FD_MARK: u32 = 0x8000_0000;
/// Room for one `SCM_RIGHTS` descriptor on every supported platform.
const CMSG_BUF: usize = 64;

// Message types.
//
// Control program to engine and back.
pub const IMSG_CTL_OK: u32 = 1;
pub const IMSG_CTL_FAIL: u32 = 2;
pub const IMSG_CTL_SHOW_STATUS: u32 = 3;
pub const IMSG_CTL_STATUS: u32 = 4;
pub const IMSG_CTL_VERBOSE: u32 = 5;
pub const IMSG_CTL_RELOAD: u32 = 6;
pub const IMSG_CTL_SHUTDOWN: u32 = 7;
// Parent to engine.
pub const IMSG_CONTROLFD: u32 = 100;
pub const IMSG_RECONF_CONF: u32 = 101;
pub const IMSG_RECONF_END: u32 = 102;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hdr {
    pub typ: u32,
    /// Total length, header included.
    pub len: u32,
    pub peerid: u32,
    pub pid: u32,
}

impl Hdr {
    fn encode(&self, fd: bool) -> [u8; HEADER] {
        let len = if fd { self.len | FD_MARK } else { self.len };
        let mut b = [0u8; HEADER];
        b[0..4].copy_from_slice(&self.typ.to_le_bytes());
        b[4..8].copy_from_slice(&len.to_le_bytes());
        b[8..12].copy_from_slice(&self.peerid.to_le_bytes());
        b[12..16].copy_from_slice(&self.pid.to_le_bytes());
        b
    }

    /// Decode a header; the second value tells whether a descriptor
    /// is expected.
    fn decode(b: &[u8]) -> io::Result<(Self, bool)> {
        let u =
            |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        let raw = u(4);
        let len = raw & !FD_MARK;
        if (len as usize) < HEADER || len as usize > MAX_IMSGSIZE {
            return Err(invalid("bad imsg length"));
        }
        Ok((
            Self {
                typ: u(0),
                len,
                peerid: u(8),
                pid: u(12),
            },
            raw & FD_MARK != 0,
        ))
    }
}

/// A received message.
#[derive(Debug)]
pub struct Imsg {
    pub hdr: Hdr,
    pub data: Vec<u8>,
    pub fd: Option<OwnedFd>,
}

impl Imsg {
    /// Decode the payload.
    pub fn get<T: DeserializeOwned>(&self) -> io::Result<T> {
        postcard::from_bytes(&self.data).map_err(invalid)
    }

    /// Take the descriptor, if any.
    pub fn take_fd(&mut self) -> Option<OwnedFd> {
        self.fd.take()
    }
}

/// Serialize a payload.
pub fn payload<T: Serialize>(data: &T) -> io::Result<Vec<u8>> {
    postcard::to_stdvec(data).map_err(invalid)
}

/// Encode one complete message.
pub fn encode<T: Serialize>(
    typ: u32,
    peerid: u32,
    pid: u32,
    data: &T,
) -> io::Result<Vec<u8>> {
    encode_raw(typ, peerid, pid, &payload(data)?)
}

/// Encode one complete message with an already serialized payload.
pub fn encode_raw(
    typ: u32,
    peerid: u32,
    pid: u32,
    data: &[u8],
) -> io::Result<Vec<u8>> {
    frame(typ, peerid, pid, data, false)
}

fn frame(
    typ: u32,
    peerid: u32,
    pid: u32,
    data: &[u8],
    fd: bool,
) -> io::Result<Vec<u8>> {
    let len = HEADER + data.len();
    if len > MAX_IMSGSIZE {
        return Err(invalid("imsg too large"));
    }
    let hdr = Hdr {
        typ,
        len: len as u32,
        peerid,
        pid,
    };
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&hdr.encode(fd));
    out.extend_from_slice(data);
    Ok(out)
}

/// Decode one message from the front of `buf` without ancillary data.
/// Returns the message and the bytes consumed, or `None` when
/// incomplete.
pub fn decode(buf: &[u8]) -> io::Result<Option<(Imsg, usize)>> {
    if buf.len() < HEADER {
        return Ok(None);
    }
    let (hdr, _) = Hdr::decode(buf)?;
    let end = hdr.len as usize;
    if buf.len() < end {
        return Ok(None);
    }
    Ok(Some((
        Imsg {
            hdr,
            data: buf[HEADER..end].to_vec(),
            fd: None,
        },
        end,
    )))
}

struct Out {
    buf: Vec<u8>,
    off: usize,
    fd: Option<OwnedFd>,
}

/// Buffered, non-blocking message channel over a socket.
pub struct Imsgbuf {
    fd: OwnedFd,
    rbuf: Vec<u8>,
    /// Descriptors received, in order; one per marked message.
    rfds: VecDeque<OwnedFd>,
    wqueue: VecDeque<Out>,
    allow_fdpass: bool,
}

impl Imsgbuf {
    /// Wrap a socket; the caller sets non-blocking mode.
    pub fn new(fd: OwnedFd) -> Self {
        Self {
            fd,
            rbuf: Vec::new(),
            rfds: VecDeque::new(),
            wqueue: VecDeque::new(),
            allow_fdpass: false,
        }
    }

    /// Accept descriptors from the peer.
    /// Off by default: received descriptors are closed.
    pub fn allow_fdpass(&mut self, on: bool) {
        self.allow_fdpass = on;
    }

    pub fn fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    /// Messages waiting to be written.
    pub fn queuelen(&self) -> usize {
        self.wqueue.len()
    }

    /// `poll(2)` events of interest.
    pub fn events(&self) -> libc::c_short {
        if self.wqueue.is_empty() {
            libc::POLLIN
        } else {
            libc::POLLIN | libc::POLLOUT
        }
    }

    /// Queue a message.
    /// `pid` of `None` means the current process.
    pub fn compose<T: Serialize>(
        &mut self,
        typ: u32,
        peerid: u32,
        pid: Option<u32>,
        fd: Option<OwnedFd>,
        data: &T,
    ) -> io::Result<()> {
        self.compose_raw(typ, peerid, pid, fd, &payload(data)?)
    }

    /// Queue a message with an already encoded payload.
    pub fn compose_raw(
        &mut self,
        typ: u32,
        peerid: u32,
        pid: Option<u32>,
        fd: Option<OwnedFd>,
        data: &[u8],
    ) -> io::Result<()> {
        let pid = pid.unwrap_or_else(process::id);
        let buf = frame(typ, peerid, pid, data, fd.is_some())?;
        self.wqueue.push_back(Out { buf, off: 0, fd });
        Ok(())
    }

    /// Read what the socket has.
    /// Returns `false` at end of file; `EAGAIN` is not an error.
    pub fn read(&mut self) -> io::Result<bool> {
        let mut chunk = vec![0u8; READ_SIZE];
        loop {
            match self.recv(&mut chunk) {
                Ok(0) => return Ok(false),
                Ok(n) => self.rbuf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(true);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// Pop the next complete message.
    pub fn get(&mut self) -> io::Result<Option<Imsg>> {
        if self.rbuf.len() < HEADER {
            return Ok(None);
        }
        let (hdr, has_fd) = Hdr::decode(&self.rbuf)?;
        let end = hdr.len as usize;
        if self.rbuf.len() < end {
            return Ok(None);
        }
        let data = self.rbuf[HEADER..end].to_vec();
        self.rbuf.drain(..end);
        let fd = if has_fd { self.rfds.pop_front() } else { None };
        Ok(Some(Imsg { hdr, data, fd }))
    }

    /// Write as much of the queue as the socket takes.
    pub fn write(&mut self) -> io::Result<()> {
        while let Some(out) = self.wqueue.front_mut() {
            match send(
                self.fd.as_raw_fd(),
                &out.buf[out.off..],
                out.fd.as_ref(),
            ) {
                Ok(0) => {
                    return Err(io::Error::from(io::ErrorKind::WriteZero));
                }
                Ok(n) => {
                    // The descriptor travelled with the first byte.
                    out.fd = None;
                    out.off += n;
                    if out.off >= out.buf.len() {
                        self.wqueue.pop_front();
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(());
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Write until the queue is empty, blocking on a non-blocking socket
    /// with `poll(2)`.
    pub fn flush(&mut self) -> io::Result<()> {
        while !self.wqueue.is_empty() {
            self.write()?;
            if self.wqueue.is_empty() {
                break;
            }
            let mut fds = [crate::daemon::pollfd(self.fd(), libc::POLLOUT)];
            crate::daemon::poll(&mut fds, -1)?;
        }
        Ok(())
    }

    /// `recvmsg(2)` into `buf`, keeping the first descriptor received.
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut iov = libc::iovec {
            iov_base: buf.as_mut_ptr().cast(),
            iov_len: buf.len(),
        };
        let mut cbuf = [0u8; CMSG_BUF];
        // SAFETY: msghdr is zeroed then every used field is set to
        // valid, live buffers.
        let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
        msg.msg_iov = &raw mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = cbuf.as_mut_ptr().cast();
        // SAFETY: CMSG_SPACE is a pure size computation.
        msg.msg_controllen =
            unsafe { libc::CMSG_SPACE(size_of::<libc::c_int>() as u32) } as _;

        #[cfg(target_os = "linux")]
        let flags = libc::MSG_CMSG_CLOEXEC;
        #[cfg(not(target_os = "linux"))]
        let flags = 0;

        // SAFETY: msg points to valid buffers for the call.
        let mut n =
            unsafe { libc::recvmsg(self.fd.as_raw_fd(), &raw mut msg, flags) };
        if n == -1
            && io::Error::last_os_error().raw_os_error() == Some(libc::EMSGSIZE)
        {
            // Retry without room for descriptors; whatever the peer sent
            // is dropped by the kernel.
            msg.msg_control = std::ptr::null_mut();
            msg.msg_controllen = 0;
            // SAFETY: as above.
            n = unsafe { libc::recvmsg(self.fd.as_raw_fd(), &raw mut msg, 0) };
        }
        if n == -1 {
            return Err(io::Error::last_os_error());
        }
        if !msg.msg_control.is_null() {
            self.collect_fds(&msg);
        }
        Ok(n as usize)
    }

    fn collect_fds(&mut self, msg: &libc::msghdr) {
        // SAFETY: cmsg pointers come from the macros over a msghdr the
        // kernel just filled in; every access stays inside msg_control.
        unsafe {
            let mut cmsg = libc::CMSG_FIRSTHDR(msg);
            while !cmsg.is_null() {
                let c = &*cmsg;
                if c.cmsg_level == libc::SOL_SOCKET
                    && c.cmsg_type == libc::SCM_RIGHTS
                {
                    let hdr = libc::CMSG_LEN(0) as usize;
                    let n =
                        (c.cmsg_len as usize - hdr) / size_of::<libc::c_int>();
                    let data = libc::CMSG_DATA(cmsg).cast::<libc::c_int>();
                    for i in 0..n {
                        let fd = OwnedFd::from_raw_fd(*data.add(i));
                        if self.allow_fdpass {
                            let _ = set_cloexec(fd.as_raw_fd());
                            self.rfds.push_back(fd);
                        }
                    }
                }
                cmsg = libc::CMSG_NXTHDR(msg, cmsg);
            }
        }
    }
}

/// `sendmsg(2)` with an optional `SCM_RIGHTS` descriptor.
fn send(sock: RawFd, buf: &[u8], fd: Option<&OwnedFd>) -> io::Result<usize> {
    let mut iov = libc::iovec {
        iov_base: buf.as_ptr().cast_mut().cast(),
        iov_len: buf.len(),
    };
    let mut cbuf = [0u8; CMSG_BUF];
    // SAFETY: msghdr is zeroed then every used field is set to valid
    // buffers that outlive the call.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &raw mut iov;
    msg.msg_iovlen = 1;
    if let Some(fd) = fd {
        // SAFETY: cbuf is large enough for one descriptor; the cmsg
        // pointer returned by CMSG_FIRSTHDR is inside it.
        unsafe {
            msg.msg_control = cbuf.as_mut_ptr().cast();
            msg.msg_controllen =
                libc::CMSG_SPACE(size_of::<libc::c_int>() as u32) as _;
            let cmsg = libc::CMSG_FIRSTHDR(&msg);
            (*cmsg).cmsg_level = libc::SOL_SOCKET;
            (*cmsg).cmsg_type = libc::SCM_RIGHTS;
            (*cmsg).cmsg_len =
                libc::CMSG_LEN(size_of::<libc::c_int>() as u32) as _;
            std::ptr::write_unaligned(
                libc::CMSG_DATA(cmsg).cast::<libc::c_int>(),
                fd.as_raw_fd(),
            );
        }
    }
    #[cfg(target_os = "linux")]
    let flags = libc::MSG_NOSIGNAL;
    #[cfg(not(target_os = "linux"))]
    let flags = 0;
    // SAFETY: msg points to valid buffers for the call.
    let n = unsafe { libc::sendmsg(sock, &raw const msg, flags) };
    if n == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(n as usize)
}

/// Blocking read of one message, for the control program.
pub fn read_blocking(r: &mut impl io::Read) -> io::Result<Imsg> {
    let mut hdr = [0u8; HEADER];
    r.read_exact(&mut hdr)?;
    let (hdr, _) = Hdr::decode(&hdr)?;
    let mut data = vec![0u8; hdr.len as usize - HEADER];
    r.read_exact(&mut data)?;
    Ok(Imsg {
        hdr,
        data,
        fd: None,
    })
}

/// Create a connected, non-blocking, close-on-exec socket pair.
pub fn socketpair() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    #[cfg(any(
        target_os = "linux",
        target_os = "openbsd",
        target_os = "freebsd"
    ))]
    let ty = libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
    #[cfg(not(any(
        target_os = "linux",
        target_os = "openbsd",
        target_os = "freebsd"
    )))]
    let ty = libc::SOCK_STREAM;
    // SAFETY: fds is a valid two-element array.
    if unsafe { libc::socketpair(libc::AF_UNIX, ty, 0, fds.as_mut_ptr()) } == -1
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both descriptors were just returned by socketpair(2).
    let (a, b) =
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [&a, &b] {
        crate::daemon::set_nonblock_cloexec(fd.as_raw_fd())?;
    }
    Ok((a, b))
}

fn invalid<E: ToString>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Read, os::fd::AsFd};

    use super::*;

    fn pair() -> (Imsgbuf, Imsgbuf) {
        let (a, b) = socketpair().unwrap();
        (Imsgbuf::new(a), Imsgbuf::new(b))
    }

    fn pump(from: &mut Imsgbuf, to: &mut Imsgbuf) -> Imsg {
        from.flush().unwrap();
        assert!(to.read().unwrap());
        to.get().unwrap().unwrap()
    }

    #[test]
    fn header_roundtrip() {
        let buf = encode(IMSG_CTL_VERBOSE, 7, 9, &true).unwrap();
        assert_eq!(buf.len(), HEADER + 1);
        let (msg, n) = decode(&buf).unwrap().unwrap();
        assert_eq!(n, buf.len());
        assert_eq!(msg.hdr.typ, IMSG_CTL_VERBOSE);
        assert_eq!(msg.hdr.peerid, 7);
        assert_eq!(msg.hdr.pid, 9);
        assert!(msg.get::<bool>().unwrap());
    }

    #[test]
    fn decode_incomplete_is_none() {
        let buf = encode(IMSG_CTL_RELOAD, 0, 0, &"x").unwrap();
        for n in 0..buf.len() {
            assert!(decode(&buf[..n]).unwrap().is_none(), "{n}");
        }
    }

    #[test]
    fn rejects_oversized() {
        assert!(encode(1, 0, 0, &vec![0u8; MAX_IMSGSIZE]).is_err());
        let mut bad = [0u8; HEADER];
        bad[4..8].copy_from_slice(&(MAX_IMSGSIZE as u32 + 1).to_le_bytes());
        assert!(decode(&bad).is_err());
        bad[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(decode(&bad).is_err());
    }

    #[test]
    fn socket_roundtrip() {
        let (mut a, mut b) = pair();
        a.compose(IMSG_CTL_FAIL, 3, None, None, &"nope".to_string())
            .unwrap();
        a.compose(IMSG_RECONF_END, 0, Some(1), None, &()).unwrap();
        assert_eq!(a.queuelen(), 2);
        a.flush().unwrap();
        assert!(b.read().unwrap());
        let m = b.get().unwrap().unwrap();
        assert_eq!(m.hdr.typ, IMSG_CTL_FAIL);
        assert_eq!(m.hdr.peerid, 3);
        assert_eq!(m.hdr.pid, process::id());
        assert_eq!(m.get::<String>().unwrap(), "nope");
        let m = b.get().unwrap().unwrap();
        assert_eq!(m.hdr.typ, IMSG_RECONF_END);
        assert_eq!(m.hdr.pid, 1);
        assert!(b.get().unwrap().is_none());
    }

    #[test]
    fn eof() {
        let (a, mut b) = pair();
        drop(a);
        assert!(!b.read().unwrap());
    }

    #[test]
    fn fd_passing() {
        let (mut a, mut b) = pair();
        b.allow_fdpass(true);
        let f = File::open("/dev/null").unwrap();
        a.compose(
            IMSG_CONTROLFD,
            0,
            None,
            Some(f.as_fd().try_clone_to_owned().unwrap()),
            &(),
        )
        .unwrap();
        let mut m = pump(&mut a, &mut b);
        assert_eq!(m.hdr.typ, IMSG_CONTROLFD);
        let fd = m.take_fd().expect("descriptor");
        let mut got = File::from(fd);
        let mut s = String::new();
        got.read_to_string(&mut s).unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn fd_dropped_when_not_allowed() {
        let (mut a, mut b) = pair();
        let f = File::open("/dev/null").unwrap();
        a.compose(IMSG_CONTROLFD, 0, None, Some(f.into()), &())
            .unwrap();
        let mut m = pump(&mut a, &mut b);
        assert!(m.take_fd().is_none());
    }

    #[test]
    fn no_fd_is_none() {
        let (mut a, mut b) = pair();
        b.allow_fdpass(true);
        a.compose_raw(IMSG_CTL_OK, 0, None, None, &[]).unwrap();
        let mut m = pump(&mut a, &mut b);
        assert!(m.take_fd().is_none());
    }

    #[test]
    fn blocking_helpers() {
        let (a, b) = socketpair().unwrap();
        let mut w = std::os::unix::net::UnixStream::from(a);
        let mut r = std::os::unix::net::UnixStream::from(b);
        w.set_nonblocking(false).unwrap();
        r.set_nonblocking(false).unwrap();
        std::io::Write::write_all(
            &mut w,
            &encode(IMSG_CTL_SHOW_STATUS, 0, 0, &()).unwrap(),
        )
        .unwrap();
        let m = read_blocking(&mut r).unwrap();
        assert_eq!(m.hdr.typ, IMSG_CTL_SHOW_STATUS);
        assert!(m.data.is_empty());
    }
}
