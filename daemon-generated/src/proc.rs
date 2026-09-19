// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Privilege separation: the parent re-executes itself for every child
//! process, handing it one end of a socket pair on descriptor 3.

use std::{
    env, fs, io,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child as StdChild, Command, ExitStatus},
    sync::OnceLock,
};

use crate::{
    daemon::{clear_cloexec, set_nonblock_cloexec},
    imsg::{self, Imsgbuf},
};

/// Descriptor a child finds its parent socket on.
pub const PARENT_SOCK_FILENO: libc::c_int = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProcId {
    Parent,
    Engine,
}

impl ProcId {
    /// Name used with `-P` and in logs.
    pub fn title(self) -> &'static str {
        match self {
            Self::Parent => "parent",
            Self::Engine => "engine",
        }
    }

    pub fn from_title(s: &str) -> Option<Self> {
        match s {
            "parent" => Some(Self::Parent),
            "engine" => Some(Self::Engine),
            _ => None,
        }
    }
}

static EXEC_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Record the path to re-execute, from the kernel when it knows it or
/// from `argv[0]` otherwise.
/// Must run before any `chdir(2)`.
pub fn init_exec_path(argv0: &str) -> io::Result<()> {
    let p = match env::current_exe() {
        Ok(p) if p.is_absolute() => p,
        _ => fs::canonicalize(argv0)?,
    };
    let _ = EXEC_PATH.set(p);
    Ok(())
}

pub fn exec_path() -> &'static Path {
    EXEC_PATH.get().map_or(Path::new(""), PathBuf::as_path)
}

/// Options every child inherits on its command line.
#[derive(Clone, Debug, Default)]
pub struct ChildOpts {
    pub debug: bool,
    pub verbose: u8,
    /// User to drop to, and the directory to chroot into.
    pub privdrop: Option<(String, PathBuf)>,
}

impl ChildOpts {
    fn argv(&self, id: ProcId) -> Vec<String> {
        let mut v = vec!["-P".into(), id.title().into()];
        if self.debug {
            v.push("-d".into());
        }
        for _ in 0..self.verbose {
            v.push("-v".into());
        }
        if let Some((user, dir)) = &self.privdrop {
            v.push("-u".into());
            v.push(user.clone());
            v.push("-C".into());
            v.push(dir.display().to_string());
        }
        v
    }
}

/// A running child and the channel to it.
pub struct Child {
    pub id: ProcId,
    child: StdChild,
    pub ibuf: Imsgbuf,
}

impl Child {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Non-blocking check for exit.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Wait for exit.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    /// Send SIGTERM; a child that already exited is not an error.
    pub fn terminate(&self) {
        // SAFETY: kill on a pid we spawned and have not reaped.
        unsafe {
            libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM);
        }
    }
}

/// Fork and exec `id` with the parent socket on
/// [`PARENT_SOCK_FILENO`].
/// Standard descriptors are inherited.
pub fn spawn(id: ProcId, opts: &ChildOpts) -> io::Result<Child> {
    let (ours, theirs) = imsg::socketpair()?;
    let theirs = theirs.into_raw_fd();
    let mut cmd = Command::new(exec_path());
    cmd.args(opts.argv(id));
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        cmd.pre_exec(move || {
            if theirs != PARENT_SOCK_FILENO {
                if libc::dup2(theirs, PARENT_SOCK_FILENO) == -1 {
                    return Err(io::Error::last_os_error());
                }
            } else {
                clear_cloexec(theirs)?;
            }
            Ok(())
        });
    }
    let child = cmd.spawn();
    // SAFETY: theirs is still open in this process and owned by no one
    // else.
    drop(unsafe { OwnedFd::from_raw_fd(theirs) });
    let child = child?;
    set_nonblock_cloexec(ours.as_raw_fd())?;
    Ok(Child {
        id,
        child,
        ibuf: Imsgbuf::new(ours),
    })
}

/// The socket to the parent, in a child started by [`spawn`].
///
/// # Safety
/// Call once per process, before anything else touches descriptor 3.
pub unsafe fn parent_socket() -> io::Result<Imsgbuf> {
    // SAFETY: guaranteed by the caller.
    let fd = unsafe { OwnedFd::from_raw_fd(PARENT_SOCK_FILENO) };
    set_nonblock_cloexec(fd.as_raw_fd())?;
    Ok(Imsgbuf::new(fd))
}
