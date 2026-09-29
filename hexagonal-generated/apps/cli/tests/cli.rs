// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Run the binary end to end against a temporary SQLite file.

use std::process::{Command, Output};

const EXE: &str = env!("CARGO_BIN_EXE_hexagonal-generated-cli");

const PROG: &str = "hexagonal-generated-cli";

/// Run without any database.
fn bare(args: &[&str]) -> Output {
    Command::new(EXE)
        .env_remove("DATABASE_URL")
        .args(args)
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// A migrated database.
struct Db {
    url: String,
    dir: std::path::PathBuf,
}

impl Db {
    fn new() -> Option<Self> {
        let dir = std::env::temp_dir()
            .join(format!("{PROG}-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let url = format!("sqlite://{}/items.db?mode=rwc", dir.display());
        let db = Self { url, dir };
        let o = db.run(&["migrate"]);
        assert!(o.status.success(), "{}", stderr(&o));
        Some(db)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(EXE)
            .env("DATABASE_URL", &self.url)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn version() {
    let o = bare(&["--version"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.starts_with(&format!("{PROG} ")), "{out}");
    assert!(out.contains(env!("CARGO_PKG_VERSION")), "{out}");
}

#[test]
fn help() {
    let o = bare(&["--help"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stdout(&o).contains("items"), "{}", stdout(&o));
}

#[test]
fn needs_a_database_url() {
    let o = bare(&["items", "list"]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("DATABASE_URL"), "{}", stderr(&o));
}

#[test]
fn crud() {
    let Some(db) = Db::new() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let o = db.run(&["items", "create", "widget", "-d", "d"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let id = stdout(&o).trim().to_owned();
    assert!(uuid::Uuid::parse_str(&id).is_ok(), "{id}");

    let o = db.run(&["items", "list"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(
        stdout(&o).contains(&format!("{id}\twidget\n")),
        "{}",
        stdout(&o)
    );

    let o = db.run(&["items", "get", &id]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("name\twidget\n"), "{out}");
    assert!(out.contains("description\td\n"), "{out}");

    let o = db.run(&["items", "update", &id, "gadget"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let o = db.run(&["items", "get", &id]);
    let out = stdout(&o);
    assert!(out.contains("name\tgadget\n"), "{out}");
    assert!(out.contains("description\t\n"), "{out}");

    let o = db.run(&["items", "create", ""]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("name: must not be blank"),
        "{}",
        stderr(&o)
    );

    let o = db.run(&["items", "delete", &id]);
    assert!(o.status.success(), "{}", stderr(&o));
    let o = db.run(&["items", "delete", &id]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("not found"), "{}", stderr(&o));
}
