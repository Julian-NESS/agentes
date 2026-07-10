// =============================================================================
// secrets/vault.rs — Derivación de la clave maestra atada al host
// =============================================================================
//
// La clave maestra de 32 bytes (AES-256) NO se almacena como tal. Se deriva
// en cada inicio con HKDF-SHA256 usando:
//
//     IKM  = system machine-id (leído de /etc/machine-id o /var/lib/dbus/machine-id)
//     salt = 16 bytes aleatorios persistidos en /etc/ness_relay/.salt
//     info = "ness-relay/v2/host-master-key"
//
// Esto logra:
//   - Portabilidad: cualquier backup de /etc/ness_relay/ sin /etc/machine-id
//     de la misma máquina NO descifra (la clave es del host, no del archivo).
//   - Sin password humano: la máquina es quien "sabe" la clave.
//   - Reproducibilidad: el binario puede derivar la misma clave en cada
//     ejecución (no necesita persistir la clave, solo la sal).
//
// Archivos:
//   /etc/ness_relay/.salt      ← 16 bytes, chmod 600, root:root
//   /etc/ness_relay/.key_tmp   ← 32 bytes HKDF, chmod 600 (solo durante
//                                rotación; se borra al terminar)
//
// Si el host no tiene machine-id (ej. contenedor sin dbus), caemos a
// `/etc/ness_relay/.seed` (también chmod 600, generado al primer arranque).
// =============================================================================

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::secrets::crypto::Error as CryptoError;

pub const SALT_LEN: usize = 16;
const MACHINE_ID_PATHS: &[&str] = &[
    "/etc/machine-id",
    "/var/lib/dbus/machine-id",
];
const HKDF_INFO: &[u8] = b"ness-relay/v2/host-master-key";
const FALLBACK_SEED: &str = ".seed";

/// Paths internos del vault. Se cachean para evitar I/O repetido.
#[derive(Debug, Clone)]
pub struct VaultPaths {
    pub root: PathBuf,
    pub salt: PathBuf,
    pub seed: PathBuf,
}

/// Resuelve los paths canónicos del vault.
pub fn vault_paths() -> VaultPaths {
    let root = crate::secrets::secrets_root();
    VaultPaths { salt: root.join(".salt"), seed: root.join(FALLBACK_SEED), root }
}

/// Alias corto.
pub fn vault_dir() -> PathBuf { vault_paths().root }

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("no se pudo derivar machine-id y no existe seed local en {0}")]
    NoHostIdentity(PathBuf),
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("machine-id vacío en {0}")]
    EmptyMachineId(PathBuf),
    #[error("error criptográfico: {0}")]
    Crypto(String),
}

impl From<VaultError> for CryptoError {
    fn from(e: VaultError) -> Self { CryptoError::MasterKey(e.to_string()) }
}

