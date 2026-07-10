//! Vendor plugins — Phase 1 bridge to ness-sentinel-core.
//!
//! The `Vendor` enum is re-exported both by its short name (for compatibility)
//! and as `NessVendor` (to disambiguate from string-slug vendor identifiers
//! used elsewhere in ness-relay's `DeviceProfile` trait).

pub use ness_sentinel_core::vendor::*;
pub use ness_sentinel_core::inventory::Vendor;

/// Alias used to disambiguate the SSH-based auditing vendor enum from the
/// SNMP-based vendor string used elsewhere in ness-relay.
pub type NessVendor = Vendor;
