// =============================================================================
// secrets/env_file.rs — Reemplazo cifrado de /etc/ness_relay/secrets.env
// =============================================================================
//
// Antes (v2.4.0):
//   /etc/ness_relay/secrets.env   # export NESS_SSH_PASSWORD_xxx='plaintext'
//
// Ahora (v2.5.0):
//   /etc/ness_relay/secrets.enc   # binario con todas las NESS_SSH_PASSWORD_*
//                                # cifradas con AES-256-GCM, salt del host.
//
// Formato binario (little-endian, longitud-prefixed):
//   magic:    4 bytes  = b"NRSE"  (NESS Relay Secrets Encrypted)
//   version:  u8       = 0x01
//   reserved: 3 bytes  = 0x00 0x00 0x00
//   count:    u32 LE   = número de pares
//   para cada par (count veces):
//     key_len:   u16 LE
//     key:       key_len bytes (UTF-8, ej: "NESS_SSH_PASSWORD_FORTINET_1")
//     nonce_len: u8   (= 12)
//     nonce:     12 bytes
//     ct_len:    u32 LE
//     ct:        ct_len bytes (ciphertext + GCM tag, 16B)
//     aad_len:   u16 LE
//     aad:       aad_len bytes (AAD usado al cifrar, recomendado:
//                "env-var|<nombre>" o solo "env-var")
//
// El AAD se serializa junto al ciphertext para poder verificar al descifrar
// que nadie manipuló la asociación (key, valor).
// =============================================================================

use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use thiserror::Error;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::secrets::crypto::{Error as CryptoError, NONCE_LEN};

pub const MAGIC: &[u8; 4] = b"NRSE";
pub const FORMAT_VERSION: u8 = 0x01;

#[derive(Debug, Error)]
pub enum EnvFileError {
    #[error("archivo secrets.enc no encontrado: {0}")]
    NotFound(String),
    #[error("magic bytes inválidos (no es un secrets.enc de NESS Relay)")]
    BadMagic,
    #[error("versión de formato no soportada: {0} (esperada {FORMAT_VERSION})")]
    UnsupportedVersion(u8),
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("formato corrupto: {0}")]
    Corrupt(String),
    #[error("error criptográfico: {0}")]
    Crypto(String),
}

impl From<CryptoError> for EnvFileError {
    fn from(e: CryptoError) -> Self { EnvFileError::Crypto(e.to_string()) }
}

impl From<EnvFileError> for CryptoError {
    fn from(e: EnvFileError) -> Self { CryptoError::MasterKey(e.to_string()) }
}

/// Carga y descifra el archivo `secrets.enc` y devuelve un `HashMap`
/// `env-var -> plaintext`. Los valores son `String` normales (no Zeroizing)
/// porque su ciclo de vida lo controla el caller.
pub fn load_env_file_decrypted<P: AsRef<Path>>(
    master_key: &[u8; 32],
    path: P,
) -> Result<HashMap<String, String>, EnvFileError> {
    let bytes = match fs::read(path.as_ref()) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HashMap::new());
        }
        Err(e) => return Err(EnvFileError::Io(e)),
    };
    if bytes.is_empty() {
        return Ok(HashMap::new());
    }
    parse_env_file(master_key, &bytes)
}

