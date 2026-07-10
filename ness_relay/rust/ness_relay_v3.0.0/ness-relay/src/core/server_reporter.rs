// ==============================================================================
// NESS Relay v2.0.0 — Reportador de Actualizaciones
// Envía notificaciones al servidor NESS sobre el ciclo de vida de actualizaciones
// ==============================================================================

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Estructura para reportar estado de actualización al servidor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatusReport {
    /// Timestamp UNIX en segundos
    pub timestamp: u64,
    /// Versión actual del agente
    pub current_version: String,
    /// Nueva versión disponible (si aplica)
    pub new_version: Option<String>,
    /// Estado de la actualización
    pub status: UpdateStatus,
    /// Mensaje descriptivo
    pub message: String,
    /// Detalles técnicos (si es error)
    pub details: Option<String>,
}

/// Estados posibles de una actualización
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum UpdateStatus {
    /// Se detectó una nueva versión disponible
    #[serde(rename = "AVAILABLE")]
    Available,
    /// Se inició la descarga de la actualización
    #[serde(rename = "STARTED")]
    Started,
    /// La actualización se completó exitosamente
    #[serde(rename = "COMPLETED")]
    Completed,
    /// La actualización falló
    #[serde(rename = "FAILED")]
    Failed,
    /// Actualización pendiente (waiting for restart)
    #[serde(rename = "PENDING")]
    Pending,
}

/// Obtiene el timestamp actual en segundos desde UNIX_EPOCH.
fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Envía un reporte de actualización al servidor NESS.
///
/// # Arguments
/// * `report_url` - URL del endpoint de reportes en el servidor
/// * `api_token` - Token de autenticación API
/// * `report` - Estructura con la información a reportar
///
/// # Ejemplo
/// ```ignore
/// let report = UpdateStatusReport {
///     timestamp: get_current_timestamp(),
///     current_version: "2.0.0".to_string(),
///     new_version: Some("2.1.0".to_string()),
///     status: UpdateStatus::Available,
///     message: "Nueva versión disponible".to_string(),
///     details: None,
/// };
/// send_report("https://..../api/relay/update-status", "token123", &report).await?;
/// ```
pub async fn send_report(
    report_url: &str,
    api_token: &str,
    report: &UpdateStatusReport,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .use_rustls_tls()
        .build()?;

    debug!(
        "Enviando reporte de actualización al servidor: {}",
        report_url
    );

    let response = client
        .post(report_url)
        .header("Authorization", format!("Token {}", api_token))
        .header("Content-Type", "application/json")
        .json(report)
        .send()
        .await
        .context("Error enviando reporte al servidor")?;

    if response.status().is_success() {
        info!(
            "✓ Reporte enviado exitosamente (status: {})",
            response.status().as_u16()
        );
        Ok(())
    } else {
        warn!(
            "Servidor rechazó reporte (HTTP {}): {}",
            response.status().as_u16(),
            response.text().await.unwrap_or_default()
        );
        // No error fatal, logging solamente
        Ok(())
    }
}

/// Reporta que se detectó una nueva versión disponible.
pub async fn report_update_available(
    report_url: &str,
    api_token: &str,
    current_version: &str,
    new_version: &str,
) -> Result<()> {
    let report = UpdateStatusReport {
        timestamp: get_current_timestamp(),
        current_version: current_version.to_string(),
        new_version: Some(new_version.to_string()),
        status: UpdateStatus::Available,
        message: format!(
            "Nueva versión disponible: {} → {}",
            current_version, new_version
        ),
        details: None,
    };

    send_report(report_url, api_token, &report).await
}

/// Reporta que se inició la descarga de una actualización.
pub async fn report_update_started(
    report_url: &str,
    api_token: &str,
    current_version: &str,
    new_version: &str,
) -> Result<()> {
    let report = UpdateStatusReport {
        timestamp: get_current_timestamp(),
        current_version: current_version.to_string(),
        new_version: Some(new_version.to_string()),
        status: UpdateStatus::Started,
        message: format!("Iniciando actualización a v{}", new_version),
        details: None,
    };

    send_report(report_url, api_token, &report).await
}

/// Reporta que una actualización se completó exitosamente.
pub async fn report_update_completed(
    report_url: &str,
    api_token: &str,
    previous_version: &str,
    new_version: &str,
) -> Result<()> {
    let report = UpdateStatusReport {
        timestamp: get_current_timestamp(),
        current_version: new_version.to_string(),
        new_version: Some(new_version.to_string()),
        status: UpdateStatus::Completed,
        message: format!(
            "Actualización completada: {} → {} (reinicio pendiente)",
            previous_version, new_version
        ),
        details: None,
    };

    send_report(report_url, api_token, &report).await
}

/// Reporta que una actualización falló.
pub async fn report_update_failed(
    report_url: &str,
    api_token: &str,
    current_version: &str,
    error_message: &str,
) -> Result<()> {
    let report = UpdateStatusReport {
        timestamp: get_current_timestamp(),
        current_version: current_version.to_string(),
        new_version: None,
        status: UpdateStatus::Failed,
        message: "Error durante la actualización".to_string(),
        details: Some(error_message.to_string()),
    };

    send_report(report_url, api_token, &report).await
}

/// Reporta que hay una actualización pendiente de aplicar.
pub async fn report_update_pending(
    report_url: &str,
    api_token: &str,
    current_version: &str,
    new_version: &str,
) -> Result<()> {
    let report = UpdateStatusReport {
        timestamp: get_current_timestamp(),
        current_version: current_version.to_string(),
        new_version: Some(new_version.to_string()),
        status: UpdateStatus::Pending,
        message: format!(
            "Actualización pendiente: {} → {} (se aplicará en siguiente reinicio)",
            current_version, new_version
        ),
        details: None,
    };

    send_report(report_url, api_token, &report).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_structure_serialize() {
        let report = UpdateStatusReport {
            timestamp: 1704067200,
            current_version: "2.0.0".to_string(),
            new_version: Some("2.1.0".to_string()),
            status: UpdateStatus::Available,
            message: "Nueva versión disponible".to_string(),
            details: None,
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"current_version\":\"2.0.0\""));
        assert!(json.contains("\"status\":\"AVAILABLE\""));
        assert!(json.contains("\"new_version\":\"2.1.0\""));
    }

    #[test]
    fn test_status_enum_serialization() {
        let statuses = vec![
            (UpdateStatus::Available, "AVAILABLE"),
            (UpdateStatus::Started, "STARTED"),
            (UpdateStatus::Completed, "COMPLETED"),
            (UpdateStatus::Failed, "FAILED"),
            (UpdateStatus::Pending, "PENDING"),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert!(json.contains(expected));
        }
    }

    #[test]
    fn test_report_with_error_details() {
        let report = UpdateStatusReport {
            timestamp: 1704067200,
            current_version: "2.0.0".to_string(),
            new_version: None,
            status: UpdateStatus::Failed,
            message: "Error durante la actualización".to_string(),
            details: Some(
                "Hash SHA-256 inválido: esperado abc123, calculado def456".to_string(),
            ),
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"status\":\"FAILED\""));
        assert!(json.contains("Hash SHA-256 inválido"));
    }
}
