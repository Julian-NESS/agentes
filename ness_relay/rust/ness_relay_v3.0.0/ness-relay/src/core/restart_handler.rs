// ==============================================================================
// NESS Relay v2.0.0 — Manejador de Reinicio Graceful
// Coordina el reinicio del agente después de una actualización
// ==============================================================================

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, warn};

/// Ruta por defecto del archivo de flag de reinicio pendiente.
pub fn default_restart_flag_path() -> PathBuf {
    PathBuf::from("/tmp/ness_relay.restart_pending")
}

/// Estructura que representa un reinicio pendiente.
#[derive(Debug, Clone)]
pub struct RestartPending {
    /// Nueva versión que se aplicará después del reinicio
    pub new_version: String,
    /// Timestamp UNIX de cuándo se marcó como pendiente
    pub marked_at: u64,
}

/// Marca una actualización como pendiente de reinicio.
/// Esta función escribe un archivo flag que será detectado por el supervisor (systemd/docker).
pub fn trigger_graceful_restart(
    new_version: &str,
    flag_path: Option<&Path>,
) -> Result<PathBuf> {
    let restart_flag = flag_path.map(|p| p.to_path_buf()).unwrap_or_else(default_restart_flag_path);

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Crear contenido del flag con información útil para debugging
    let flag_content = format!(
        "{}|{}|{}",
        new_version,
        timestamp,
        chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string()
    );

    fs::write(&restart_flag, flag_content)
        .context("No se pudo crear archivo de flag de reinicio")?;

    info!(
        "✓ Reinicio graceful marcado: agente se reiniciará al completar el ciclo actual",
    );
    info!(
        "  Nueva versión: {}",
        new_version
    );
    info!(
        "  Flag: {}",
        restart_flag.display()
    );

    Ok(restart_flag)
}

/// Verifica si hay un reinicio pendiente.
pub fn check_restart_pending(flag_path: Option<&Path>) -> Result<Option<RestartPending>> {
    let restart_flag = flag_path.map(|p| p.to_path_buf()).unwrap_or_else(default_restart_flag_path);

    if !restart_flag.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&restart_flag)
        .context("No se pudo leer archivo de flag de reinicio")?;

    let parts: Vec<&str> = content.split('|').collect();
    if parts.len() >= 2 {
        let new_version = parts[0].to_string();
        let marked_at: u64 = parts[1]
            .parse()
            .unwrap_or(0);

        Ok(Some(RestartPending {
            new_version,
            marked_at,
        }))
    } else {
        warn!("Flag de reinicio tiene formato inválido, ignorando");
        Ok(None)
    }
}

/// Limpia el flag de reinicio después de que se haya aplicado la actualización.
pub fn clear_restart_flag(flag_path: Option<&Path>) -> Result<()> {
    let restart_flag = flag_path.map(|p| p.to_path_buf()).unwrap_or_else(default_restart_flag_path);

    if restart_flag.exists() {
        fs::remove_file(&restart_flag)
            .context("No se pudo eliminar archivo de flag de reinicio")?;
        info!("Flag de reinicio eliminado");
    }

    Ok(())
}

/// Ejecuta el nuevo binario con la configuración preservada.
///
/// # Nota
/// Esta función reemplaza el proceso actual. No retorna a menos que falle.
/// El supervisor (systemd/docker) será responsable de reiniciar el proceso.
#[cfg(unix)]
pub async fn restart_with_new_binary(
    new_binary_path: &Path,
    preserved_config_args: Vec<String>,
) -> Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let mut cmd = Command::new(new_binary_path);

    // Agregar argumentos de configuración preservada
    for arg in preserved_config_args {
        cmd.arg(arg);
    }

    // Agregar modo continuo (recolección)
    cmd.arg("--continuous");
    cmd.arg("5"); // Intervalo por defecto

    info!(
        "Ejecutando nuevo binario: {} (reemplazo del proceso actual)",
        new_binary_path.display()
    );

    // exec reemplaza el proceso actual, nunca retorna a menos que falle
    let err = cmd.exec();
    Err(anyhow!(
        "No se pudo ejecutar nuevo binario: {}",
        err
    ))
}

/// Limpia artefactos de reinicio (flags antiguos, etc.)
pub fn cleanup_restart_artifacts(flag_path: Option<&Path>) -> Result<()> {
    let restart_flag = flag_path.map(|p| p.to_path_buf()).unwrap_or_else(default_restart_flag_path);

    if restart_flag.exists() {
        fs::remove_file(&restart_flag)
            .context("No se pudo limpiar flag de reinicio antiguo")?;
        info!("Artefactos de reinicio limpiados");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_trigger_graceful_restart() {
        let tmp = TempDir::new().unwrap();
        let flag_path = tmp.path().join("restart_flag");

        let result = trigger_graceful_restart("2.1.0", Some(&flag_path));
        assert!(result.is_ok());
        assert!(flag_path.exists());

        let content = fs::read_to_string(&flag_path).unwrap();
        assert!(content.contains("2.1.0"));
    }

    #[test]
    fn test_check_restart_pending() {
        let tmp = TempDir::new().unwrap();
        let flag_path = tmp.path().join("restart_flag");

        // Sin flag
        let result = check_restart_pending(Some(&flag_path)).unwrap();
        assert!(result.is_none());

        // Crear flag
        trigger_graceful_restart("2.1.0", Some(&flag_path)).unwrap();

        // Verificar que se detecta
        let result = check_restart_pending(Some(&flag_path)).unwrap();
        assert!(result.is_some());
        let pending = result.unwrap();
        assert_eq!(pending.new_version, "2.1.0");
        assert!(pending.marked_at > 0);
    }

    #[test]
    fn test_clear_restart_flag() {
        let tmp = TempDir::new().unwrap();
        let flag_path = tmp.path().join("restart_flag");

        trigger_graceful_restart("2.1.0", Some(&flag_path)).unwrap();
        assert!(flag_path.exists());

        clear_restart_flag(Some(&flag_path)).unwrap();
        assert!(!flag_path.exists());
    }
}
