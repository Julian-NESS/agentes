// ==============================================================================
// NESS Relay v2.0.0 — Preservación de Configuración
// Guarda y restaura variables críticas del agente durante actualizaciones
// ==============================================================================

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Configuración crítica del agente que debe preservarse durante actualizaciones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreservedConfig {
    /// Token de autenticación API
    pub api_token: String,
    /// ID del servidor NESS
    pub server_id: String,
    /// URL del servidor NESS personalizada (si existe)
    pub server_url: Option<String>,
    /// URL personalizada para verificación de versiones (si existe)
    pub version_check_url: Option<String>,
    /// URL personalizada para reportes de actualización (si existe)
    pub update_report_url: Option<String>,
    /// Intervalo de recolección en minutos
    pub collection_interval_minutes: u64,
    /// Directorio de configuración de dispositivos
    pub devices_config_path: PathBuf,
    /// Directorio de salida de datos
    pub output_dir: PathBuf,
    /// Directorio de logs
    pub log_dir: PathBuf,
    /// Timestamp de cuando se guardó esta configuración
    pub saved_at: String,
}

impl PreservedConfig {
    /// Crea un nuevo `PreservedConfig` desde variables de entorno y rutas.
    pub fn from_env(
        api_token: String,
        server_id: String,
        collection_interval_minutes: u64,
        devices_config_path: PathBuf,
        output_dir: PathBuf,
        log_dir: PathBuf,
    ) -> Self {
        use std::env;

        let server_url = env::var("NESS_SERVER_URL").ok();
        let version_check_url = env::var("NESS_VERSION_CHECK_URL").ok();
        let update_report_url = env::var("NESS_UPDATE_REPORT_URL").ok();

        PreservedConfig {
            api_token,
            server_id,
            server_url,
            version_check_url,
            update_report_url,
            collection_interval_minutes,
            devices_config_path,
            output_dir,
            log_dir,
            saved_at: chrono::Utc::now()
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
        }
    }
}

/// Ruta por defecto donde guardar la configuración preservada.
/// Usa `/tmp` para que sea independiente del directorio de instalación.
pub fn default_backup_path() -> PathBuf {
    PathBuf::from("/tmp/ness_relay_config_backup.json")
}

/// Guarda la configuración crítica en un archivo JSON.
/// El archivo se protege con permisos 0600 (solo lectura/escritura del propietario).
pub fn save_config(config: &PreservedConfig, path: Option<&Path>) -> Result<PathBuf> {
    let backup_path = path.map(|p| p.to_path_buf()).unwrap_or_else(default_backup_path);

    let json_str = serde_json::to_string_pretty(config)
        .context("No se pudo serializar configuración a JSON")?;

    // Crear directorio padre si no existe
    if let Some(parent) = backup_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .context("No se pudo crear directorio para backup de configuración")?;
        }
    }

    fs::write(&backup_path, json_str).context("No se pudo escribir archivo de configuración")?;

    // Establecer permisos restrictivos (0600: rw-------)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        fs::set_permissions(&backup_path, perms)
            .context("No se pudo establecer permisos en archivo de configuración")?;
    }

    info!(
        "Configuración guardada en: {} (guardada a las {})",
        backup_path.display(),
        config.saved_at
    );
    Ok(backup_path)
}

/// Carga la configuración desde un archivo JSON.
pub fn load_config(path: Option<&Path>) -> Result<PreservedConfig> {
    let backup_path = path.map(|p| p.to_path_buf()).unwrap_or_else(default_backup_path);

    if !backup_path.exists() {
        return Err(anyhow!(
            "Archivo de configuración no encontrado: {}",
            backup_path.display()
        ));
    }

    let json_str = fs::read_to_string(&backup_path)
        .context("No se pudo leer archivo de configuración")?;

    let config: PreservedConfig = serde_json::from_str(&json_str)
        .context("No se pudo deserializar archivo de configuración")?;

    info!(
        "Configuración restaurada desde: {} (guardada a las {})",
        backup_path.display(),
        config.saved_at
    );
    Ok(config)
}

