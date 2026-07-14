// ==============================================================================
// NESS Relay v2.0.0 — Configuración global
// Equivalente Python: core/config.py
// ==============================================================================
//
// Lee variables de entorno y el archivo connection.config.
// Formato de connection.config (idéntico al Python):
//   pfsense_1_ip=192.168.1.1
//   pfsense_1_community=public
//   pfsense_1_snmp_version=2c
//   pfsense_1_description=Firewall Principal
//   pfsense_1_port=161
//   ...
// ==============================================================================

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use crate::secrets;

// ==============================================================================
// CONSTANTES
// ==============================================================================

pub const RELAY_VERSION: &str = "3.0.0";
pub const RELAY_TYPE: &str = "ness-relay";
pub const MAX_BACKUPS: usize = 5;
pub const UPDATE_CHECK_INTERVAL_HOURS: u64 = 24;
pub const VERSION_CHECK_URL_GCP: &str = "https://storage.googleapis.com/agent-updates-lab/utilities/relay/latest.json";
pub const HOSTING_BASE_URL_GCP: &str = "https://storage.googleapis.com/agent-updates-lab/utilities/relay";

/// URLs de servidores NESS por SERVER_ID (hardcodeadas por seguridad).
/// El instalador solo maneja IDs (1, 2, 3) sin exponer las rutas reales.
pub fn server_url_by_id(server_id: &str) -> &'static str {
    match server_id {
        "1" => "http://172.206.0.217:8080/api/relay/data/",
        "2" => "https://testing.nesshq.com/api/relay/data/",
        "3" => "https://cloud.nesshq.com/api/relay/data/",
        _   => "https://cloud.nesshq.com/api/relay/data/",
    }
}

/// Vendors soportados (solo dispositivos de red).
/// Se usan para parsear el archivo connection.config:
///   {vendor}_{index}_ip=...
pub const SUPPORTED_VENDORS: &[&str] = &[
    "pfsense",
    "cisco",
    "fortinet",
    "mikrotik",
    "mikrotik_fw",
    "c_n",
    "ubnt",
    "huawei",
    "tp_link",
    "dell",
    "datacomm",
    "generic",
];

// ==============================================================================
// ESTRUCTURA DE CONFIGURACIÓN GLOBAL
// ==============================================================================

#[derive(Debug, Clone)]
pub struct AppConfig {
    /// URL del servidor NESS (de la variable NESS_SERVER_URL o AUTO por default)
    pub server_url: String,
    /// ID del servidor NESS
    pub server_id: String,
    /// Token de autenticación API
    pub api_token: String,
    /// Versión del relay
    pub version: String,
    /// Tipo del relay
    pub relay_type: String,
    /// Directorio base (donde está el ejecutable)
    pub base_dir: PathBuf,
    /// Directorio raíz de instalación (ej. /opt/ness_relay/)
    pub install_dir: PathBuf,
    /// Archivo de configuración de dispositivos
    pub config_file: PathBuf,
    /// Directorio de salida de datos JSON
    pub output_dir: PathBuf,
    /// Directorio de logs
    pub log_dir: PathBuf,
    /// URL de verificación de actualizaciones
    pub version_check_url: String,
    /// URL base de descarga de hosting
    pub hosting_base_url: String,
    /// URL para reportar actualizaciones realizadas
    pub update_report_url: String,
}

