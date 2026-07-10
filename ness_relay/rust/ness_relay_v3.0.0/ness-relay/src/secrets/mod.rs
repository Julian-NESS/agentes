// =============================================================================
// NESS Relay v2.5.0 — Credential encryption module
// =============================================================================
//
// Cifrado en reposo de credenciales (SNMPv3 auth/priv + SSH passwords)
// con AES-256-GCM y clave maestra derivada del host (HKDF-SHA256).
//
// Componentes:
//   - crypto.rs:  wrapper AES-256-GCM con zeroize y AAD contextual.
//   - vault.rs:   derivación de la clave maestra (machine-id + sal local).
//   - env_file.rs: cifrado binario de `/etc/ness_relay/secrets.enc` (reemplaza
//                  al `secrets.env` en texto plano).
//   - migration.rs: lee `connection.config` plano, pide pass por consola,
//                   cifra, escribe `$enc$2$...` y respalda el original.
//
// API pública usada por el resto del agente:
//   - `is_encrypted_token(s)`           → bool
//   - `encrypt_str(plain, aad)`         → Result<String>
//   - `decrypt_str(token, aad)`         → Result<String>
//   - `decrypt_optional(plain_or_token, aad)` → Result<String>  (compat v2.4)
//   - `master_key()`                    → Result<Zeroizing<[u8; 32]>>
//   - `vault_dir()` / `secrets_path()`  → Result<PathBuf>
//   - `migrate_plaintext_config(...)`   → Result<MigrationReport>
//   - `load_env_file_decrypted(...)`    → Result<HashMap<String,String>>
//   - `save_env_file_encrypted(...)`    → Result<()>
//
// Formato de token (en `connection.config`):
//     $enc$<ver>$<base64(nonce(12B) || ciphertext || tag(16B))>
// Donde `<ver>` es la versión del esquema (1 = legacy CBC | 2 = AES-256-GCM).
//
// El AAD recomendado por el llamador es `vendor|device_idx|field_name` para
// impedir copiar un cifrado entre campos de dispositivos distintos.
// =============================================================================

#![deny(unsafe_code)]
// Las re-exports de `pub use` están pensadas para que el resto del agente
// las consuma (Fases 2-3 de la hoja de ruta). En esta Fase 1 aún no hay
// consumidores, así que las silenciamos para no contaminar `cargo build`
// con warnings.
#![allow(unused_imports)]

pub mod crypto;
pub mod env_file;
pub mod migration;
pub mod vault;

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use zeroize::Zeroizing;

pub use crypto::{decrypt_str, decrypt_optional, encrypt_str, Error as CryptoError, ENC_PREFIX,
                SCHEME_VERSION};
pub use env_file::{load_env_file_decrypted, save_env_file_encrypted, EnvFileError};
pub use migration::{migrate_plaintext_config, MigrationReport};
pub use vault::{ensure_vault, master_key, vault_dir, vault_paths, VaultPaths};
// Re-exportar el TIPO Zeroizing para call sites que no quieran importar
// zeroize directamente.
pub use zeroize::Zeroizing as ZVec;

// ------------------------------------------------------------------
// Constantes públicas (reusables desde otros módulos)
// ------------------------------------------------------------------

/// Prefijo que marca una cadena como token cifrado. Todo lo que no comience
/// con este prefijo se considera plaintext y se devuelve tal cual (v2.4.0
/// compatibility).
pub const fn encrypted_token_prefix() -> &'static str { ENC_PREFIX }

// ------------------------------------------------------------------
// Helpers de paths
// ------------------------------------------------------------------

/// Ubicación canónica de `/etc/ness_relay` (la única soportada).
pub fn secrets_root() -> PathBuf { PathBuf::from("/etc/ness_relay") }

/// Compat: `vault_dir()` ya vive en `vault.rs`; este alias reduce el ruido
/// en los call sites.
pub fn secrets_dir() -> PathBuf { vault_dir() }

