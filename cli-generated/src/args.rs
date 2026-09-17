// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::{env, path::PathBuf, process};

use crate::{PROG, version};

const USAGE: &str = "usage: cli-generated [-hVv] [-f file] [arg ...]";

/// Print usage and exit; `-h` goes to stdout with status 0, errors to
/// stderr with status 1.
fn usage(status: i32) -> ! {
    if status == 0 {
        println!("{USAGE}");
    } else {
        eprintln!("{USAGE}");
    }
    process::exit(status);
}

pub struct Opts {
    /// Number of `-v` flags.
    pub verbose: u8,
    /// Configuration file given with `-f`.
    pub conf: Option<PathBuf>,
    /// Remaining positional arguments.
    pub args: Vec<String>,
}

pub fn parse() -> Opts {
    let args: Vec<String> = env::args().collect();
    let mut p = getopt::Parser::new(&args, "f:hVv");
    let mut o = Opts {
        verbose: 0,
        conf: None,
        args: Vec::new(),
    };
    loop {
        match p.next().transpose() {
            Ok(None) => break,
            Ok(Some(getopt::Opt('f', Some(f)))) => o.conf = Some(f.into()),
            Ok(Some(getopt::Opt('h', _))) => usage(0),
            Ok(Some(getopt::Opt('V', _))) => {
                println!("{PROG} {}", version());
                process::exit(0);
            }
            Ok(Some(getopt::Opt('v', _))) => o.verbose += 1,
            Ok(Some(_)) => unreachable!(),
            Err(e) => {
                eprintln!("{PROG}: {e}");
                usage(1);
            }
        }
    }
    o.args = args[p.index()..].to_vec();
    o
}
