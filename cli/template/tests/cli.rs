// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! Run the binary and check its exit status and output.

use std::process::{Command, Output};

use {{crate_name}}::PROG;

const EXE: &str = env!("CARGO_BIN_EXE_{{project-name}}");

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
{%- if config %}

#[test]
fn missing_config_file() {
    let o = run(&["-f", "/nonexistent/x.toml"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("/nonexistent/x.toml: No such file or directory"),
        "{}",
        stderr(&o)
    );
}

#[test]
fn config_file() {
    let dir = std::env::temp_dir()
        .join(format!("{PROG}-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let conf = dir.join("config.toml");
    std::fs::write(&conf, "name = \"tests\"\n").unwrap();
    let o = Command::new(EXE).arg("-f").arg(&conf).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o), "hello, tests\n");
}
{%- endif %}
