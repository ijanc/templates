// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 generate-lib-serde <author@example.com>

use lib_generated::{Error, Greeting};

#[test]
fn display() {
    let g = Greeting::new("world").unwrap();
    assert_eq!(g.name(), "world");
    assert_eq!(g.to_string(), "hello, world");
}

#[test]
fn empty() {
    assert_eq!(Greeting::new("").unwrap_err(), Error::Empty);
}
