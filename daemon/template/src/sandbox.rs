// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Process sandboxing with the `pledge(2)`/`unveil(2)` interface.
//!
//! On OpenBSD the calls go straight to the kernel.
//! On Linux with the `landlock` feature, [`unveil`] and [`unveil_lock`]
//! build a Landlock ruleset restricting file system access to the
//! unveiled paths; [`pledge`] does nothing.
//! Landlock rules bind to inodes, so a rule for a file is placed on its
//! directory (without the right to list it) to survive the file being
//! replaced, as happens on every configuration edit.
//! Everywhere else all three are no-ops.

use std::{io, path::Path};

/// Restrict the process to the given promises.
pub fn pledge(promises: &str) -> io::Result<()> {
    imp::pledge(promises)
}

/// Expose `path` with `perms`, a subset of `rwxc`.
/// The first call hides everything else once [`unveil_lock`] runs.
pub fn unveil(path: &Path, perms: &str) -> io::Result<()> {
    imp::unveil(path, perms)
}

/// Disallow further [`unveil`] calls and activate the restrictions.
pub fn unveil_lock() -> io::Result<()> {
    imp::unveil_lock()
}

#[cfg(target_os = "openbsd")]
mod imp {
    use std::{ffi::CString, io, os::unix::ffi::OsStrExt, path::Path};

    pub fn pledge(promises: &str) -> io::Result<()> {
        let p = CString::new(promises)?;
        // SAFETY: valid NUL-terminated string; NULL execpromises keeps
        // the current ones.
        if unsafe { libc::pledge(p.as_ptr(), std::ptr::null()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn unveil(path: &Path, perms: &str) -> io::Result<()> {
        let p = CString::new(path.as_os_str().as_bytes())?;
        let a = CString::new(perms)?;
        // SAFETY: both are valid NUL-terminated strings.
        if unsafe { libc::unveil(p.as_ptr(), a.as_ptr()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn unveil_lock() -> io::Result<()> {
        // SAFETY: NULL arguments lock the unveil list.
        if unsafe { libc::unveil(std::ptr::null(), std::ptr::null()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(all(target_os = "linux", feature = "landlock"))]
mod imp {
    use std::{
        io,
        path::{Path, PathBuf},
        sync::Mutex,
    };

    use landlock::{
        ABI, Access, AccessFs, BitFlags, CompatLevel, Compatible, PathBeneath,
        PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr,
    };

    /// Newest ABI whose access rights this module knows how to map.
    const ABI_LEVEL: ABI = ABI::V3;

    /// Paths collected by `unveil`, applied by `unveil_lock`.
    static RULES: Mutex<Vec<(PathBuf, BitFlags<AccessFs>)>> =
        Mutex::new(Vec::new());

    pub fn pledge(_promises: &str) -> io::Result<()> {
        Ok(())
    }

    pub fn unveil(path: &Path, perms: &str) -> io::Result<()> {
        let is_dir = path.is_dir();
        let mut access = BitFlags::<AccessFs>::EMPTY;
        let mut create = false;
        for c in perms.chars() {
            match c {
                'r' => access |= AccessFs::ReadFile | AccessFs::ReadDir,
                'w' => access |= AccessFs::WriteFile | AccessFs::Truncate,
                'x' => access |= AccessFs::Execute,
                'c' => create = true,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("bad unveil permission {c:?}"),
                    ));
                }
            }
        }
        let mut rules = RULES.lock().unwrap_or_else(|e| e.into_inner());
        if is_dir {
            if create {
                access |= AccessFs::from_write(ABI_LEVEL);
            }
            rules.push((path.to_path_buf(), access));
        } else {
            // File rights go on the directory; creating or removing the
            // file is a directory right anyway.
            let dir = path.parent().unwrap_or(path).to_path_buf();
            access &= AccessFs::from_file(ABI_LEVEL);
            if create {
                access |= AccessFs::RemoveFile
                    | AccessFs::MakeReg
                    | AccessFs::MakeSock;
            }
            rules.push((dir, access));
        }
        Ok(())
    }

    pub fn unveil_lock() -> io::Result<()> {
        let rules = std::mem::take(
            &mut *RULES.lock().unwrap_or_else(|e| e.into_inner()),
        );
        let mut ruleset = Ruleset::default()
            .set_compatibility(CompatLevel::BestEffort)
            .handle_access(AccessFs::from_all(ABI_LEVEL))
            .map_err(io::Error::other)?
            .create()
            .map_err(io::Error::other)?;
        for (path, access) in rules {
            if access.is_empty() {
                continue;
            }
            let fd = match PathFd::new(&path) {
                Ok(fd) => fd,
                Err(e) => {
                    log::debug!("unveil {}: {e}", path.display());
                    continue;
                }
            };
            ruleset = ruleset
                .add_rule(PathBeneath::new(fd, access))
                .map_err(io::Error::other)?;
        }
        let status = ruleset.restrict_self().map_err(io::Error::other)?;
        log::debug!("landlock: {:?}", status.ruleset);
        Ok(())
    }
}

#[cfg(not(any(
    target_os = "openbsd",
    all(target_os = "linux", feature = "landlock")
)))]
mod imp {
    use std::{io, path::Path};

    pub fn pledge(_promises: &str) -> io::Result<()> {
        Ok(())
    }

    pub fn unveil(_path: &Path, _perms: &str) -> io::Result<()> {
        Ok(())
    }

    pub fn unveil_lock() -> io::Result<()> {
        Ok(())
    }
}

#[cfg(all(test, not(target_os = "openbsd")))]
mod tests {
    use super::*;

    #[test]
    fn calls_succeed() {
        assert!(pledge("stdio").is_ok());
        assert!(unveil(Path::new("/nonexistent"), "r").is_ok());
    }
}
