//! Common test doubles and helpers for flow tests.
//! Re-exports the shared FakeLlm server from `tests/support` as MockServer.

#![allow(dead_code)]

#[path = "../support/mod.rs"]
pub mod support;

pub use support::*;

/// Alias `MockServer` to the shared `FakeLlm` test server.
pub type MockServer = support::FakeLlm;

