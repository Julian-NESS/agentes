//! ness-relay-core — Audit subsystem for ness-relay v2.4.0+.
//!
//! **Phase 1 (bridge)**: this crate re-exports the relevant modules from
//! `ness-sentinel-core` so that `ness-relay` (the binary) can adopt the
//! `--audit` flag without having to physically copy ~14k lines of code.
//!
//! **Phase 2+ (native)**: native implementations of `ssh`, `vendor`, `cis`,
//! `vulns`, and `report` will replace the re-exports. The public API of this
//! crate will remain stable so consumers are not affected.
//!
//! # Modules
//!
//! - [`ssh`] — Re-exports `SshClient`, `SshCredentials`, `SshTarget`, etc.
//! - [`vendor`] — Re-exports `PluginRegistry`, `FortiOsPlugin`, `VendorFacts`, etc.
//! - [`cis`] — Re-exports `CisEngine`, `all_fortios_checks`, `Check`, etc.
//! - [`vulns`] — Re-exports `VulnEngine`, `NvdClient`, `EpssClient`, `KevCatalog`.
//! - [`report`] — Re-exports `render` functions for json/markdown/html.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod cis;
pub mod report;
pub mod ssh;
pub mod vendor;
pub mod vulns;

/// Library semantic version. Read from `CARGO_PKG_VERSION` at build time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name (matches Cargo.toml `[package].name`).
pub const NAME: &str = "ness-relay-core";