/// Aplica la configuración preservada estableciendo variables de entorno.
///
/// # Nota
/// Esta función establece variables de entorno en el proceso actual.
/// Para que se apliquen persistentemente en el agente reiniciado,
/// debe pasarse como argumentos al nuevo proceso.
pub fn apply_config_as_env_vars(config: &PreservedConfig) -> Result<()> {
    use std::env;

    // Establecer variables de entorno críticas
    env::set_var("NESS_API_TOKEN", &config.api_token);
    env::set_var("NESS_SERVER_ID", &config.server_id);

    if let Some(server_url) = &config.server_url {
        env::set_var("NESS_SERVER_URL", server_url);
    }

    if let Some(version_check_url) = &config.version_check_url {
        env::set_var("NESS_VERSION_CHECK_URL", version_check_url);
    }

    if let Some(update_report_url) = &config.update_report_url {
        env::set_var("NESS_UPDATE_REPORT_URL", update_report_url);
    }

    env::set_var(
        "NESS_DEVICES_FILE",
        config.devices_config_path.to_string_lossy().to_string(),
    );
    env::set_var(
        "NESS_OUTPUT_DIR",
        config.output_dir.to_string_lossy().to_string(),
    );
    env::set_var(
        "NESS_LOG_DIR",
        config.log_dir.to_string_lossy().to_string(),
    );

    info!("Configuración aplicada como variables de entorno");
    Ok(())
}

/// Genera argumentos de línea de comandos para pasar la configuración al nuevo binario.
///
/// Retorna un vector de String en formato ["--key", "value", ...].
/// Útil para pasar al nuevo proceso después de actualizar.
pub fn config_as_args(config: &PreservedConfig) -> Vec<String> {
    let mut args = Vec::new();

    // Siempre pasar token y server_id
    args.push("--api-token".to_string());
    args.push(config.api_token.clone());
    args.push("--server-id".to_string());
    args.push(config.server_id.clone());

    // Pasar URLs opcionales si existen
    if let Some(server_url) = &config.server_url {
        args.push("--server-url".to_string());
        args.push(server_url.clone());
    }

    if let Some(version_check_url) = &config.version_check_url {
        args.push("--version-check-url".to_string());
        args.push(version_check_url.clone());
    }

    if let Some(update_report_url) = &config.update_report_url {
        args.push("--update-report-url".to_string());
        args.push(update_report_url.clone());
    }

    // Pasar rutas
    args.push("--devices-file".to_string());
    args.push(config.devices_config_path.to_string_lossy().to_string());
    args.push("--output-dir".to_string());
    args.push(config.output_dir.to_string_lossy().to_string());
    args.push("--log-dir".to_string());
    args.push(config.log_dir.to_string_lossy().to_string());

    args
}

/// Limpia el archivo de configuración preservada.
/// Llamar después de una actualización exitosa.
pub fn cleanup_backup(path: Option<&Path>) -> Result<()> {
    let backup_path = path.map(|p| p.to_path_buf()).unwrap_or_else(default_backup_path);

    if backup_path.exists() {
        fs::remove_file(&backup_path).context("No se pudo eliminar archivo de configuración")?;
        info!("Archivo de configuración eliminado: {}", backup_path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_serialize_deserialize() {
        let config = PreservedConfig {
            api_token: "test_token_123".to_string(),
            server_id: "1".to_string(),
            server_url: Some("http://test.example.com".to_string()),
            version_check_url: None,
            update_report_url: None,
            collection_interval_minutes: 5,
            devices_config_path: PathBuf::from("/tmp/devices.config"),
            output_dir: PathBuf::from("/tmp/output"),
            log_dir: PathBuf::from("/tmp/logs"),
            saved_at: "2026-04-30 12:00:00 UTC".to_string(),
        };

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");

        // Guardar
        save_config(&config, Some(&path)).unwrap();
        assert!(path.exists());

        // Cargar
        let loaded = load_config(Some(&path)).unwrap();
        assert_eq!(loaded.api_token, config.api_token);
        assert_eq!(loaded.server_id, config.server_id);
        assert_eq!(loaded.collection_interval_minutes, config.collection_interval_minutes);
    }

    #[test]
    fn test_config_as_args() {
        let config = PreservedConfig {
            api_token: "token".to_string(),
            server_id: "2".to_string(),
            server_url: Some("http://custom.com".to_string()),
            version_check_url: None,
            update_report_url: None,
            collection_interval_minutes: 10,
            devices_config_path: PathBuf::from("/etc/ness/devices.config"),
            output_dir: PathBuf::from("/var/ness/output"),
            log_dir: PathBuf::from("/var/ness/logs"),
            saved_at: "2026-04-30 12:00:00 UTC".to_string(),
        };

        let args = config_as_args(&config);
        assert!(args.contains(&"--api-token".to_string()));
        assert!(args.contains(&"token".to_string()));
        assert!(args.contains(&"--server-id".to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(args.contains(&"--server-url".to_string()));
        assert!(args.contains(&"http://custom.com".to_string()));
    }
}
