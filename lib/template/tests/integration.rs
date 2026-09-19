// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use {{crate_name}}::{Error, Greeting};

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
{%- if serde %}

#[test]
#[cfg(feature = "serde")]
fn serde_roundtrip() {
    let g = Greeting::new("world").unwrap();
    let json = serde_json::to_string(&g).unwrap();
    assert_eq!(json, r#"{"name":"world"}"#);
    assert_eq!(serde_json::from_str::<Greeting>(&json).unwrap(), g);
}
{%- endif %}