/// Ruta al archivo binario cifrado de env vars (`/etc/ness_relay/secrets.enc`).
pub fn secrets_file() -> PathBuf { secrets_root().join("secrets.enc") }

/// Ruta al `connection.config` (lo usa `migration.rs` para escanear campos
/// a cifrar de forma retroactiva).
pub fn connection_config_default() -> PathBuf {
    // Respeta `NESS_DEVICES_FILE` si está presente.
    if let Ok(p) = std::env::var("NESS_DEVICES_FILE") {
        return PathBuf::from(p);
    }
    // Convencional: `/opt/ness_relay/configs/connection.config`.
    PathBuf::from("/opt/ness_relay/configs/connection.config")
}

// ------------------------------------------------------------------
// Detección y parseo de tokens
// ------------------------------------------------------------------

/// `true` si `s` luce como un token cifrado por este módulo.
pub fn is_encrypted_token(s: &str) -> bool { s.starts_with(ENC_PREFIX) }

/// Decodifica un token `$enc$v$...` a (nonce, ciphertext_with_tag).
/// Devuelve error si el prefijo/versión son inválidos o el base64 está mal.
pub(crate) fn decode_token(token: &str) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
    let stripped = token
        .strip_prefix(ENC_PREFIX)
        .ok_or_else(|| CryptoError::BadPrefix(token.to_string()))?;
    // Formato: `<ver>$<base64>`. Buscamos el PRIMER `$` (el de la versión).
    // El base64 puede terminar en `=` pero `=` ≠ `$`, así que no hay
    // ambigüedad: el primer `$` siempre separa versión de payload.
    let sep = stripped
        .find('$')
        .ok_or_else(|| CryptoError::BadFormat("missing '$' separator".into()))?;
    let ver = &stripped[..sep];
    let payload = &stripped[sep + 1..];
    if ver != SCHEME_VERSION {
        return Err(CryptoError::UnsupportedVersion(ver.to_string()));
    }
    let raw = B64
        .decode(payload.trim())
        .map_err(|e| CryptoError::Base64(e.to_string()))?;
    if raw.len() < crypto::NONCE_LEN + crypto::TAG_LEN {
        return Err(CryptoError::BadFormat(format!(
            "payload too short: {} bytes",
            raw.len()
        )));
    }
    let nonce = raw[..crypto::NONCE_LEN].to_vec();
    let ct = raw[crypto::NONCE_LEN..].to_vec();
    Ok((nonce, ct))
}

/// Codifica `(nonce, ciphertext_with_tag)` a un string `$enc$v$base64(...)`.
/// Útil para tests; producción normalmente va por `encrypt_str`.
pub(crate) fn encode_token(nonce: &[u8], ct: &[u8]) -> String {
    let mut buf = Vec::with_capacity(nonce.len() + ct.len());
    buf.extend_from_slice(nonce);
    buf.extend_from_slice(ct);
    format!("{}{}${}", ENC_PREFIX, SCHEME_VERSION, B64.encode(&buf))
}

/// Permisos de los archivos sensibles en disco (modo 0o600, root-only).
pub fn chmod_owner_only<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path.as_ref())?.permissions();
    perm.set_mode(0o600);
    std::fs::set_permissions(path.as_ref(), perm)
}

/// Asegura que un directorio exista con permisos 0o700 (root-only).
pub fn mkdir_owner_only<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.as_ref())?;
    let mut perm = std::fs::metadata(path.as_ref())?.permissions();
    perm.set_mode(0o700);
    std::fs::set_permissions(path.as_ref(), perm)
}

/// Helper: sanity-check rápido de una clave maestra (no all-zero, no
/// trivial). Llamado por `vault.rs` antes de persistirla.
pub(crate) fn is_strong_key(k: &[u8; 32]) -> bool {
    // No todos ceros
    let nonzero = k.iter().any(|b| *b != 0);
    // No trivial (16 ceros + 16 ceros)
    let half_zero = k[..16].iter().all(|b| *b == 0) || k[16..].iter().all(|b| *b == 0);
    nonzero && !half_zero
}
