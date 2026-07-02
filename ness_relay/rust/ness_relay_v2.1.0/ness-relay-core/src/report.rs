//! Report renderers — Phase 1 bridge to ness-sentinel-core.
//!
//! Only the format renderers are re-exported; the SQLite-backed `store`
//! module stays in ness-sentinel-core (out of scope for Phase 1).

pub use ness_sentinel_core::report::{
    html, json, markdown, terminal, ReportFormat,
};