/// Guarda un mapa `env-var -> plaintext` cifrado en disco (atómico: escribe
/// a `.tmp` y luego `rename`). Aplica `chmod 600`.
pub fn save_env_file_encrypted<P: AsRef<Path>>(
    master_key: &[u8; 32],
    path: P,
    entries: &HashMap<String, String>,
) -> Result<(), EnvFileError> {
    let mut buf = Vec::with_capacity(64 + entries.len() * 96);
    // Header
    buf.extend_from_slice(MAGIC);
    buf.push(FORMAT_VERSION);
    buf.extend_from_slice(&[0u8; 3]);
    buf.write_u32::<LittleEndian>(entries.len() as u32)?;

    for (k, v) in entries {
        // AAD = "env-file|<key>" para ligar la entrada a su nombre.
        let aad = format!("env-file|{k}");
        let token = crate::secrets::crypto::encrypt_str(master_key, v, aad.as_bytes())?;
        let (nonce, ct) = crate::secrets::decode_token(&token)?;

        // key
        let key_bytes = k.as_bytes();
        buf.write_u16::<LittleEndian>(key_bytes.len() as u16)?;
        buf.extend_from_slice(key_bytes);

        // nonce
        assert_eq!(nonce.len(), NONCE_LEN);
        buf.push(NONCE_LEN as u8);
        buf.extend_from_slice(&nonce);

        // ct (incluye tag)
        buf.write_u32::<LittleEndian>(ct.len() as u32)?;
        buf.extend_from_slice(&ct);

        // aad
        let aad_bytes = aad.as_bytes();
        buf.write_u16::<LittleEndian>(aad_bytes.len() as u16)?;
        buf.extend_from_slice(aad_bytes);
    }

    // Escritura atómica
    let final_path = path.as_ref();
    if let Some(parent) = final_path.parent() {
        crate::secrets::mkdir_owner_only(parent).ok();
    }
    let tmp_path = final_path.with_extension("enc.tmp");
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(&buf)?;
        f.sync_all().ok();
    }
    crate::secrets::chmod_owner_only(&tmp_path).ok();
    fs::rename(&tmp_path, final_path)?;
    crate::secrets::chmod_owner_only(final_path).ok();
    Ok(())
}

fn parse_env_file(master_key: &[u8; 32], bytes: &[u8]) -> Result<HashMap<String, String>, EnvFileError> {
    let mut cur = Cursor::new(bytes);

    // Magic
    let mut magic = [0u8; 4];
    cur.read_exact(&mut magic)?;
    if &magic != MAGIC { return Err(EnvFileError::BadMagic); }

    // Version
    let mut ver = [0u8; 1];
    cur.read_exact(&mut ver)?;
    if ver[0] != FORMAT_VERSION {
        return Err(EnvFileError::UnsupportedVersion(ver[0]));
    }

    // Reserved
    let mut reserved = [0u8; 3];
    cur.read_exact(&mut reserved)?;

    // Count
    let count = cur.read_u32::<LittleEndian>()?;

    let mut out = HashMap::with_capacity(count as usize);
    for i in 0..count {
        let key_len = cur.read_u16::<LittleEndian>()? as usize;
        let mut key_bytes = vec![0u8; key_len];
        cur.read_exact(&mut key_bytes)?;
        let key = String::from_utf8(key_bytes)
            .map_err(|_| EnvFileError::Corrupt(format!("entry #{i}: key no UTF-8")))?;

        let mut nonce_len_b = [0u8; 1];
        cur.read_exact(&mut nonce_len_b)?;
        let nonce_len = nonce_len_b[0] as usize;
        if nonce_len != NONCE_LEN {
            return Err(EnvFileError::Corrupt(format!(
                "entry #{i}: nonce_len={nonce_len}, esperado {NONCE_LEN}"
            )));
        }
        let mut nonce = vec![0u8; nonce_len];
        cur.read_exact(&mut nonce)?;

        let ct_len = cur.read_u32::<LittleEndian>()? as usize;
        let mut ct = vec![0u8; ct_len];
        cur.read_exact(&mut ct)?;

        let aad_len = cur.read_u16::<LittleEndian>()? as usize;
        let mut aad_bytes = vec![0u8; aad_len];
        cur.read_exact(&mut aad_bytes)?;
        let aad = String::from_utf8(aad_bytes)
            .map_err(|_| EnvFileError::Corrupt(format!("entry #{i}: aad no UTF-8")))?;

        // Reconstruir token y descifrar.
        let token = crate::secrets::encode_token(&nonce, &ct);
        let plain = crate::secrets::crypto::decrypt_str(master_key, &token, aad.as_bytes())?;
        out.insert(key, plain.to_string());
    }
    Ok(out)
}

