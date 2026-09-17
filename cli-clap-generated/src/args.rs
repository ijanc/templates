// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::sync::LazyLock;

use clap::{ArgAction, Parser};

use crate::version;

static VERSION: LazyLock<String> = LazyLock::new(version);

#[derive(Debug, Parser)]
#[command(name = "cli-clap-generated", version = VERSION.as_str())]
#[command(about = "An example generated using the cli template")]
pub struct Opts {
    /// Increase verbosity; once for debug, twice for trace messages
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,
    /// Remaining positional arguments
    #[arg(value_name = "ARG", trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub fn parse() -> Opts {
    Opts::parse()
}
