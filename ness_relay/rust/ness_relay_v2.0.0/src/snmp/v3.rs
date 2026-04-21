// ==============================================================================
// NESS Relay v2.0.0 - SNMPv3 USM Security Model
// ==============================================================================
// Implementación completa del User-based Security Model (USM) para SNMPv3.
// Soporta autenticación y privacidad usando RustCrypto (100% Rust, sin OpenSSL).
//
// Referencia: RFC 3414 (USM for SNMPv3), RFC 3826 (AES for SNMPv3)
//
// Protocolos de Autenticación:
//   - HMAC-MD5-96  (legacy, compatible)
//   - HMAC-SHA-96  (SHA1, estándar SNMPv3)
//   - HMAC-SHA-256 (SHA2, moderno)
//
// Protocolos de Privacidad:
//   - DES-CBC      (legacy, compatible)
//   - AES-128-CFB  (estándar moderno, RFC 3826)
// ==============================================================================

use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use md5::Md5;
use sha2::{Sha256, Sha384, Sha512};

// ==============================================================================
// TIPOS DE PROTOCOLO
// ==============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum AuthProtocol {
    None,
    Md5,
    Sha1,
    Sha256,
    Sha256_192,
    Sha384,
    Sha512,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrivProtocol {
    None,
    Des,
    Aes128,
    Aes192,
    Aes256,
}

impl AuthProtocol {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "MD5" => Self::Md5,
            "SHA" | "SHA1" | "SHA-1" | "SHA96" | "HMACSHA96" => Self::Sha1,
            "SHA256AUTH" | "SHA256-192" | "SHA-256-192" | "HMAC-SHA2-256-192" => {
                Self::Sha256_192
            }
            "SHA256"
            | "SHA-256"
            | "SHA2-256"
            | "SHA256-128"
            | "HMACSHA256"
            | "HMAC-SHA2-256-128" => Self::Sha256,
            "SHA384" | "SHA-384" | "SHA2-384" | "HMAC-SHA2-384-192" => Self::Sha384,
            "SHA512" | "SHA-512" | "SHA2-512" | "HMAC-SHA2-512-256" => Self::Sha512,
            _ => Self::None,
        }
    }

    pub fn digest_length(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Md5 => 12,    // HMAC-MD5-96: 12 bytes
            Self::Sha1 => 12,   // HMAC-SHA-96: 12 bytes
            Self::Sha256 => 16,     // HMAC-SHA2-256-128: 16 bytes
            Self::Sha256_192 => 24, // SHA256AUTH: 24 bytes (compat legacy)
            Self::Sha384 => 24,     // HMAC-SHA2-384-192: 24 bytes
            Self::Sha512 => 32,     // HMAC-SHA2-512-256: 32 bytes
        }
    }

    pub fn key_length(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Md5 => 16,   // MD5 = 16 bytes
            Self::Sha1 => 20,  // SHA1 = 20 bytes
            Self::Sha256 => 32,     // SHA256 = 32 bytes
            Self::Sha256_192 => 32, // SHA256 = 32 bytes
            Self::Sha384 => 48,     // SHA384 = 48 bytes
            Self::Sha512 => 64,     // SHA512 = 64 bytes
        }
    }
}

impl PrivProtocol {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "DES" | "DES-CBC" | "CBC-DES" => Self::Des,
            "AES" | "AES128" | "AES-128" | "AES-128-CFB" => Self::Aes128,
            "AES192" | "AES-192" | "AES-192-CFB" => Self::Aes192,
            "AES256" | "AES-256" | "AES-256-CFB" => Self::Aes256,
            _ => Self::None,
        }
    }

    pub fn key_length(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Des => 16,   // 8B key + 8B pre-IV
            Self::Aes128 => 16,
            Self::Aes192 => 24,
            Self::Aes256 => 32,
        }
    }
}

// ==============================================================================
// DERIVACIÓN DE CLAVE DE CONTRASEÑA (RFC 3414 §2.6)
// ==============================================================================