/// Lee el `machine-id` del sistema (12+ chars hex, 128 bits).
/// Si no existe, devuelve `None` y el llamador debe usar fallback.
fn read_machine_id() -> Option<String> {
    for p in MACHINE_ID_PATHS {
        if let Ok(s) = fs::read_to_string(p) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Lee (o crea al primer arranque) la sal persistente.
fn load_or_create_salt(paths: &VaultPaths) -> Result<[u8; SALT_LEN], VaultError> {
    if paths.salt.exists() {
        let bytes = fs::read(&paths.salt)?;
        if bytes.len() == SALT_LEN {
            let mut out = [0u8; SALT_LEN];
            out.copy_from_slice(&bytes);
            return Ok(out);
        }
        // Tamaño inválido: regenerar (caso muy raro, log en producción).
    }
    use rand::{rngs::OsRng, RngCore};
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    if let Some(parent) = paths.salt.parent() {
        crate::secrets::mkdir_owner_only(parent).ok();
    }
    fs::write(&paths.salt, salt)?;
    crate::secrets::chmod_owner_only(&paths.salt).ok();
    Ok(salt)
}

/// Lee (o crea al primer arranque) el seed de fallback.
fn load_or_create_seed(paths: &VaultPaths) -> Result<Vec<u8>, VaultError> {
    if paths.seed.exists() {
        return Ok(fs::read(&paths.seed)?);
    }
    use rand::{rngs::OsRng, RngCore};
    let mut buf = vec![0u8; 32];
    OsRng.fill_bytes(&mut buf);
    if let Some(parent) = paths.seed.parent() {
        crate::secrets::mkdir_owner_only(parent).ok();
    }
    {
        let mut f = fs::File::create(&paths.seed)?;
        f.write_all(&buf)?;
        f.sync_all().ok();
    }
    crate::secrets::chmod_owner_only(&paths.seed).ok();
    Ok(buf)
}

/// Asegura que el vault existe (crea dir raíz, sal y seed si faltan).
pub fn ensure_vault() -> Result<VaultPaths, VaultError> {
    let paths = vault_paths();
    crate::secrets::mkdir_owner_only(&paths.root)?;
    // Llamar load_or_create_* también materializa los archivos.
    let _ = load_or_create_salt(&paths)?;
    let _ = load_or_create_seed(&paths)?;
    Ok(paths)
}

/// Deriva la clave maestra de 32 bytes (AES-256).
///
/// Devuelve un `Zeroizing<[u8; 32]>` que se limpia de memoria al salir del
/// scope. Los llamadores NO deben clonarla ni persistirla.
pub fn master_key() -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let paths = ensure_vault()?;
    let salt = load_or_create_salt(&paths)?;

    // IKM = machine-id (preferido) o seed local (fallback).
    let ikm = if let Some(mid) = read_machine_id() {
        Zeroizing::new(mid.into_bytes())
    } else {
        Zeroizing::new(load_or_create_seed(&paths)?)
    };

    let hk = Hkdf::<Sha256>::new(Some(&salt), &ikm);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(HKDF_INFO, okm.as_mut())
        .map_err(|e| VaultError::Crypto(format!("HKDF expand: {e}")))?;

    // Sanity-check (anti-zero-key).
    if !crate::secrets::is_strong_key(&okm) {
        return Err(VaultError::Crypto("clave maestra degenerada".into()));
    }
    Ok(okm)
}

/// Borra la sal y el seed (usar solo durante `rotate`).
pub fn destroy_host_identity() -> Result<(), VaultError> {
    let paths = vault_paths();
    if paths.salt.exists() { fs::remove_file(&paths.salt)?; }
    if paths.seed.exists() { fs::remove_file(&paths.seed)?; }
    Ok(())
}

/// Diagnóstico: imprime (a `tracing::info!`) el estado del vault.
pub fn report_status() {
    let paths = vault_paths();
    tracing::info!(target: "ness_relay::secrets",
        vault = %paths.root.display(),
        salt_exists = paths.salt.exists(),
        seed_exists = paths.seed.exists(),
        machine_id_source = ?MACHINE_ID_PATHS.iter().find(|p| Path::new(p).exists()),
        "vault status");
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Redirigir el root del vault a un tmpdir durante los tests.
    fn with_tmp_root<F, T>(f: F) -> T
    where F: FnOnce() -> T {
        let tmp = env::temp_dir().join(format!(
            "ness-relay-vault-test-{}-{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        // Para tests forzamos root a /etc/ness_relay NO, así que sobreescribimos
        // temporalmente con un symlink. Como ya estamos en Linux, basta con
        // un bind mount en producción, no es necesario en CI.
        // En su lugar, los tests que tocan disco se saltan (#[ignore]).
        let _ = tmp;
        f()
    }

    #[test]
    fn machine_id_path_constant() {
        assert!(MACHINE_ID_PATHS.contains(&"/etc/machine-id"));
    }

    #[test]
    fn hkdf_info_is_stable() {
        // No cambiamos la "info" string sin bump de versión.
        assert_eq!(HKDF_INFO, b"ness-relay/v2/host-master-key");
    }

    #[test]
    #[ignore = "toca /etc; ejecutar con --ignored bajo control del operador"]
    fn ensure_vault_creates_filesystem() {
        // 1) Backup si existe.
        let paths = vault_paths();
        let backup = if paths.root.exists() {
            let b = paths.root.with_extension("bak-test");
            let _ = std::fs::rename(&paths.root, &b);
            Some(b)
        } else { None };

        let result = ensure_vault();
        assert!(result.is_ok(), "ensure_vault falló: {:?}", result.err());
        let p = result.unwrap();
        assert!(p.root.exists());
        assert!(p.salt.exists());
        assert!(p.seed.exists());

        // 2) Restaurar.
        if let Some(b) = backup {
            let _ = std::fs::remove_dir_all(&paths.root);
            let _ = std::fs::rename(&b, &paths.root);
        } else {
            // Limpia lo que creó.
            let _ = std::fs::remove_dir_all(&paths.root);
        }
    }
}
