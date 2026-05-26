// ==============================================================================
// NESS Relay v2.0.0 — Módulo de perfiles de dispositivo
// ==============================================================================
// ==============================================================================
// NESS Relay v2.0.0 — Módulo de vendors (FINAL - SOLO ARCHIVOS REALES)
// ==============================================================================

// 1. Declaración de las carpetas como módulos
#[path = "../switches/mod.rs"]
pub mod switches;

#[path = "../routers/mod.rs"]
pub mod routers;

#[path = "../firewalls/mod.rs"]
pub mod firewalls;

#[path = "../wireless/mod.rs"]
pub mod wireless;

// 2. Re-exportación (ÚNICAMENTE lo que sale en tus capturas)
// SWITCHES: cisco, huawei, dell, datacomm, tp_link
pub use self::switches::{cisco, huawei, dell, datacomm, tp_link};

// ROUTERS: mikrotik
pub use self::routers::{mikrotik}; 

// FIREWALLS: fortinet, mikrotik_fw, pfsense, sophos
pub use self::firewalls::{fortinet, mikrotik_fw, pfsense, sophos};

// WIRELESS: ubnt
pub use self::wireless::{ubnt};