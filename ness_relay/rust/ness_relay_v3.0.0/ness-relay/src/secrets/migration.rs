// =============================================================================
// secrets/migration.rs — Migración retroactiva plaintext → cifrado
// =============================================================================
//
// Usado por el binario `ness-relay-cred migrate-plaintext` para que los
// operadores con instalaciones v2.4.0 (texto plano) puedan subir a v2.5.0
// sin reinstalar.
//
// Flujo:
//   1. Lee `/opt/ness_relay/configs/connection.config` (o NESS_DEVICES_FILE).
//   2. Por cada campo sensible (v3_auth_password, v3_priv_password, community
//      si el operador quiere), pide por consola la pass (con confirmación).
//   3. Cifra con AAD = `vendor|device_idx|field` y reemplaza el valor.
//   4. Backup del archivo original como `connection.config.bak.YYYYMMDD-HHMMSS`.
//   5. Reporta cuántos campos migró y a cuántos dispositivos.
// =============================================================================

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::secrets::crypto::Error as CryptoError;

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("no se encontró connection.config en {0}")]
    NoConfig(PathBuf),
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("config malformado: {0}")]
    Malformed(String),
    #[error("confirmación no coincide — campo '{0}' sin modificar")]
    Mismatch(String),
    #[error("error criptográfico: {0}")]
    Crypto(String),
    #[error("interrumpido por el usuario (EOF)")]
    Interrupted,
}

impl From<CryptoError> for MigrationError {
    fn from(e: CryptoError) -> Self { MigrationError::Crypto(e.to_string()) }
}

/// Resumen de la migración (para imprimir al operador).
#[derive(Debug, Default, Clone)]
pub struct MigrationReport {
    pub devices_scanned: u32,
    pub fields_migrated: u32,
    pub fields_skipped_existing: u32,
    pub backup_path: Option<PathBuf>,
}

/// Campos sensibles que se migran a `$enc$` cuando están en plano.
/// (community también es sensible, pero SNMPv1/v2c es legacy — el operador
/// puede elegir migrarlo o no).
pub const SENSITIVE_FIELDS: &[&str] = &["v3_auth_password", "v3_priv_password"];

