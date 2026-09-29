// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

//! The domain: what an item is, the rules it obeys and, as traits, what
//! the application needs from the outside world. Nothing here knows
//! about HTTP, databases or the async runtime; the adapters do.

pub mod item;
pub mod ports;

pub use item::{Item, NAME_MAX, Name, NameError};
pub use ports::{ItemRepository, RepositoryError};