/// Convierte una contraseña a clave localizada para SNMPv3 USM.
/// Implementación según RFC 3414, Appendix A.
pub fn password_to_key(password: &str, engine_id: &[u8], proto: &AuthProtocol) -> Vec<u8> {
    let password_bytes = password.as_bytes();
    if password_bytes.is_empty() || proto == &AuthProtocol::None {
        return Vec::new();
    }

    // Paso 1: Generar 1MB de datos repitiendo la contraseña
    const MIN_BYTES: usize = 1_048_576; // 1MB
    let mut extended = Vec::with_capacity(MIN_BYTES);
    let mut written = 0;
    while written < MIN_BYTES {
        let chunk = &password_bytes[..std::cmp::min(password_bytes.len(), MIN_BYTES - written)];
        extended.extend_from_slice(chunk);
        written += chunk.len();
    }

    // Paso 2: Hashear el MB de datos
    let key = match proto {
        AuthProtocol::Md5 => {
            use md5::Digest;
            let mut hasher = Md5::new();
            hasher.update(&extended[..MIN_BYTES]);
            hasher.finalize().to_vec()
        }
        AuthProtocol::Sha1 => {
            use sha1::Digest;
            let mut hasher = sha1::Sha1::new();
            hasher.update(&extended[..MIN_BYTES]);
            hasher.finalize().to_vec()
        }
        AuthProtocol::Sha256 | AuthProtocol::Sha256_192 => {
            use sha2::Digest;
            let mut hasher = Sha256::new();
            hasher.update(&extended[..MIN_BYTES]);
            hasher.finalize().to_vec()
        }
        AuthProtocol::Sha384 => {
            use sha2::Digest;
            let mut hasher = Sha384::new();
            hasher.update(&extended[..MIN_BYTES]);
            hasher.finalize().to_vec()
        }
        AuthProtocol::Sha512 => {
            use sha2::Digest;
            let mut hasher = Sha512::new();
            hasher.update(&extended[..MIN_BYTES]);
            hasher.finalize().to_vec()
        }
        AuthProtocol::None => return Vec::new(),
    };

    // Paso 3: Localizar la clave con el engineID
    localize_key(&key, engine_id, proto)
}

/// Localiza una clave maestra con el engineID del agente (RFC 3414 §2.6).
fn localize_key(key: &[u8], engine_id: &[u8], proto: &AuthProtocol) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(key);
    data.extend_from_slice(engine_id);
    data.extend_from_slice(key);

    match proto {
        AuthProtocol::Md5 => {
            use md5::Digest;
            Md5::digest(&data).to_vec()
        }
        AuthProtocol::Sha1 => {
            use sha1::Digest;
            sha1::Sha1::digest(&data).to_vec()
        }
        AuthProtocol::Sha256 | AuthProtocol::Sha256_192 => {
            use sha2::Digest;
            Sha256::digest(&data).to_vec()
        }
        AuthProtocol::Sha384 => {
            use sha2::Digest;
            Sha384::digest(&data).to_vec()
        }
        AuthProtocol::Sha512 => {
            use sha2::Digest;
            Sha512::digest(&data).to_vec()
        }
        AuthProtocol::None => Vec::new(),
    }
}

/// Deriva y ajusta la clave de privacidad según el protocolo seleccionado.
/// Para AES-192/AES-256 se extiende de forma determinística cuando la clave
/// localizada base no alcanza el largo requerido.
pub fn derive_priv_key(
    password: &str,
    engine_id: &[u8],
    auth_proto: &AuthProtocol,
    priv_proto: &PrivProtocol,
) -> Vec<u8> {
    let required = priv_proto.key_length();
    if required == 0 {
        return Vec::new();
    }

    let mut key = password_to_key(password, engine_id, auth_proto);
    if key.is_empty() {
        return key;
    }

    while key.len() < required {
        let extension = localize_key(&key, engine_id, auth_proto);
        if extension.is_empty() {
            break;
        }
        key.extend_from_slice(&extension);
    }

    key.truncate(required);
    key
}

// ==============================================================================
// AUTENTICACIÓN USM (RFC 3414 §7.3)
// ==============================================================================

