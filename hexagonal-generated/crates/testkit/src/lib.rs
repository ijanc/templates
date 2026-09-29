// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Test support shared by the adapters: the repository contract, one
//! set of checks every `ItemRepository` has to pass, stamped out per
//! store with [`repository_contract_tests!`].

pub mod contract;

/// Generate the contract tests for one store. `$make` is an
/// `async fn() -> Option<R>` giving a fresh, empty repository, or `None`
/// when its backing service is not available, which skips the tests.
/// The caller needs `tokio` as a dev-dependency.
#[macro_export]
macro_rules! repository_contract_tests {
    ($make:path) => {
        $crate::contract_test!($make, ping);
        $crate::contract_test!($make, insert_then_get);
        $crate::contract_test!($make, list_oldest_first);
        $crate::contract_test!($make, count);
        $crate::contract_test!($make, update_replaces);
        $crate::contract_test!($make, delete_twice);
    };
}

/// One test of [`repository_contract_tests!`].
#[doc(hidden)]
#[macro_export]
macro_rules! contract_test {
    ($make:path, $name:ident) => {
        #[tokio::test]
        async fn $name() {
            let _serial = $crate::contract::serial().await;
            let Some(repo) = $make().await else {
                eprintln!("repository unavailable, skipping");
                return;
            };
            $crate::contract::$name(&repo).await;
        }
    };
}
