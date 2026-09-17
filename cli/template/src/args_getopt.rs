// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

{% if config -%}
use std::{env, path::PathBuf, process};
{%- else -%}
use std::{env, process};
{%- endif %}

use crate::{PROG, version};

const USAGE: &str = "usage: {{project-name}} [-hVv]{% if config %} [-f file]{% endif %} [arg ...]";

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
{%- if config %}
    /// Configuration file given with `-f`.
    pub conf: Option<PathBuf>,
{%- endif %}
    /// Remaining positional arguments.
    pub args: Vec<String>,
}

pub fn parse() -> Opts {
    let args: Vec<String> = env::args().collect();
    let mut p = getopt::Parser::new(&args, "{% if config %}f:{% endif %}hVv");
    let mut o = Opts {
        verbose: 0,
{%- if config %}
        conf: None,
{%- endif %}
        args: Vec::new(),
    };
    loop {
        match p.next().transpose() {
            Ok(None) => break,
{%- if config %}
            Ok(Some(getopt::Opt('f', Some(f)))) => o.conf = Some(f.into()),
{%- endif %}
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