/// Calcula el HMAC de autenticación para un mensaje SNMPv3.
/// Modifica en lugar el campo msgAuthenticationParameters antes de calcular.
pub fn authenticate_message(
    message: &[u8],
    auth_key: &[u8],
    proto: &AuthProtocol,
) -> Result<Vec<u8>> {
    let tag = match proto {
        AuthProtocol::Md5 => {
            type HmacMd5 = Hmac<Md5>;
            let mut mac = HmacMd5::new_from_slice(auth_key)
                .map_err(|e| anyhow!("HMAC-MD5 key error: {}", e))?;
            mac.update(message);
            mac.finalize().into_bytes().to_vec()
        }
        AuthProtocol::Sha1 => {
            type HmacSha1 = Hmac<sha1::Sha1>;
            let mut mac = HmacSha1::new_from_slice(auth_key)
                .map_err(|e| anyhow!("HMAC-SHA1 key error: {}", e))?;
            mac.update(message);
            mac.finalize().into_bytes().to_vec()
        }
        AuthProtocol::Sha256 | AuthProtocol::Sha256_192 => {
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(auth_key)
                .map_err(|e| anyhow!("HMAC-SHA256 key error: {}", e))?;
            mac.update(message);
            mac.finalize().into_bytes().to_vec()
        }
        AuthProtocol::Sha384 => {
            type HmacSha384 = Hmac<Sha384>;
            let mut mac = HmacSha384::new_from_slice(auth_key)
                .map_err(|e| anyhow!("HMAC-SHA384 key error: {}", e))?;
            mac.update(message);
            mac.finalize().into_bytes().to_vec()
        }
        AuthProtocol::Sha512 => {
            type HmacSha512 = Hmac<Sha512>;
            let mut mac = HmacSha512::new_from_slice(auth_key)
                .map_err(|e| anyhow!("HMAC-SHA512 key error: {}", e))?;
            mac.update(message);
            mac.finalize().into_bytes().to_vec()
        }
        AuthProtocol::None => return Err(anyhow!("No auth protocol configured")),
    };
    // Retornar solo los primeros N bytes según el protocolo
    let digest_len = proto.digest_length();
    Ok(tag[..digest_len.min(tag.len())].to_vec())
}

// ==============================================================================
// PRIVACIDAD USM - AES-128-CFB (RFC 3826)
// ==============================================================================

// ===========================================================================
// AES-128-CFB128 (RFC 3826 §3.1) — implementación manual sin cfb-mode crate
// CFB usa AES-encrypt en ambas direcciones (enc y dec).
// ===========================================================================

fn aes_cfb128_encrypt_with_cipher<C>(key: &[u8], iv: &[u8; 16], data: &mut [u8]) -> Result<()>
where
    C: aes::cipher::BlockEncrypt
        + aes::cipher::KeyInit
        + aes::cipher::BlockSizeUser<BlockSize = aes::cipher::consts::U16>,
{
    use aes::cipher::generic_array::GenericArray;

    let aes = C::new_from_slice(key).map_err(|_| anyhow!("AES key inválida"))?;
    let mut feedback = *iv;
    let mut offset = 0;
    while offset < data.len() {
        let mut block = GenericArray::from(feedback);
        aes.encrypt_block(&mut block);
        let len = (data.len() - offset).min(16);
        for i in 0..len {
            data[offset + i] ^= block[i];
        }
        if len == 16 {
            feedback.copy_from_slice(&data[offset..offset + 16]);
        }
        offset += len;
    }
    Ok(())
}

fn aes_cfb128_decrypt_with_cipher<C>(key: &[u8], iv: &[u8; 16], data: &mut [u8]) -> Result<()>
where
    C: aes::cipher::BlockEncrypt
        + aes::cipher::KeyInit
        + aes::cipher::BlockSizeUser<BlockSize = aes::cipher::consts::U16>,
{
    use aes::cipher::generic_array::GenericArray;

    let aes = C::new_from_slice(key).map_err(|_| anyhow!("AES key inválida"))?;
    let mut feedback = *iv;
    let mut offset = 0;
    while offset < data.len() {
        let mut block = GenericArray::from(feedback);
        aes.encrypt_block(&mut block); // CFB usa encrypt para descifrar también
        let len = (data.len() - offset).min(16);
        let mut ct_save = [0u8; 16];
        ct_save[..len].copy_from_slice(&data[offset..offset + len]);
        for i in 0..len {
            data[offset + i] ^= block[i];
        }
        if len == 16 {
            feedback.copy_from_slice(&ct_save);
        }
        offset += len;
    }
    Ok(())
}

