// ==============================================================================
// NESS Relay v2.0.0 — Rastreador de Actualizaciones
// Mantiene estado del último chequeo y próximas actualizaciones
// ==============================================================================

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

/// Estado del sistema de actualización del agente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
    /// Timestamp (segundos desde UNIX_EPOCH) del último chequeo exitoso
    pub last_check_timestamp: u64,
    /// Timestamp (segundos desde UNIX_EPOCH) de la última actualización completada
    pub last_successful_update: Option<u64>,
    /// Indica si hay una actualización pendiente de aplicar
    pub pending_update: bool,
    /// Versión del agente que será aplicada en la siguiente actualización
    pub pending_version: Option<String>,
    /// Timestamp de cuando se marcó como pendiente
    pub pending_marked_at: Option<u64>,
    /// Contador de intentos fallidos consecutivos de actualización
    pub failed_attempts: u32,
    /// Último error registrado durante actualización
    pub last_error: Option<String>,
}

impl Default for UpdateState {
    fn default() -> Self {
        UpdateState {
            last_check_timestamp: 0,
            last_successful_update: None,
            pending_update: false,
            pending_version: None,
            pending_marked_at: None,
            failed_attempts: 0,
            last_error: None,
        }
    }
}

/// Ruta por defecto donde guardar el estado de actualización.
/// Se usa `/tmp` para independencia del directorio de instalación.
pub fn default_state_path() -> PathBuf {
    PathBuf::from("/tmp/ness_relay_update_state.json")
}

/// Obtiene el timestamp actual en segundos desde UNIX_EPOCH.
fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Carga el estado actual de actualización desde archivo.
/// Si el archivo no existe, retorna estado por defecto.
pub fn load_state(path: Option<&Path>) -> Result<UpdateState> {
    let state_path = path.map(|p| p.to_path_buf()).unwrap_or_else(default_state_path);

    if !state_path.exists() {
        debug!(
            "Archivo de estado no encontrado ({}), usando estado por defecto",
            state_path.display()
        );
        return Ok(UpdateState::default());
    }

    let json_str = fs::read_to_string(&state_path)
        .context("No se pudo leer archivo de estado de actualización")?;

    let state: UpdateState = serde_json::from_str(&json_str)
        .context("No se pudo deserializar archivo de estado")?;

    debug!("Estado de actualización cargado desde: {}", state_path.display());
    Ok(state)
}

/// Guarda el estado de actualización en un archivo JSON.
pub fn save_state(state: &UpdateState, path: Option<&Path>) -> Result<PathBuf> {
    let state_path = path.map(|p| p.to_path_buf()).unwrap_or_else(default_state_path);

    // Crear directorio padre si no existe
    if let Some(parent) = state_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .context("No se pudo crear directorio para estado de actualización")?;
        }
    }

    let json_str = serde_json::to_string_pretty(state)
        .context("No se pudo serializar estado a JSON")?;

    fs::write(&state_path, json_str).context("No se pudo escribir archivo de estado")?;

    debug!("Estado de actualización guardado en: {}", state_path.display());
    Ok(state_path)
}

/// Determina si es tiempo de chequear actualizaciones.
///
/// Retorna `true` si:
/// - Nunca se ha hecho un chequeo (last_check_timestamp == 0), O
/// - Han pasado al menos `check_interval_hours` desde el último chequeo
///
/// Por defecto utiliza 24 horas como intervalo.
pub fn should_check_now(state: &UpdateState, check_interval_hours: Option<u64>) -> bool {
    let interval_secs = check_interval_hours.unwrap_or(24) * 3600;
    let current_time = get_current_timestamp();

    // Primer chequeo
    if state.last_check_timestamp == 0 {
        debug!("Primer chequeo de actualización");
        return true;
    }

    // Chequear si han pasado suficientes segundos
    let time_since_last_check = current_time.saturating_sub(state.last_check_timestamp);
    let should_check = time_since_last_check >= interval_secs;

    if should_check {
        debug!(
            "Tiempo para chequear: {} segundos desde último chequeo (intervalo: {} segundos)",
            time_since_last_check, interval_secs
        );
    } else {
        debug!(
            "Aún no es tiempo: {} segundos hasta el próximo chequeo",
            interval_secs.saturating_sub(time_since_last_check)
        );
    }

    should_check
}

/// Marca que se acaba de hacer un chequeo exitoso.
/// Actualiza `last_check_timestamp` al tiempo actual.
pub fn mark_check_completed(state: &mut UpdateState) {
    state.last_check_timestamp = get_current_timestamp();
    state.failed_attempts = 0;
    info!(
        "Chequeo de actualización completado ({})",
        format_timestamp(state.last_check_timestamp)
    );
}