impl AppConfig {
    /// Carga la configuración desde variables de entorno.
    pub fn load(base_dir: PathBuf) -> Self {
        let server_id = env::var("NESS_SERVER_ID").unwrap_or_else(|_| "3".to_string());
        let api_token = env::var("NESS_API_TOKEN").unwrap_or_default();

        // Determinar la URL del servidor NESS
        // Prioridad: 1) NESS_SERVER_URL env var, 2) URL hardcodeada por SERVER_ID
        let server_url = env::var("NESS_SERVER_URL")
            .unwrap_or_else(|_| server_url_by_id(&server_id).to_string());

        let hosting_base_url = env::var("NESS_HOSTING_URL")
            .unwrap_or_else(|_| HOSTING_BASE_URL_GCP.to_string());
        let version_check_url = env::var("NESS_VERSION_CHECK_URL")
            .unwrap_or_else(|_| VERSION_CHECK_URL_GCP.to_string());
        let update_report_url = env::var("NESS_UPDATE_REPORT_URL")
            .unwrap_or_else(|_| "https://nesshq.com/api/report-relay-update/".to_string());

        let config_file = env::var("NESS_DEVICES_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| base_dir.join("connection.config"));

        let output_dir = env::var("NESS_OUTPUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| base_dir.join("relay_output"));

        let log_dir = env::var("NESS_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| base_dir.join("logs"));

        // Directorio raíz de instalación: NESS_INSTALL_DIR → base_dir/../ → base_dir
        let install_dir = env::var("NESS_INSTALL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| base_dir.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| base_dir.clone()));

        AppConfig {
            server_url,
            server_id,
            api_token,
            version: RELAY_VERSION.to_string(),
            relay_type: RELAY_TYPE.to_string(),
            base_dir,
            install_dir,
            config_file,
            output_dir,
            log_dir,
            version_check_url,
            hosting_base_url,
            update_report_url,
        }
    }

    /// URL completa para envío de datos al servidor NESS.
    /// La URL ya incluye el path completo (/api/relay/data/) desde server_url_by_id().
    pub fn send_data_url(&self) -> String {
        self.server_url.clone()
    }

    /// URL completa para verificación de versión.
    pub fn version_check_url(&self) -> &str {
        &self.version_check_url
    }
}

// ==============================================================================
// CONFIGURACIÓN DE DISPOSITIVOS
// ==============================================================================

/// Configuración de un dispositivo individual.
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    /// Identificador único (ej. "pfsense_1")
    pub device_id: String,
    /// Vendor del dispositivo (decodificado: ej: "fortinet" no "fortinet_1")
    pub vendor: String,
    /// IP del dispositivo
    pub ip: String,
    /// Puerto SNMP (default 161)
    pub port: u16,
    /// Descripción del dispositivo
    pub description: String,
    /// Versión SNMP ("1", "2c", "3")
    pub snmp_version: String,
    /// Community string (v1/v2c)
    pub community: String,
    /// Usuario SNMPv3
    pub v3_user: String,
    /// Protocolo de autenticación SNMPv3
    pub v3_auth_protocol: String,
    /// Password de autenticación SNMPv3
    pub v3_auth_password: String,
    /// Protocolo de privacidad SNMPv3
    pub v3_priv_protocol: String,
    /// Password de privacidad SNMPv3
    pub v3_priv_password: String,

    // =========================================================================
    // SSH audit fields (Phase 2.4 — opt-in, defaults to disabled)
    // =========================================================================
    /// True only if the connection.config file has `ssh_enabled=true` AND all
    /// other ssh_* fields are present and the env var resolves. Computed
    /// during `load_devices_from_config`.
    pub ssh_enabled: bool,
    /// Hostname/IP to use for SSH. Defaults to `self.ip` if empty.
    pub ssh_host: Option<String>,
    /// SSH TCP port. Defaults to 22 if None.
    pub ssh_port: Option<u16>,
    /// SSH username (e.g. "admin"). Required for audit.
    pub ssh_username: Option<String>,
    /// Name of the env var that holds the SSH password (the password itself
    /// is NEVER stored in the connection.config file — it must be exported
    /// in the shell environment before the agent runs).
    pub ssh_password_env: Option<String>,
}

impl DeviceConfig {
    /// Convierte a serde_json::Value para pasarlo al SnmpClient.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "device_id": self.device_id,
            "vendor": self.vendor,
            "ip": self.ip,
            "port": self.port,
            "description": self.description,
            "snmp_version": self.snmp_version,
            "community": self.community,
            "v3_user": self.v3_user,
            "v3_auth_protocol": self.v3_auth_protocol,
            "v3_auth_password": self.v3_auth_password,
            "v3_priv_protocol": self.v3_priv_protocol,
            "v3_priv_password": self.v3_priv_password,
        })
    }

    /// Resolve SSH credentials from the device config + vault (v2.5.0).
    ///
    /// Returns `Some(SshCredentials)` only when **all** of the following hold:
    /// 1. `ssh_enabled == true` (declared in the config file)
    /// 2. A non-empty `ssh_username` is configured
    /// 3. A non-empty `ssh_password_env` looks like a valid env var name
    ///    (`[A-Z0-9_]+`, not starting with a digit)
    /// 4. The SSH password is available, from one of (in priority order):
    ///    a. **Encrypted vault** (`/etc/ness_relay/secrets.enc`) — preferred.
    ///    b. Process environment (`std::env::var(env_var)`) — v2.4.0 fallback.
    ///
    /// Otherwise returns `None` — the caller should log a warning and skip the
    /// audit phases for this device. The agent never **stores** the password
    /// in plaintext on disk; even the v2.4.0 fallback is only used when
    /// the vault is unavailable.
    pub fn ssh_credentials(&self) -> Option<SshCredentials> {
        if !self.ssh_enabled {
            return None;
        }
        let username = self.ssh_username.as_deref()?.trim();
        if username.is_empty() {
            return None;
        }
        let env_var = self.ssh_password_env.as_deref()?.trim();
        if env_var.is_empty() || !is_valid_env_var_name(env_var) {
            return None;
        }

        // 1) Intentar primero desde el vault cifrado (v2.5.0+).
        if let Ok(mk) = secrets::master_key() {
            let vault_path = secrets::secrets_file();
            if vault_path.exists() {
                if let Ok(map) = secrets::load_env_file_decrypted(&mk, &vault_path) {
                    if let Some(pw) = map.get(env_var) {
                        if !pw.is_empty() {
                            return Some(self.build_ssh_creds(username, pw.clone()));
                        }
                    }
                }
            }
        }

        // 2) Fallback al env var del proceso (v2.4.0 compat).
        let password = std::env::var(env_var).ok()?;
        if password.is_empty() {
            return None;
        }
        Some(self.build_ssh_creds(username, password))
    }

