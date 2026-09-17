// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Run the binary and check its exit status and output.

use std::process::{Command, Output};

use cli_clap_generated::PROG;

const EXE: &str = env!("CARGO_BIN_EXE_cli-clap-generated");

fn run(args: &[&str]) -> Output {
    Command::new(EXE).args(args).output().unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn version() {
    let o = run(&["-V"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.starts_with(&format!("{PROG} ")), "{out}");
    assert!(out.contains(env!("CARGO_PKG_VERSION")), "{out}");
}

#[test]
fn help() {
    let o = run(&["-h"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stdout(&o).contains(PROG));
}

#[test]
fn unknown_option() {
    let o = run(&["-Z"]);
    assert!(!o.status.success());
    assert!(!stderr(&o).is_empty());
}

#[test]
fn runs_with_arguments() {
    let o = run(&["-vv", "foo", "bar"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stdout(&o).starts_with("hello"), "{}", stdout(&o));
    let err = stderr(&o);
    assert!(err.contains("arg: foo"), "{err}");
    assert!(err.contains("arg: bar"), "{err}");
    assert!(err.contains("done"), "{err}");
}