/// Punto de entrada principal. NO imprime nada (esa responsabilidad es del
/// binario CLI). Lee pass por STDIN usando el `prompt_fn` provisto.
///
/// `prompt_fn(field, vendor_idx)` debe devolver `Ok(Some(plaintext))` cuando
/// el usuario tecleó la pass, u `Ok(None)` si presionó Enter (skip).
pub fn migrate_plaintext_config<F, G>(
    master_key: &[u8; 32],
    config_path: &Path,
    backup_dir: Option<&Path>,
    mut prompt_fn: F,
    mut confirm_fn: G,
) -> Result<MigrationReport, MigrationError>
where
    F: FnMut(&str, &str) -> Result<Option<String>, MigrationError>,
    G: FnMut(&str) -> Result<bool, MigrationError>,
{
    if !config_path.exists() {
        return Err(MigrationError::NoConfig(config_path.to_path_buf()));
    }

    // 1) Backup
    let backup_path = write_backup(config_path, backup_dir)?;

    // 2) Parsear línea por línea
    let file = fs::File::open(config_path)?;
    let reader = BufReader::new(file);
    let mut out_lines: Vec<String> = Vec::new();
    let mut current_vendor: Option<String> = None;
    let mut current_idx: Option<String> = None;
    let mut report = MigrationReport { backup_path: Some(backup_path.clone()), ..Default::default() };

    for line_res in reader.lines() {
        let line = line_res?;
        let trimmed = line.trim_start();

        // Detectar bloque [device ...] para conocer el vendor|idx actual.
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // No migramos dentro de secciones especiales ([general], [paths], etc.)
            // porque ahí no hay credenciales. Mantenemos current_* y seguimos.
            current_vendor = None;
            current_idx = None;
            out_lines.push(line);
            continue;
        }

        // Detectar `device = <vendor>_<idx>` o similar.
        if let Some(rest) = trimmed.strip_prefix("device") {
            if let Some(eq) = rest.find('=') {
                let key = rest[..eq].trim();
                let val = rest[eq + 1..].trim();
                if key.is_empty() && !val.is_empty() {
                    // "device = fortinet_1" → vendor=fortinet, idx=1
                    if let Some((v, i)) = val.rsplit_once('_') {
                        current_vendor = Some(v.to_string());
                        current_idx = Some(i.to_string());
                        report.devices_scanned += 1;
                    } else {
                        current_vendor = Some(val.to_string());
                        current_idx = String::new().into();
                    }
                }
            }
        }

        // ¿Es una credencial sensible a migrar?
        if let Some((key_full, raw_value)) = parse_kv_field(&line) {
            // Buscar por sufijo exacto (`<vendor>_<idx>_v3_auth_password`).
            // Comparar el final de la key (después del último `_` de vendor|idx)
            // contra cada `SENSITIVE_FIELDS[i]`. Si la key COMPLETA termina
            // con `_<field>`, es candidata.
            let field_suffix = SENSITIVE_FIELDS
                .iter()
                .find(|f| key_full.len() > f.len() + 1
                    && key_full.as_bytes()[key_full.len() - f.len() - 1] == b'_'
                    && key_full.ends_with(*f))
                .copied()
                .unwrap_or("");
            if !field_suffix.is_empty() {
                // Phase 2.5.8 fix: el AAD debe coincidir EXACTAMENTE con el
                // que usa el agente al descifrar (`<config_key>|<field>`).
                //
                // El agente deriva el AAD a partir del config_key del
                // dispositivo (NO de bloques [device] INI), por lo que
                // debemos derivarlo igual aquí:
                //
                //   <key_full> = "<vendor>_<idx>_<field>"
                //   config_key = "<vendor>_<idx>"   ←  se quita el sufijo `_<field>`
                //   aad        = "<config_key>|<field>"
                //
                // Antes se usaba `current_vendor|current_idx|field` que
                // dependía de un bloque [device] INI inexistente en
                // connection.config plano. Resultado: AAD != AAD-agente,
                // y el descifrado al iniciar el relay fallaba con
                // "AES-GCM AEAD falló: autenticación/ciphertext inválido".
                let config_key = key_full[..key_full.len() - field_suffix.len() - 1].to_string();
                let aad = format!("{config_key}|{field_suffix}");

                // ¿Ya está cifrado?
                if raw_value.starts_with(crate::secrets::ENC_PREFIX) {
                    report.fields_skipped_existing += 1;
                    out_lines.push(line);
                    continue;
                }
                // ¿Está vacío?
                if raw_value.is_empty() {
                    out_lines.push(line);
                    continue;
                }

                // Pedir pass al operador.
                let new_plain = match prompt_fn(field_suffix, &aad)? {
                    Some(p) => p,
                    None => {
                        // Skip intencional: mantener el valor actual (probablemente correcto).
                        out_lines.push(line);
                        continue;
                    }
                };
                // Confirmar.
                if !confirm_fn(field_suffix)? {
                    return Err(MigrationError::Mismatch(field_suffix.to_string()));
                }

                // Cifrar y reescribir.
                let token = crate::secrets::crypto::encrypt_str(master_key, &new_plain, aad.as_bytes())?;
                let new_line = rewrite_field(&line, &token);
                out_lines.push(new_line);
                report.fields_migrated += 1;
                continue;
            }
        }
        out_lines.push(line);
    }

    // 3) Escribir resultado
    let tmp = config_path.with_extension("config.migrating");
    {
        let mut f = fs::File::create(&tmp)?;
        for l in &out_lines { writeln!(f, "{l}")?; }
        f.sync_all().ok();
    }
    crate::secrets::chmod_owner_only(&tmp).ok();
    fs::rename(&tmp, config_path)?;
    crate::secrets::chmod_owner_only(config_path).ok();

    Ok(report)
}

/// Intenta parsear `<key> = <value>` de una línea, conservando comentarios
/// y formato en lo posible. Devuelve `None` si la línea no es un par
/// clave-valor (ej: comentario, sección, vacía).
fn parse_kv_field(line: &str) -> Option<(String, String)> {
    let stripped = line.trim_start();
    if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with(';') || stripped.starts_with('[') {
        return None;
    }
    // Cortar comentario inline `;` o `#` (si va precedido de espacio).
    let no_comment = if let Some(idx) = stripped.find(" #") {
        &stripped[..idx]
    } else if let Some(idx) = stripped.find(" ;") {
        &stripped[..idx]
    } else {
        stripped
    };
    let no_comment = no_comment.trim_end();
    let mut parts = no_comment.splitn(2, '=');
    let key = parts.next()?.trim();
    let val = parts.next()?.trim();
    if key.is_empty() || val.is_empty() { return None; }
    // key debe ser alfanumérico + underscore.
    if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { return None; }
    Some((key.to_string(), val.to_string()))
}