    /// Helper: construye SshCredentials con host/port resueltos.
    fn build_ssh_creds(&self, username: &str, password: String) -> SshCredentials {
        let host = self
            .ssh_host
            .clone()
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_else(|| self.ip.clone());
        let port = self.ssh_port.unwrap_or(22);
        SshCredentials { host, port, username: username.to_string(), password }
    }
}

/// Devuelve el motivo por el cual las credenciales SSH NO están disponibles.
/// Se usa para diagnóstico en runtime.
pub fn ssh_unavailable_reason(device: &DeviceConfig) -> &'static str {
    if !device.ssh_enabled {
        return "ssh_enabled=false en connection.config";
    }
    if device.ssh_username.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
        return "ssh_username vacío o ausente en connection.config";
    }
    let env_var = match device.ssh_password_env.as_deref() {
        Some(v) if !v.trim().is_empty() => v.trim(),
        _ => return "ssh_password_env ausente o vacío en connection.config",
    };
    if !is_valid_env_var_name(env_var) {
        return "ssh_password_env no tiene formato válido de env var (ej: NESS_SSH_PASSWORD_fortinet_1)";
    }
    match std::env::var(env_var) {
        Ok(v) if !v.is_empty() => "OK",
        Ok(_) => "la variable de entorno existe pero su valor está VACÍO",
        Err(_) => "la variable de entorno NO está seteada en este proceso",
    }
}

/// Resolved SSH connection inputs. The `password` field is intended only for
/// short-lived use; do not log it, do not persist it, do not return it from
/// API surfaces.
#[derive(Debug, Clone)]
pub struct SshCredentials {
    /// Target host (IP or hostname). Defaults to the device's SNMP IP.
    pub host: String,
    /// TCP port (defaults to 22).
    pub port: u16,
    /// Username.
    pub username: String,
    /// Password (read from env var at run time — never persisted).
    pub password: String,
}

impl SshCredentials {
    /// Returns a redacted description safe for logs.
    pub fn describe(&self) -> String {
        format!("ssh://{}@{}:{}", self.username, self.host, self.port)
    }
}

