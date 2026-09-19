// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Control protocol between the control program and the engine, carried
//! as [`imsg`] messages: the message type selects the variant, the
//! payload is the variant's data.

use std::io;

use serde::{Deserialize, Serialize};

use crate::{
    config::Config,
    imsg::{self, Imsg},
};

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

impl Request {
    /// Whether only root (or the daemon's own user) may issue it.
    pub fn privileged(&self) -> bool {
        !matches!(self, Self::ShowStatus)
    }

    /// Message type and payload.
    pub fn parts(&self) -> io::Result<(u32, Vec<u8>)> {
        Ok(match self {
            Self::ShowStatus => (imsg::IMSG_CTL_SHOW_STATUS, Vec::new()),
            Self::LogVerbose(on) => {
                (imsg::IMSG_CTL_VERBOSE, imsg::payload(on)?)
            }
            Self::Reload => (imsg::IMSG_CTL_RELOAD, Vec::new()),
            Self::Shutdown => (imsg::IMSG_CTL_SHUTDOWN, Vec::new()),
        })
    }

    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let (typ, body) = self.parts()?;
        imsg::encode_raw(typ, 0, std::process::id(), &body)
    }

    pub fn from_imsg(m: &Imsg) -> io::Result<Self> {
        Ok(match m.hdr.typ {
            imsg::IMSG_CTL_SHOW_STATUS => Self::ShowStatus,
            imsg::IMSG_CTL_VERBOSE => Self::LogVerbose(m.get()?),
            imsg::IMSG_CTL_RELOAD => Self::Reload,
            imsg::IMSG_CTL_SHUTDOWN => Self::Shutdown,
            t => return Err(unknown(t)),
        })
    }
}

impl Response {
    /// Message type and payload.
    pub fn parts(&self) -> io::Result<(u32, Vec<u8>)> {
        Ok(match self {
            Self::Status(s) => (imsg::IMSG_CTL_STATUS, imsg::payload(s)?),
            Self::Ok => (imsg::IMSG_CTL_OK, Vec::new()),
            Self::Fail(e) => (imsg::IMSG_CTL_FAIL, imsg::payload(e)?),
        })
    }

    pub fn encode(&self, peerid: u32) -> io::Result<Vec<u8>> {
        let (typ, body) = self.parts()?;
        imsg::encode_raw(typ, peerid, std::process::id(), &body)
    }

    pub fn from_imsg(m: &Imsg) -> io::Result<Self> {
        Ok(match m.hdr.typ {
            imsg::IMSG_CTL_STATUS => Self::Status(m.get()?),
            imsg::IMSG_CTL_OK => Self::Ok,
            imsg::IMSG_CTL_FAIL => Self::Fail(m.get()?),
            t => return Err(unknown(t)),
        })
    }
}

fn unknown(t: u32) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("unknown imsg type {t}"))
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
            let buf = req.encode().unwrap();
            let (m, n) = imsg::decode(&buf).unwrap().unwrap();
            assert_eq!(n, buf.len());
            assert_eq!(Request::from_imsg(&m).unwrap(), req);
        }
    }

    #[test]
    fn roundtrip_response() {
        for resp in [
            Response::Status(Status {
                pid: 42,
                uptime_secs: 7,
                config: Config::default(),
                verbose: true,
                reloads: 3,
            }),
            Response::Ok,
            Response::Fail("x".into()),
        ] {
            let buf = resp.encode(9).unwrap();
            let (m, _) = imsg::decode(&buf).unwrap().unwrap();
            assert_eq!(m.hdr.peerid, 9);
            assert_eq!(Response::from_imsg(&m).unwrap(), resp);
        }
    }

    #[test]
    fn unknown_type_is_error() {
        let buf = imsg::encode(999, 0, 0, &()).unwrap();
        let (m, _) = imsg::decode(&buf).unwrap().unwrap();
        assert!(Request::from_imsg(&m).is_err());
        assert!(Response::from_imsg(&m).is_err());
    }

    #[test]
    fn only_status_is_unprivileged() {
        assert!(!Request::ShowStatus.privileged());
        assert!(Request::Reload.privileged());
        assert!(Request::LogVerbose(false).privileged());
        assert!(Request::Shutdown.privileged());
    }
}