/// Reemplaza el valor de `line` (formato `key = old_val`) por `key = new_val`,
/// preservando el resto del formato (indentación, comentario inline si lo había).
fn rewrite_field(line: &str, new_val: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let mut parts = trimmed.splitn(2, '=');
    let key = parts.next().unwrap_or("").trim_end();
    let rest = parts.next().unwrap_or("");
    // Buscar comentario inline
    let (val_part, comment) = if let Some(idx) = rest.find(" #") {
        (&rest[..idx], &rest[idx..])
    } else if let Some(idx) = rest.find(" ;") {
        (&rest[..idx], &rest[idx..])
    } else {
        (rest, "")
    };
    let trailing_ws = val_part.len() - val_part.trim_end().len();
    let trailing_ws = " ".repeat(trailing_ws);
    format!("{indent}{key} = {new_val}{trailing_ws}{comment}")
}

fn write_backup(config_path: &Path, backup_dir: Option<&Path>) -> Result<PathBuf, MigrationError> {
    let stamp = unix_timestamp();
    let fname = format!(
        "{}.bak.{}",
        config_path.file_name().and_then(|s| s.to_str()).unwrap_or("connection.config"),
        stamp
    );
    let dir = backup_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| config_path.parent().map(|p| p.to_path_buf()).unwrap_or_default());
    fs::create_dir_all(&dir)?;
    let backup_path = dir.join(fname);
    fs::copy(config_path, &backup_path)?;
    crate::secrets::chmod_owner_only(&backup_path).ok();
    Ok(backup_path)
}

