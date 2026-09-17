// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

{% if config -%}
use std::{path::PathBuf, sync::LazyLock};
{%- else -%}
use std::sync::LazyLock;
{%- endif %}

use clap::{ArgAction, Parser};

use crate::version;

static VERSION: LazyLock<String> = LazyLock::new(version);

#[derive(Debug, Parser)]
#[command(name = "{{project-name}}", version = VERSION.as_str())]
#[command(about = "{{project-description}}")]
pub struct Opts {
    /// Increase verbosity; once for debug, twice for trace messages
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,
{%- if config %}
    /// Configuration file
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    pub conf: Option<PathBuf>,
{%- endif %}
    /// Remaining positional arguments
    #[arg(value_name = "ARG", trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub fn parse() -> Opts {
    Opts::parse()
}
