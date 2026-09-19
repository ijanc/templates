// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! {{project-description}}
//!
//! ```
//! use {{crate_name}}::Greeting;
//!
//! let g = Greeting::new("world").unwrap();
//! assert_eq!(g.to_string(), "hello, world");
//! ```
{%- if serde %}
//!
//! # Features
//!
//! - `serde`: `Serialize`/`Deserialize` for public types.
{%- endif %}

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod error;

use std::fmt;

pub use error::{Error, Result};

/// A greeting for `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
{%- if serde %}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
{%- endif %}
pub struct Greeting {
    name: String,
}

impl Greeting {
    /// Create a greeting. Fails with [`Error::Empty`] when `name` is
    /// blank.
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(Error::Empty);
        }
        Ok(Self { name })
    }

    /// The greeted name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for Greeting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hello, {}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_name() {
        assert_eq!(Greeting::new("  ").unwrap_err(), Error::Empty);
    }
}
