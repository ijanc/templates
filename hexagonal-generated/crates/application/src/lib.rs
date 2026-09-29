// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Use cases. [`ItemUseCases`] is the driving port, what the HTTP and
//! command line adapters call; [`ItemService`] implements it over the
//! domain's driven ports, so it runs on any store, including the
//! in-memory one the tests use.

pub mod error;
pub mod ports;
pub mod service;

pub use error::{Error, FieldError};
pub use ports::{
    CreateItem, ItemUseCases, LIMIT_DEFAULT, LIMIT_MAX, ListQuery, Page,
    UpdateItem,
};
pub use service::ItemService;
