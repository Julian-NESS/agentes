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

// ==============================================================================
// CONSTANTES
// ==============================================================================

pub const RELAY_VERSION: &str = "2.0.0";
pub const RELAY_TYPE: &str = "ness-relay";
pub const MAX_BACKUPS: usize = 5;
pub const UPDATE_CHECK_INTERVAL_HOURS: u64 = 24;
pub const VERSION_CHECK_URL_GCP: &str = "https://storage.googleapis.com/repo.nesshq.com/utilities/relay/latest.json";
pub const HOSTING_BASE_URL_GCP: &str = "https://storage.googleapis.com/repo.nesshq.com/utilities/relay";

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

/// Vendors soportados.
pub const SUPPORTED_VENDORS: &[&str] = &[
    "pfsense",
    "cisco",
    "fortinet",
    "mikrotik",
    "mikrotik_fw",
    "c_n",
    "ubnt",
    "linux",
    "windows",
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
    /// Vendor del dispositivo
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

    // Agrupar por vendor_index (e.g. "pfsense_1")
    let mut device_keys: std::collections::BTreeMap<String, HashMap<String, String>> =
        std::collections::BTreeMap::new();

    for vendor in SUPPORTED_VENDORS {
        let mut idx = 1;
        loop {
            let prefix = format!("{}_{}", vendor, idx);
            let ip_key = format!("{}_ip", prefix);
            if !flat.contains_key(&ip_key) {
                break;
            }
            let entry = device_keys.entry(prefix.clone()).or_default();
            // Recolectar todas las claves con este prefijo
            for (k, v) in &flat {
                if let Some(field) = k.strip_prefix(&format!("{}_", prefix)) {
                    entry.insert(field.to_string(), v.clone());
                }
            }
            entry.insert("vendor".to_string(), vendor.to_string());
            entry.insert("device_id".to_string(), prefix.clone());
            idx += 1;
        }
    }

    let mut devices = Vec::new();
    for (device_id, props) in device_keys {
        let ip = match props.get("ip") {
            Some(ip) if !ip.is_empty() => ip.clone(),
            _ => continue, // sin IP, ignorar
        };
        let vendor = props
            .get("vendor")
            .cloned()
            .unwrap_or_else(|| "generic".to_string());

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
            community: props
                .get("community")
                .cloned()
                .unwrap_or_else(|| "public".to_string()),
            v3_user: props.get("v3_user").cloned().unwrap_or_default(),
            v3_auth_protocol: props
                .get("v3_auth_protocol")
                .cloned()
                .unwrap_or_else(|| "SHA".to_string()),
            v3_auth_password: props.get("v3_auth_password").cloned().unwrap_or_default(),
            v3_priv_protocol: props
                .get("v3_priv_protocol")
                .cloned()
                .unwrap_or_else(|| "AES128".to_string()),
            v3_priv_password: props.get("v3_priv_password").cloned().unwrap_or_default(),
        });
    }

    Ok(devices)
}