/// Validates that a string looks like an env var name: `[A-Za-z_][A-Za-z0-9_]+`.
///
/// Phase 2.4 update: aceptamos minúsculas además de mayúsculas. El validador
/// original solo aceptaba `[A-Z_]` pero en la práctica los nombres de env var
/// pueden contener minúsculas (ej: `NESS_SSH_PASSWORD_fortinet_1` con el
/// vendor slug en minúscula). Ser estrictos con mayúsculas bloqueaba la
/// auditoría sin motivo real.
///
/// Sigue siendo defensivo: no aceptamos dígitos al inicio (regla POSIX) ni
/// caracteres especiales fuera de `[A-Za-z0-9_]`.
fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Carga los dispositivos desde el archivo connection.config.
///
/// Formato del archivo:
/// ```
/// # Comentario
/// pfsense_1_ip=192.168.1.1
/// pfsense_1_description=Firewall Principal
/// pfsense_1_community=public
/// pfsense_1_snmp_version=2c
/// pfsense_1_port=161
///
/// mikrotik_1_ip=10.0.0.1
/// mikrotik_1_community=public
/// ```
pub fn load_devices_from_config(config_file: &Path) -> Result<Vec<DeviceConfig>> {
    let content = std::fs::read_to_string(config_file)
        .with_context(|| format!("No se pudo leer el archivo: {:?}", config_file))?;

    // Parsear key=value (ignorando comentarios y líneas vacías)
    let mut flat: HashMap<String, String> = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            flat.insert(key.trim().to_string(), val.trim().to_string());
        }
    }

    // Agrupar por vendor_index (e.g. "pfsense_1", "fortinet_2", "generic_3")
    // ────────────────────────────────────────────────────────
    // BUG CRÍTICO Phase 2.7: el código anterior iteraba SUPPORTED_VENDORS
    // en orden fijo y rompía el loop cuando vendor_1_ip no existía. Esto
    // provocaba que un device generic_2_ip existente NUNCA se cargara
    // si generic_1_ip había sido renombrado a fortinet_1_ip por un probe.
    //
    // Fix: ahora escaneamos TODAS las claves y agrupamos por el primer
    // segmento numérico que encontremos (después del vendor). Esto coincide
    // con la lógica de `reconcile_vendor_counters` del instalador bash
    // y garantiza que TODOS los devices se carguen, sin importar el orden
    // en SUPPORTED_VENDORS.
    // ────────────────────────────────────────────────────────
    let mut device_keys: std::collections::BTreeMap<String, HashMap<String, String>> =
        std::collections::BTreeMap::new();

    // Patrón: <vendor_part>_<idx>_<field>
    // Capturamos el primer segmento numérico como idx.
    // vendor_part puede contener guiones bajos (e.g. "mikrotik_fw").
    //
    // Implementación sin regex (POSIX): split por "_" y encontrar el primer
    // segmento que sea numérico puro.
    for key in flat.keys() {
        // Split key por "_"
        let parts: Vec<&str> = key.split('_').collect();
        if parts.len() < 3 {
            continue; // Necesitamos al menos vendor_idx_field
        }

        // Encontrar el primer segmento numérico (ese es idx).
        let mut idx_pos: Option<usize> = None;
        for (i, part) in parts.iter().enumerate() {
            if !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()) {
                idx_pos = Some(i);
                break;
            }
        }

        let idx_pos = match idx_pos {
            Some(p) if p >= 1 && p < parts.len() - 1 => p, // idx entre vendor y field
            _ => continue,
        };

        let idx_str = parts[idx_pos];
        let idx: u32 = match idx_str.parse() {
            Ok(n) if n > 0 => n,
            _ => continue,
        };

        // vendor_part = parts[0..idx_pos] joined con "_"
        let vendor_part = parts[..idx_pos].join("_");
        // field = parts[idx_pos+1..] joined con "_"
        let field = parts[idx_pos + 1..].join("_");

        let prefix = format!("{}_{}", vendor_part, idx);
        let entry = device_keys.entry(prefix).or_default();

        // Inyectar este field
        entry.insert(field, flat[key].clone());
    }

    // Ahora completar cada device con su vendor explícito o derivado
    let mut devices = Vec::new();
    for (device_id, props) in device_keys {
        let ip = match props.get("ip") {
            Some(ip) if !ip.is_empty() => ip.clone(),
            _ => continue, // sin IP, ignorar
        };

        // Determinar vendor:
        //   1. Si el campo vendor= explícito está en SUPPORTED_VENDORS, usarlo
        //   2. Si no, derivarlo del prefijo (e.g. "fortinet_1" → "fortinet",
        //      "mikrotik_fw_2" → "mikrotik_fw", "generic_3" → "generic")
        let explicit_vendor = props
            .get("vendor")
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty() && SUPPORTED_VENDORS.contains(&v.as_str()));

        // Derivar vendor del device_id
        let derived_vendor = {
            // device_id = "<vendor>_<idx>" → vendor = todo antes del último "_<idx>"
            let last_underscore = device_id.rfind('_').unwrap_or(0);
            device_id[..last_underscore].to_string()
        };

        let vendor = explicit_vendor.unwrap_or(derived_vendor);
        // Clonar device_id antes de moverlo al struct — lo necesitamos
        // para construir el AAD de descifrado de campos sensibles.
        let device_id_for_aad = device_id.clone();
        let port = props
            .get("port")
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(161);

        let snmp_version = props
            .get("snmp_version")
            .cloned()
            .unwrap_or_else(|| "2c".to_string());

        devices.push(DeviceConfig {
            device_id,
            vendor,
            ip,
            port,
            description: props
                .get("description")
                .cloned()
                .unwrap_or_else(|| "Dispositivo SNMP".to_string()),
            snmp_version,
            community: {
                let raw = props
                    .get("community")
                    .cloned()
                    .unwrap_or_else(|| "public".to_string());
                // `community` solo se cifra si NO es el default "public"
                // (la mayoría de los agentes v2.4 lo dejan plano y vacío → public).
                if raw == "public" { raw } else { decrypt_sensitive_field(raw, &device_id_for_aad, "community") }
            },
            v3_user: props.get("v3_user").cloned().unwrap_or_default(),
            v3_auth_protocol: props
                .get("v3_auth_protocol")
                .cloned()
                .unwrap_or_else(|| "SHA".to_string()),
            v3_auth_password: decrypt_sensitive_field(
                props.get("v3_auth_password").cloned().unwrap_or_default(),
                &device_id_for_aad,
                "v3_auth_password",
            ),
            v3_priv_protocol: props
                .get("v3_priv_protocol")
                .cloned()
                .unwrap_or_else(|| "AES128".to_string()),
            v3_priv_password: decrypt_sensitive_field(
                props.get("v3_priv_password").cloned().unwrap_or_default(),
                &device_id_for_aad,
                "v3_priv_password",
            ),
            // -- SSH audit fields (Phase 2.4 — opt-in) --
            // `ssh_enabled` only becomes true if ALL SSH-side prerequisites
            // are satisfied. If anything is missing we silently fall back to
            // false (audit phases will be skipped for that device).
            ssh_enabled: parse_ssh_enabled(&props),
            ssh_host: props
                .get("ssh_host")
                .cloned()
                .filter(|h| !h.trim().is_empty()),
            ssh_port: props.get("ssh_port").and_then(|p| p.parse().ok()),
            ssh_username: props
                .get("ssh_username")
                .cloned()
                .filter(|u| !u.trim().is_empty()),
            ssh_password_env: props
                .get("ssh_password_env")
                .cloned()
                .filter(|e| !e.trim().is_empty() && is_valid_env_var_name(e.trim())),
        });
    }

    Ok(devices)
}