fn unix_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() { *b = (i as u8).wrapping_mul(13).wrapping_add(7); }
        k
    }

    #[test]
    fn parse_kv_field_basic() {
        let l = "fortinet_1_v3_auth_password = Emanuel0125*";
        let (k, v) = parse_kv_field(l).unwrap();
        assert_eq!(k, "fortinet_1_v3_auth_password");
        assert_eq!(v, "Emanuel0125*");
    }

    #[test]
    fn parse_kv_field_inline_comment() {
        let l = "fortinet_1_v3_auth_password = Emanuel0125*  # operador1";
        let (k, v) = parse_kv_field(l).unwrap();
        assert_eq!(v, "Emanuel0125*");
    }

    #[test]
    fn parse_kv_field_section() {
        assert!(parse_kv_field("[paths]").is_none());
        assert!(parse_kv_field("# comment").is_none());
        assert!(parse_kv_field("").is_none());
    }

    #[test]
    fn rewrite_field_preserves_indent() {
        let line = "    v3_auth_password = old";
        let r = rewrite_field(line, "$enc$2$XYZ");
        assert!(r.starts_with("    v3_auth_password = $enc$2$XYZ"));
    }

    #[test]
    fn migrate_end_to_end() {
        let k = test_key();
        let dir = std::env::temp_dir().join(format!(
            "ness-relay-mig-test-{}-{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("connection.config");
        // Pre-cifrar un valor que luego se queda intacto (campo "ya cifrado").
        let pre_enc = crate::secrets::crypto::encrypt_str(
            &k, "PreCifrado!99", b"mikrotik_1|v3_priv_password",
        ).unwrap();
        {
            let mut f = fs::File::create(&cfg).unwrap();
            writeln!(f, "[paths]").unwrap();
            writeln!(f, "log_dir = /var/log/ness_relay").unwrap();
            writeln!(f).unwrap();
            writeln!(f, "[devices]").unwrap();
            writeln!(f, "device = fortinet_1").unwrap();
            writeln!(f, "fortinet_1_ip = 192.168.10.17").unwrap();
            writeln!(f, "fortinet_1_v3_auth_password = Emanuel0125*").unwrap();
            writeln!(f, "fortinet_1_v3_priv_password = Emanuel0125*").unwrap();
            writeln!(f).unwrap();
            writeln!(f, "device = mikrotik_1").unwrap();
            writeln!(f, "mikrotik_1_ip = 10.0.0.1").unwrap();
            writeln!(f, "mikrotik_1_v3_auth_password = OtraPass!23").unwrap();
            // Campo ya cifrado (con un token real generado por el agente) →
            // no se debe tocar. Usamos un valor cifrado válido para que el
            // test de verificación al final pueda descifrarlo.
            writeln!(f, "mikrotik_1_v3_priv_password = {pre_enc}").unwrap();
        }

        let mut calls = 0;
        let report = migrate_plaintext_config(
            &k,
            &cfg,
            None,
            |field, aad| {
                calls += 1;
                let s: String = match aad {
                    "fortinet_1|v3_auth_password" => "Emanuel0125*".to_string(),
                    "fortinet_1|v3_priv_password" => "Emanuel0125*".to_string(),
                    "mikrotik_1|v3_auth_password" => "OtraPass!23".to_string(),
                    _ => unreachable!(),
                };
                Ok(Some(s))
            },
            |_field| Ok(true),
        )
        .unwrap();

        assert_eq!(report.devices_scanned, 2);
        assert_eq!(report.fields_migrated, 3);
        assert_eq!(report.fields_skipped_existing, 1);
        assert_eq!(calls, 3);

        // Verificar que los sensibles quedaron cifrados y los otros no.
        let content = fs::read_to_string(&cfg).unwrap();
        assert!(content.contains("fortinet_1_v3_auth_password = $enc$2$"));
        assert!(content.contains("fortinet_1_v3_priv_password = $enc$2$"));
        assert!(content.contains("mikrotik_1_v3_auth_password = $enc$2$"));
        // El que ya estaba cifrado se preserva (mismo valor que escribimos).
        assert!(content.contains(&format!(
            "mikrotik_1_v3_priv_password = {pre_enc}"
        )));
        // Los no sensibles no se tocan.
        assert!(content.contains("fortinet_1_ip = 192.168.10.17"));
        assert!(content.contains("log_dir = /var/log/ness_relay"));

        // Verificar que los cifrados descifran correctamente con el AAD correcto.
        // Recorremos el archivo manteniendo el (vendor, idx) actual.
        let mut re_encountered: HashMap<String, String> = HashMap::new();
        let mut in_dev: Option<(String, String)> = None;
        for line in content.lines() {
            let t = line.trim_start();
            if let Some(rest) = t.strip_prefix("device") {
                if let Some(eq) = rest.find('=') {
                    let v = rest[eq + 1..].trim();
                    if let Some((vv, ii)) = v.rsplit_once('_') {
                        in_dev = Some((vv.to_string(), ii.to_string()));
                    }
                }
            }
            if let Some((k_name, val)) = parse_kv_field(line) {
                // Extraer el "field" final buscando por sufijo.
                let field = SENSITIVE_FIELDS
                    .iter()
                    .find(|f| k_name.len() > f.len() + 1
                        && k_name.as_bytes()[k_name.len() - f.len() - 1] == b'_'
                        && k_name.ends_with(*f))
                    .copied()
                    .unwrap_or("");
                if !field.is_empty() {
                    if val.starts_with(crate::secrets::ENC_PREFIX) {
                        if let Some((vv, ii)) = &in_dev {
                            let aad = format!("{vv}_{ii}|{field}");
                            let p = crate::secrets::crypto::decrypt_str(&k, &val, aad.as_bytes()).unwrap();
                            re_encountered.insert(k_name, p.to_string());
                        }
                    }
                }
            }
        }
        assert_eq!(re_encountered.get("fortinet_1_v3_auth_password").unwrap(), "Emanuel0125*");
        assert_eq!(re_encountered.get("fortinet_1_v3_priv_password").unwrap(), "Emanuel0125*");
        assert_eq!(re_encountered.get("mikrotik_1_v3_auth_password").unwrap(), "OtraPass!23");

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_skip_intentional() {
        let k = test_key();
        let dir = std::env::temp_dir().join(format!(
            "ness-relay-mig-skip-test-{}-{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("connection.config");
        fs::write(&cfg, "device = fortinet_1\nfortinet_1_v3_auth_password = OldPass\n").unwrap();

        let mut calls = 0;
        let report = migrate_plaintext_config(
            &k, &cfg, None,
            |_f, _a| { calls += 1; Ok(None) }, // siempre skip
            |_f| Ok(true),
        ).unwrap();

        assert_eq!(report.fields_migrated, 0);
        assert_eq!(calls, 1);

        // El campo debe seguir en plano.
        let c = fs::read_to_string(&cfg).unwrap();
        assert!(c.contains("fortinet_1_v3_auth_password = OldPass"));
        let _ = fs::remove_dir_all(&dir);
    }
}
