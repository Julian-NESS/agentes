// ==============================================================================
// NESS Relay v2.0.0 — Módulo de vendors (Rutas Corregidas)
// ==============================================================================

// 1. Importaciones de la carpeta superior (Core)
pub use crate::profiles::base;
pub use crate::profiles::loader;
pub use crate::profiles::standard_oids;

// 2. Declaración de módulos con RUTAS EXPLÍCITAS
// Esto le dice a Rust: "Sal de vendors y busca en la carpeta de arriba"

#[path = "../switches/mod.rs"]
pub mod switches;

#[path = "../routers/mod.rs"]
pub mod routers;

#[path = "../firewalls/mod.rs"]
pub mod firewalls;

#[path = "../wireless/mod.rs"]
pub mod wireless;

#[path = "../generic/mod.rs"]
pub mod generic;

// 3. Re-exportación para el Loader
// (Asegúrate de que estos nombres existan dentro de los mod.rs de cada carpeta)

pub use self::switches::{cisco, huawei, dell, datacomm, tp_link};
pub use self::routers::{mikrotik, juniper}; 
pub use self::firewalls::{fortinet, pfsense};
pub use self::wireless::{ubnt, c_n};
pub use self::generic::{linux, windows};