fn build_aes_iv(engine_boots: u32, engine_time: u32, priv_param: &[u8]) -> Result<[u8; 16]> {
    if priv_param.len() < 8 {
        return Err(anyhow!("AES IV param too short"));
    }

    // IV: engineBoots(4) || engineTime(4) || salt(8)  (RFC 3826 §3.1.2.1)
    let mut iv = [0u8; 16];
    iv[0..4].copy_from_slice(&engine_boots.to_be_bytes());
    iv[4..8].copy_from_slice(&engine_time.to_be_bytes());
    iv[8..16].copy_from_slice(&priv_param[..8]);
    Ok(iv)
}

fn encrypt_aes_cfb(
    priv_key: &[u8],
    key_len: usize,
    engine_boots: u32,
    engine_time: u32,
    data: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    if priv_key.len() < key_len {
        return Err(anyhow!("AES key too short (need {} bytes)", key_len));
    }

    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut salt = [0u8; 8];
    rng.fill_bytes(&mut salt);

    let iv = build_aes_iv(engine_boots, engine_time, &salt)?;
    let mut output = data.to_vec();

    match key_len {
        16 => aes_cfb128_encrypt_with_cipher::<aes::Aes128>(&priv_key[..16], &iv, &mut output)?,
        24 => aes_cfb128_encrypt_with_cipher::<aes::Aes192>(&priv_key[..24], &iv, &mut output)?,
        32 => aes_cfb128_encrypt_with_cipher::<aes::Aes256>(&priv_key[..32], &iv, &mut output)?,
        _ => return Err(anyhow!("AES key length no soportada: {}", key_len)),
    }

    Ok((output, salt.to_vec()))
}

fn decrypt_aes_cfb(
    priv_key: &[u8],
    key_len: usize,
    engine_boots: u32,
    engine_time: u32,
    priv_param: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    if priv_key.len() < key_len {
        return Err(anyhow!("AES key too short (need {} bytes)", key_len));
    }

    let iv = build_aes_iv(engine_boots, engine_time, priv_param)?;
    let mut output = ciphertext.to_vec();

    match key_len {
        16 => aes_cfb128_decrypt_with_cipher::<aes::Aes128>(&priv_key[..16], &iv, &mut output)?,
        24 => aes_cfb128_decrypt_with_cipher::<aes::Aes192>(&priv_key[..24], &iv, &mut output)?,
        32 => aes_cfb128_decrypt_with_cipher::<aes::Aes256>(&priv_key[..32], &iv, &mut output)?,
        _ => return Err(anyhow!("AES key length no soportada: {}", key_len)),
    }

    Ok(output)
}

/// Cifra datos usando AES-128-CFB para SNMPv3 privacidad (RFC 3826).
/// IV = engineBoots(4 BE) || engineTime(4 BE) || salt(8)
pub fn encrypt_aes128(
    priv_key: &[u8],
    engine_boots: u32,
    engine_time: u32,
    data: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    encrypt_aes_cfb(priv_key, 16, engine_boots, engine_time, data)
}

/// Cifra datos usando AES-192-CFB para SNMPv3 privacidad.
pub fn encrypt_aes192(
    priv_key: &[u8],
    engine_boots: u32,
    engine_time: u32,
    data: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    encrypt_aes_cfb(priv_key, 24, engine_boots, engine_time, data)
}

/// Cifra datos usando AES-256-CFB para SNMPv3 privacidad.
pub fn encrypt_aes256(
    priv_key: &[u8],
    engine_boots: u32,
    engine_time: u32,
    data: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    encrypt_aes_cfb(priv_key, 32, engine_boots, engine_time, data)
}

/// Descifra datos AES-128-CFB de SNMPv3.
/// IV = engineBoots(4 BE) || engineTime(4 BE) || salt(8)
pub fn decrypt_aes128(
    priv_key: &[u8],
    engine_boots: u32,
    engine_time: u32,
    priv_param: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    decrypt_aes_cfb(priv_key, 16, engine_boots, engine_time, priv_param, ciphertext)
}

/// Descifra datos AES-192-CFB de SNMPv3.
pub fn decrypt_aes192(
    priv_key: &[u8],
    engine_boots: u32,
    engine_time: u32,
    priv_param: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    decrypt_aes_cfb(priv_key, 24, engine_boots, engine_time, priv_param, ciphertext)
}

/// Descifra datos AES-256-CFB de SNMPv3.
pub fn decrypt_aes256(
    priv_key: &[u8],
    engine_boots: u32,
    engine_time: u32,
    priv_param: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    decrypt_aes_cfb(priv_key, 32, engine_boots, engine_time, priv_param, ciphertext)
}

