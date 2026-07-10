// =============================================================================
// secrets/crypto.rs — AES-256-GCM AEAD con AAD contextual
// =============================================================================
//
// Wrapper ligero sobre `aes-gcm` que:
//   - Genera nonces de 12 bytes con `OsRng` (CSPRNG del SO).
//   - Usa un AAD opcional (recomendado: `vendor|device_idx|field_name`)
//     para detectar reordenamientos/copias entre campos.
//   - Zeroiza los buffers de clave y plaintext con `zeroize::Zeroizing`.
//   - Devuelve tokens ASCII (`$enc$2$<base64>`) seguros para archivos INI.
//
// El esquema es:
//   $enc$2$<base64(nonce(12B) || ciphertext || gcm_tag(16B))>
// =============================================================================

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use rand::{rngs::OsRng, RngCore};
use thiserror::Error;
use zeroize::Zeroizing;

/// Marca visible en disco para distinguir tokens cifrados.
pub const ENC_PREFIX: &str = "$enc$";

/// Versión del esquema. Cualquier bump requiere re-cifrar todo.
pub const SCHEME_VERSION: &str = "2";

/// AES-GCM nonce = 96 bits (12 bytes) — recomendado por NIST SP 800-38D.
pub const NONCE_LEN: usize = 12;

/// GCM tag = 128 bits (16 bytes) — máximo permitido por aes-gcm.
pub const TAG_LEN: usize = 16;

#[derive(Debug, Error)]
pub enum Error {
    #[error("token no comienza con $enc$ (recibido: {0:?})")]
    BadPrefix(String),
    #[error("formato de token inválido: {0}")]
    BadFormat(String),
    #[error("versión de esquema no soportada: {0} (esperada {SCHEME_VERSION})")]
    UnsupportedVersion(String),
    #[error("base64 inválido: {0}")]
    Base64(String),
    #[error("AES-GCM AEAD falló: autenticación/ciphertext inválido (probable tampering)")]
    AeadFailed,
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("error de clave maestra: {0}")]
    MasterKey(String),
}

impl From<aes_gcm::Error> for Error {
    fn from(_: aes_gcm::Error) -> Self { Error::AeadFailed }
}

/// Genera un nonce aleatorio de 12 bytes usando el CSPRNG del SO.
fn fresh_nonce() -> [u8; NONCE_LEN] {
    let mut n = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut n);
    n
}

/// Cifra `plaintext` con la clave maestra y devuelve un token ASCII
/// (`$enc$2$<base64>`) listo para guardar en un archivo INI.
///
/// `aad` (additional authenticated data) es cualquier slice que el llamador
/// quiera ligar criptográficamente al ciphertext (no se almacena, solo se
/// verifica al descifrar). Recomendado: `vendor|device_idx|field_name`.
pub fn encrypt_str(master_key: &[u8; 32], plaintext: &str, aad: &[u8]) -> Result<String, Error> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master_key));
    let nonce_bytes = fresh_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Cifrar dentro de un buffer que se zeroiza al salir del scope.
    let pt = Zeroizing::new(plaintext.as_bytes().to_vec());
    let payload = if aad.is_empty() {
        Payload { msg: &pt, aad: &[] }
    } else {
        Payload { msg: &pt, aad }
    };
    let ct_with_tag = cipher.encrypt(nonce, payload)?;

    // Concatenar nonce || ct_with_tag (este último ya incluye el tag de 16B).
    let mut out = Vec::with_capacity(NONCE_LEN + ct_with_tag.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct_with_tag);

    Ok(format!("{}{}${}", ENC_PREFIX, SCHEME_VERSION, B64.encode(&out)))
}

/// Descifra un token `$enc$2$...`. Falla si el AAD no coincide o si el
/// token fue manipulado. Devuelve el plaintext zeroizado, válido solo dentro
/// del scope de llamada.
pub fn decrypt_str(master_key: &[u8; 32], token: &str, aad: &[u8]) -> Result<Zeroizing<String>, Error> {
    let (nonce_bytes, ct_with_tag) = crate::secrets::decode_token(token)?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master_key));
    let nonce = Nonce::from_slice(&nonce_bytes);

    let payload = if aad.is_empty() {
        Payload { msg: &ct_with_tag, aad: &[] }
    } else {
        Payload { msg: &ct_with_tag, aad }
    };
    let pt = cipher.decrypt(nonce, payload)?;
    // `pt` ya viene como Vec<u8>; lo envolvemos en Zeroizing<String>.
    let s = Zeroizing::new(String::from_utf8(pt).map_err(|_| {
        Error::BadFormat("plaintext no es UTF-8 válido tras descifrar".into())
    })?);
    Ok(s)
}

