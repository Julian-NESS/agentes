// ==============================================================================
// NESS Relay v2.0.0 — Núcleo del agente (config, motor, logging, actualización)
// Paridad conceptual con el paquete Python `core/` (config, engine, logging, updater).
// ==============================================================================

pub mod config;
pub mod config_backup;
pub mod engine;
pub mod logging;
pub mod restart_handler;
pub mod server_reporter;
pub mod smart_tester;
pub mod update_tracker;
pub mod updater;