/// Inspect the per-device SSH config and decide whether the SSH audit phases
/// should be wired into the pipeline for this device.
///
/// Required keys: `ssh_enabled=true`, `ssh_username=<user>`, plus a valid
/// `ssh_password_env=<VAR>` whose value we **do not** read at load time (only
/// at run time, via `ssh_credentials()`).
fn parse_ssh_enabled(props: &HashMap<String, String>) -> bool {
    let enabled = props
        .get("ssh_enabled")
        .map(|v| v.trim().eq_ignore_ascii_case("true") || v.trim() == "1")
        .unwrap_or(false);
    if !enabled {
        return false;
    }
    let user_ok = props
        .get("ssh_username")
        .map(|u| !u.trim().is_empty())
        .unwrap_or(false);
    let env_ok = props
        .get("ssh_password_env")
        .map(|e| !e.trim().is_empty() && is_valid_env_var_name(e.trim()))
        .unwrap_or(false);
    enabled && user_ok && env_ok
}

// =============================================================================
// Descifrado de campos sensibles (v2.5.0 — AES-GCM con AAD contextual)
// =============================================================================
//
// Si el valor leído del config empieza con `$enc$` (token cifrado), se
// descifra usando la clave maestra derivada del host (HKDF-SHA256 sobre
// `machine-id` + sal local). El AAD ata criptográficamente el campo a
// `<device_id>|<field>`, evitando que un token copiado entre dispositivos
// descifre con éxito.
//
// Si el valor NO es un token (instalación v2.4.0 legada), se devuelve tal
// cual — esto preserva la retrocompatibilidad sin requerir migración
// inmediata (la migración se hace con `ness-relay-cred migrate-plaintext`).
//
// Si el descifrado FALLA (tampering, AAD incorrecto, vault corrupto), se
// registra un warning y se devuelve string VACÍO — el SNMPv3 fallará
// luego con auth-rejected, que es preferible a crashear el agente.
// =============================================================================

fn decrypt_sensitive_field(raw: String, device_id: &str, field_name: &str) -> String {
    if raw.is_empty() || !secrets::is_encrypted_token(&raw) {
        // v2.4.0 compat: texto plano
        return raw;
    }
    let aad = format!("{device_id}|{field_name}");
    match secrets::master_key() {
        Ok(mk) => match secrets::decrypt_str(&mk, &raw, aad.as_bytes()) {
            Ok(plain) => plain.to_string(),
            Err(e) => {
                eprintln!(
                    "[WARN] No se pudo descifrar {device_id}.{field_name}: {e}. \
                     ¿se perdió /etc/ness_relay/.salt o se restauró de otro host?"
                );
                String::new()
            }
        },
        Err(e) => {
            eprintln!(
                "[WARN] vault no disponible para {device_id}.{field_name}: {e}. \
                 ¿ejecutar install_relay.sh?"
            );
            String::new()
        }
    }
}
