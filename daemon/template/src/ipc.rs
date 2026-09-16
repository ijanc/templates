// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Wire format between daemon and control program.
//!
//! A frame is a little-endian `u32` body length followed by the body,
//! a postcard-encoded [`Request`] or [`Response`].

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::config::Config;

/// Largest accepted frame body in bytes.
pub const MAX_FRAME: usize = 16384;

const HEADER: usize = 4;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Request {
    ShowStatus,
    LogVerbose(bool),
    Reload,
    Shutdown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Response {
    Status(Status),
    Ok,
    Fail(String),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Status {
    pub pid: u32,
    pub uptime_secs: u64,
    pub config: Config,
    pub verbose: bool,
    pub reloads: u32,
}

/// Encode `msg` as a complete frame.
pub fn encode<T: Serialize>(msg: &T) -> io::Result<Vec<u8>> {
    let body = postcard::to_stdvec(msg).map_err(invalid)?;
    if body.len() > MAX_FRAME {
        return Err(invalid("frame too large"));
    }
    let mut out = Vec::with_capacity(HEADER + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Decode one frame from the front of `buf`.
/// Returns the message and the number of bytes consumed, or `None` when
/// the frame is incomplete.
pub fn decode<T: DeserializeOwned>(
    buf: &[u8],
) -> io::Result<Option<(T, usize)>> {
    if buf.len() < HEADER {
        return Ok(None);
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_FRAME {
        return Err(invalid("frame too large"));
    }
    let end = HEADER + len;
    if buf.len() < end {
        return Ok(None);
    }
    let msg = postcard::from_bytes(&buf[HEADER..end]).map_err(invalid)?;
    Ok(Some((msg, end)))
}

/// Blocking read of one frame.
pub fn read_frame<T: DeserializeOwned>(r: &mut impl Read) -> io::Result<T> {
    let mut hdr = [0u8; HEADER];
    r.read_exact(&mut hdr)?;
    let len = u32::from_le_bytes(hdr) as usize;
    if len > MAX_FRAME {
        return Err(invalid("frame too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    postcard::from_bytes(&body).map_err(invalid)
}

/// Blocking write of one frame.
pub fn write_frame<T: Serialize>(
    w: &mut impl Write,
    msg: &T,
) -> io::Result<()> {
    w.write_all(&encode(msg)?)?;
    w.flush()
}

fn invalid<E: ToString>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_request() {
        for req in [
            Request::ShowStatus,
            Request::LogVerbose(true),
            Request::Reload,
            Request::Shutdown,
        ] {
            let buf = encode(&req).unwrap();
            let (got, n): (Request, usize) = decode(&buf).unwrap().unwrap();
            assert_eq!(got, req);
            assert_eq!(n, buf.len());
        }
    }

    #[test]
    fn roundtrip_response_over_stream() {
        let resp = Response::Status(Status {
            pid: 42,
            uptime_secs: 7,
            config: Config::default(),
            verbose: true,
            reloads: 3,
        });
        let mut buf = Vec::new();
        write_frame(&mut buf, &resp).unwrap();
        let got: Response = read_frame(&mut &buf[..]).unwrap();
        assert_eq!(got, resp);
    }

    #[test]
    fn decode_incomplete_is_none() {
        let buf = encode(&Request::Reload).unwrap();
        for n in 0..buf.len() {
            let r: Option<(Request, usize)> = decode(&buf[..n]).unwrap();
            assert!(r.is_none(), "prefix of {n} bytes decoded");
        }
    }

    #[test]
    fn decode_keeps_trailing_bytes() {
        let mut buf = encode(&Request::Reload).unwrap();
        let n = buf.len();
        buf.extend_from_slice(&[9, 9, 9]);
        let (_, used): (Request, usize) = decode(&buf).unwrap().unwrap();
        assert_eq!(used, n);
    }

    #[test]
    fn oversize_frame_rejected() {
        let mut buf = ((MAX_FRAME + 1) as u32).to_le_bytes().to_vec();
        buf.push(0);
        let e = decode::<Request>(&buf).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
        let e = read_frame::<Request>(&mut &buf[..]).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn truncated_stream_is_unexpected_eof() {
        let buf = encode(&Request::Shutdown).unwrap();
        let e = read_frame::<Request>(&mut &buf[..2]).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
        let e = read_frame::<Request>(&mut &buf[..buf.len() - 1]).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn garbage_body_rejected() {
        let mut buf = 2u32.to_le_bytes().to_vec();
        buf.extend_from_slice(&[0xff, 0xff]);
        let e = decode::<Request>(&buf).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
    }
}