// ==============================================================================
// PRIVACIDAD USM - DES-CBC (RFC 3414 §8)
// ==============================================================================

/// Cifra datos con DES-CBC para SNMPv3 privacidad (legacy).
pub fn encrypt_des(
    priv_key: &[u8],
    engine_boots: u32,
    data: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    use cbc::Encryptor as CbcEncryptor;
    use cipher::{BlockEncryptMut, KeyIvInit};
    use des::Des;
    type DesCbc = CbcEncryptor<Des>;

    let key = &priv_key[..8.min(priv_key.len())];
    if key.len() < 8 {
        return Err(anyhow!("DES key too short"));
    }

    // Salt = preIV XOR engineBoots+engineTime
    let pre_iv = &priv_key[8..16.min(priv_key.len())];
    let mut salt = [0u8; 8];
    if pre_iv.len() >= 8 {
        salt.copy_from_slice(&pre_iv[..8]);
    }
    // XOR con boots (4 bytes) + random (4 bytes)
    let boots_bytes = engine_boots.to_be_bytes();
    for i in 0..4 {
        salt[i] ^= boots_bytes[i];
    }

    // DES requires 8-byte blocks — pad data
    let padded_len = ((data.len() + 7) / 8) * 8;
    let mut padded = vec![0u8; padded_len];
    padded[..data.len()].copy_from_slice(data);

    let mut key_arr = [0u8; 8];
    key_arr.copy_from_slice(&key[..8]);

    let mut cipher = DesCbc::new(&key_arr.into(), &salt.into());
    // encrypt_padded_vec_mut con NoPadding retorna Vec<u8> directamente
    let ciphertext = cipher.encrypt_padded_vec_mut::<cipher::block_padding::NoPadding>(&padded);

    Ok((ciphertext, salt.to_vec()))
}

/// Descifra datos DES-CBC de SNMPv3.
pub fn decrypt_des(
    priv_key: &[u8],
    priv_param: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    use cbc::Decryptor as CbcDecryptor;
    use cipher::{BlockDecryptMut, KeyIvInit};
    use des::Des;
    type DesCbcDec = CbcDecryptor<Des>;

    let key = &priv_key[..8.min(priv_key.len())];
    if key.len() < 8 || priv_param.len() < 8 {
        return Err(anyhow!("DES decrypt: invalid key or IV length"));
    }

    let mut key_arr = [0u8; 8];
    key_arr.copy_from_slice(&key[..8]);
    let mut iv_arr = [0u8; 8];
    iv_arr.copy_from_slice(&priv_param[..8]);

    let cipher = DesCbcDec::new(&key_arr.into(), &iv_arr.into());
    let plaintext = cipher
        .decrypt_padded_vec_mut::<cipher::block_padding::NoPadding>(ciphertext)
        .map_err(|e| anyhow!("DES decrypt error: {:?}", e))?;

    Ok(plaintext)
}

// ==============================================================================
// INFORMACIÓN DE SEGURIDAD USM
// ==============================================================================

/// Parámetros de seguridad USM para un usuario SNMPv3.
#[derive(Debug, Clone)]
pub struct UsmSecurityParams {
    pub username: String,
    pub auth_protocol: AuthProtocol,
    pub auth_key_localized: Vec<u8>,
    pub priv_protocol: PrivProtocol,
    pub priv_key_localized: Vec<u8>,
    pub engine_id: Vec<u8>,
    pub engine_boots: u32,
    pub engine_time: u32,
}

impl UsmSecurityParams {
    /// Crea parámetros USM desde la configuración del dispositivo.
    pub fn new(
        username: &str,
        auth_proto: AuthProtocol,
        auth_password: &str,
        priv_proto: PrivProtocol,
        priv_password: &str,
        engine_id: &[u8],
        engine_boots: u32,
        engine_time: u32,
    ) -> Self {
        let auth_key = password_to_key(auth_password, engine_id, &auth_proto);
        let priv_key = derive_priv_key(priv_password, engine_id, &auth_proto, &priv_proto);

        Self {
            username: username.to_string(),
            auth_protocol: auth_proto,
            auth_key_localized: auth_key,
            priv_protocol: priv_proto,
            priv_key_localized: priv_key,
            engine_id: engine_id.to_vec(),
            engine_boots,
            engine_time,
        }
    }
}