/// Helper de compatibilidad: si `value` empieza con `$enc$` lo descifra, si
/// no, lo devuelve tal cual (modo v2.4.0 — texto plano). Nunca falla por
/// "no es un token" — solo por errores criptográficos reales.
pub fn decrypt_optional(
    master_key: &[u8; 32],
    value: &str,
    aad: &[u8],
) -> Result<Zeroizing<String>, Error> {
    if crate::secrets::is_encrypted_token(value) {
        decrypt_str(master_key, value, aad)
    } else {
        Ok(Zeroizing::new(value.to_string()))
    }
}

// =============================================================================
// Tests unitarios
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::encode_token;

    // Clave de test fija y bien formada.
    fn test_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() { *b = (i as u8).wrapping_mul(7).wrapping_add(3); }
        k
    }

    #[test]
    fn roundtrip_with_aad() {
        let k = test_key();
        let aad = b"fortinet_1|v3_auth_password";
        let pt = "Emanuel0125*";
        let token = encrypt_str(&k, pt, aad).unwrap();
        assert!(token.starts_with("$enc$2$"));
        let dec = decrypt_str(&k, &token, aad).unwrap();
        assert_eq!(*dec, pt);
    }

    #[test]
    fn roundtrip_empty_aad() {
        let k = test_key();
        let pt = "MyS3cret!";
        let token = encrypt_str(&k, pt, b"").unwrap();
        let dec = decrypt_str(&k, &token, b"").unwrap();
        assert_eq!(*dec, pt);
    }

    #[test]
    fn aad_mismatch_fails() {
        let k = test_key();
        let token = encrypt_str(&k, "secret", b"context_A").unwrap();
        let err = decrypt_str(&k, &token, b"context_B").unwrap_err();
        assert!(matches!(err, Error::AeadFailed), "got {err:?}");
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let k = test_key();
        let mut token = encrypt_str(&k, "secret", b"ctx").unwrap();
        // Modificar un carácter del payload base64 (es un "X" válido).
        let last = token.pop().unwrap();
        token.push(if last == 'A' { 'B' } else { 'A' });
        let err = decrypt_str(&k, &token, b"ctx").unwrap_err();
        // Puede ser Base64 o AeadFailed — cualquiera es válido.
        assert!(matches!(err, Error::Base64(_) | Error::AeadFailed), "got {err:?}");
    }

    #[test]
    fn decoded_token_too_short() {
        // Construir un token con base64 de 10 bytes (menos de NONCE+TAG).
        // El token debe verse como `$enc$2$<base64>` (un solo `$` separador
        // entre versión y payload).
        let token = format!("{}{}${}", ENC_PREFIX, SCHEME_VERSION, B64.encode([0u8; 10]));
        let err = crate::secrets::decode_token(&token).unwrap_err();
        assert!(matches!(err, Error::BadFormat(_)), "got {err:?}");
    }

    #[test]
    fn bad_prefix_fails() {
        let err = crate::secrets::decode_token("plain string").unwrap_err();
        assert!(matches!(err, Error::BadPrefix(_)), "got {err:?}");
    }

    #[test]
    fn decrypt_optional_passthrough_plain() {
        let k = test_key();
        let v = "Emanuel0125*";
        let dec = decrypt_optional(&k, v, b"any").unwrap();
        assert_eq!(*dec, v);
    }

    #[test]
    fn decrypt_optional_with_encrypted() {
        let k = test_key();
        let token = encrypt_str(&k, "p", b"x").unwrap();
        let dec = decrypt_optional(&k, &token, b"x").unwrap();
        assert_eq!(*dec, "p");
    }

    #[test]
    fn wrong_version_fails() {
        // Construir un token con versión "1" (no soportada).
        let token = format!("{}$1${}", ENC_PREFIX, B64.encode([0u8; 40]));
        let err = crate::secrets::decode_token(&token).unwrap_err();
        assert!(matches!(err, Error::UnsupportedVersion(_)), "got {err:?}");
    }

    #[test]
    fn encode_decode_roundtrip() {
        let nonce = [1u8; NONCE_LEN];
        let ct = vec![2u8; 32];
        let token = encode_token(&nonce, &ct);
        let (n2, c2) = crate::secrets::decode_token(&token).unwrap();
        assert_eq!(n2, nonce);
        assert_eq!(c2, ct);
    }

    #[test]
    fn two_nonces_differ() {
        // Probabilistic — si OsRng está roto, lo veríamos como no-determinismo.
        let n1 = fresh_nonce();
        let n2 = fresh_nonce();
        assert_ne!(n1, n2);
    }
}