// (Los traits `ReadBytesExt` y `WriteBytesExt` del crate `byteorder` se usan
// directamente sobre `Cursor` y `Vec`. No se requiere shim inline.)

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() { *b = (i as u8).wrapping_mul(11).wrapping_add(5); }
        k
    }

    #[test]
    fn roundtrip_in_memory() {
        let k = test_key();
        let mut entries: HashMap<String, String> = HashMap::new();
        entries.insert("NESS_SSH_PASSWORD_FORTINET_1".to_string(), "Emanuel0125*".to_string());
        entries.insert("NESS_SSH_PASSWORD_MIKROTIK_1".to_string(), "OtraPass!23".to_string());

        // Serializar a bytes manualmente (sin disco).
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.push(FORMAT_VERSION);
        buf.extend_from_slice(&[0u8; 3]);
        buf.write_u32::<LittleEndian>(entries.len() as u32).unwrap();

        for (k_name, v) in &entries {
            let aad = format!("env-file|{k_name}");
            let token = crate::secrets::crypto::encrypt_str(&k, v, aad.as_bytes()).unwrap();
            let (nonce, ct) = crate::secrets::decode_token(&token).unwrap();

            let kb = k_name.as_bytes();
            buf.write_u16::<LittleEndian>(kb.len() as u16).unwrap();
            buf.extend_from_slice(kb);
            buf.push(NONCE_LEN as u8);
            buf.extend_from_slice(&nonce);
            buf.write_u32::<LittleEndian>(ct.len() as u32).unwrap();
            buf.extend_from_slice(&ct);
            let ab = aad.as_bytes();
            buf.write_u16::<LittleEndian>(ab.len() as u16).unwrap();
            buf.extend_from_slice(ab);
        }

        let parsed = parse_env_file(&k, &buf).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.get("NESS_SSH_PASSWORD_FORTINET_1").unwrap(), "Emanuel0125*");
        assert_eq!(parsed.get("NESS_SSH_PASSWORD_MIKROTIK_1").unwrap(), "OtraPass!23");
    }

    #[test]
    fn bad_magic_rejected() {
        let k = test_key();
        let mut bytes = vec![0u8; 4];
        bytes.copy_from_slice(b"XXXX");
        bytes.push(FORMAT_VERSION);
        let err = parse_env_file(&k, &bytes).unwrap_err();
        assert!(matches!(err, EnvFileError::BadMagic), "got {err:?}");
    }

    #[test]
    fn unsupported_version_rejected() {
        let k = test_key();
        let mut bytes = vec![0u8; 4];
        bytes.copy_from_slice(MAGIC);
        bytes.push(0xFF); // version inválida
        bytes.extend_from_slice(&[0u8; 3]);
        let err = parse_env_file(&k, &bytes).unwrap_err();
        assert!(matches!(err, EnvFileError::UnsupportedVersion(0xFF)), "got {err:?}");
    }

    #[test]
    fn tampered_ct_rejected() {
        // Roundtrip + flip 1 byte del ciphertext.
        let k = test_key();
        let mut entries: HashMap<String, String> = HashMap::new();
        entries.insert("X".to_string(), "secret".to_string());

        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.push(FORMAT_VERSION);
        buf.extend_from_slice(&[0u8; 3]);
        buf.write_u32::<LittleEndian>(1).unwrap();

        let aad = "env-file|X";
        let token = crate::secrets::crypto::encrypt_str(&k, "secret", aad.as_bytes()).unwrap();
        let (nonce, mut ct) = crate::secrets::decode_token(&token).unwrap();
        if let Some(b) = ct.last_mut() { *b ^= 0xFF; }

        buf.write_u16::<LittleEndian>(1).unwrap();
        buf.push(b'X');
        buf.push(NONCE_LEN as u8);
        buf.extend_from_slice(&nonce);
        buf.write_u32::<LittleEndian>(ct.len() as u32).unwrap();
        buf.extend_from_slice(&ct);
        buf.write_u16::<LittleEndian>(aad.len() as u16).unwrap();
        buf.extend_from_slice(aad.as_bytes());

        let err = parse_env_file(&k, &buf).unwrap_err();
        assert!(matches!(err, EnvFileError::Crypto(_)), "got {err:?}");
    }
}