/// Marca que hay una actualización pendiente de aplicar.
/// La actualización se aplicará al siguiente reinicio/ciclo.
pub fn mark_update_pending(
    state: &mut UpdateState,
    version: String,
    path: Option<&Path>,
) -> Result<()> {
    state.pending_update = true;
    state.pending_version = Some(version.clone());
    state.pending_marked_at = Some(get_current_timestamp());
    info!(
        "Actualización marcada como pendiente: v{} (se aplicará en siguiente ciclo)",
        version
    );
    save_state(state, path)?;
    Ok(())
}

/// Marca que una actualización fue completada exitosamente.
pub fn mark_update_completed(state: &mut UpdateState, path: Option<&Path>) -> Result<()> {
    let now = get_current_timestamp();
    state.last_successful_update = Some(now);
    state.pending_update = false;
    state.pending_version = None;
    state.pending_marked_at = None;
    state.failed_attempts = 0;
    state.last_error = None;
    info!(
        "Actualización completada exitosamente ({})",
        format_timestamp(now)
    );
    save_state(state, path)?;
    Ok(())
}

/// Registra un intento fallido de actualización.
pub fn mark_update_failed(
    state: &mut UpdateState,
    error_message: String,
    path: Option<&Path>,
) -> Result<()> {
    state.failed_attempts += 1;
    state.last_error = Some(error_message.clone());

    // Reintentar hasta 3 veces antes de desistir
    if state.failed_attempts >= 3 {
        state.pending_update = false;
        state.pending_version = None;
        info!(
            "Actualización descartada después de {} intentos fallidos. Error: {}",
            state.failed_attempts, error_message
        );
    } else {
        info!(
            "Intento de actualización #{} falló: {}. Reintentaremos.",
            state.failed_attempts, error_message
        );
    }

    save_state(state, path)?;
    Ok(())
}

/// Limpia el estado de actualización pendiente (después de aplicarla correctamente).
pub fn clear_pending_update(state: &mut UpdateState, path: Option<&Path>) -> Result<()> {
    state.pending_update = false;
    state.pending_version = None;
    state.pending_marked_at = None;
    state.failed_attempts = 0;
    state.last_error = None;
    save_state(state, path)?;
    Ok(())
}

/// Formatea un timestamp UNIX a un string legible.
fn format_timestamp(secs: u64) -> String {
    let datetime = chrono::DateTime::<chrono::Utc>::from(
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs),
    );
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_should_check_now_first_time() {
        let state = UpdateState::default();
        assert!(should_check_now(&state, Some(24)));
    }

    #[test]
    fn test_should_check_now_not_yet() {
        let mut state = UpdateState::default();
        state.last_check_timestamp = get_current_timestamp() - 3600; // Hace 1 hora
        assert!(!should_check_now(&state, Some(24))); // 24 horas = no debería chequear
    }

    #[test]
    fn test_should_check_now_interval_passed() {
        let mut state = UpdateState::default();
        state.last_check_timestamp = get_current_timestamp() - (26 * 3600); // Hace 26 horas
        assert!(should_check_now(&state, Some(24))); // Debería chequear
    }

    #[test]
    fn test_mark_check_completed() {
        let mut state = UpdateState::default();
        mark_check_completed(&mut state);
        assert!(state.last_check_timestamp > 0);
        assert_eq!(state.failed_attempts, 0);
    }

    #[test]
    fn test_mark_update_pending() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state.json");

        let mut state = UpdateState::default();
        mark_update_pending(&mut state, "2.1.0".to_string(), Some(&path)).unwrap();
        assert!(state.pending_update);
        assert_eq!(state.pending_version, Some("2.1.0".to_string()));

        // Verificar que se guardó
        let loaded = load_state(Some(&path)).unwrap();
        assert!(loaded.pending_update);
        assert_eq!(loaded.pending_version, Some("2.1.0".to_string()));
    }

    #[test]
    fn test_mark_update_failed_retry_logic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state.json");

        let mut state = UpdateState::default();
        state.pending_update = true;

        // Intento 1
        mark_update_failed(&mut state, "Error 1".to_string(), Some(&path)).unwrap();
        assert!(state.pending_update); // Aún pendiente
        assert_eq!(state.failed_attempts, 1);

        // Intento 2
        mark_update_failed(&mut state, "Error 2".to_string(), Some(&path)).unwrap();
        assert!(state.pending_update);
        assert_eq!(state.failed_attempts, 2);

        // Intento 3 (should clear pending)
        mark_update_failed(&mut state, "Error 3".to_string(), Some(&path)).unwrap();
        assert!(!state.pending_update); // Descartada
        assert_eq!(state.failed_attempts, 3);
    }
}
