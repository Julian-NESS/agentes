// =============================================================================
// ness-relay — Library entry point (v2.5.0)
// =============================================================================
//
// Permite que el binario `ness-relay-cred` consuma los módulos públicos
// de `ness-relay` (notablemente `secrets`). El binario principal
// `ness-relay` sigue funcionando como antes gracias a `main.rs`.
//
// Esta separación `lib.rs` + `main.rs` es el patrón estándar de Cargo
// para crates con múltiples binarios.
// =============================================================================

#![deny(unsafe_code)]

// Re-exportar el módulo `secrets` (incluyendo submódulos) para que
// `ness-relay-cred` pueda hacer `use ness_relay::secrets::*`.
pub mod secrets;

// Otros módulos del binario principal. `ness-relay-cred` solo necesita
// `secrets` por ahora, pero los exponemos para que en el futuro el
// `ness-relay-cred` pueda ampliar su funcionalidad (rotación de claves,
// integración con Vault externo, etc.) sin tocar el binario principal.
#[allow(unused_imports)]
pub use crate::secrets as public_secrets;
