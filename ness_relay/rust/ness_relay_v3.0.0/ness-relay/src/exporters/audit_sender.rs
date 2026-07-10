// ==============================================================================
// NESS Relay v2.1.0 — Enviador de auditoría SSH a endpoints dedicados
// ==============================================================================
//
// Envía los payloads de vulnerabilidades y controles CIS a endpoints SEPARADOS
// del flujo SNMP principal:
//   - POST {server_url}/api/relay/audit/vulnerabilities/
//   - POST {server_url}/api/relay/audit/cis/
//
// Esto desacopla la auditoría SSH del flujo de telemetría SNMP, permite
// versionar cada schema independientemente y simplifica el manejo de errores
// (un fallo en vulnerabilidades no afecta al envío de CIS ni viceversa).
//
// Header: Authorization: Token {api_token}
// Body:   El JSON crudo que produce `audit_runner::run_audit_phases()`,
//         con la salvedad de inyectar `device_hostname` si el agente lo
//         perdió (compatibilidad con el schema v1).
// ==============================================================================

use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::server_sender::send;

/// Tipo de payload audit. Determina el endpoint y el comportamiento.
#[derive(Debug, Clone, Copy)]
pub enum AuditKind {
    /// Vulnerabilidades CVE (`ness-relay/vulnerabilities/v1`)
    Vulnerabilities,
    /// Controles CIS (`ness-relay/cis-compliance/v1`)
    Cis,
}

impl AuditKind {
    /// Slug del endpoint al que se envía.
    fn endpoint_slug(&self) -> &'static str {
        match self {
            AuditKind::Vulnerabilities => "vulnerabilities",
            AuditKind::Cis => "cis",
        }
    }

    /// Etiqueta humana para logs.
    fn label(&self) -> &'static str {
        match self {
            AuditKind::Vulnerabilities => "vulnerabilidades",
            AuditKind::Cis => "CIS",
        }
    }
}

/// Construye la URL completa del endpoint audit.
///
/// `server_url` es la URL base del servidor (ej: `http://172.206.0.217:8080`
/// o `https://cloud.nesshq.com`). El path del endpoint audit se compone
/// automáticamente.
fn build_audit_url(server_url: &str, kind: AuditKind) -> String {
    let base = server_url.trim_end_matches('/');
    format!("{}/api/relay/audit/{}/", base, kind.endpoint_slug())
}

/// Inyecta en el payload audit todos los campos de identificación del
/// dispositivo:
///   - `real_hostname` (sys_name SNMP) → `device_hostname` (prioridad máxima
///     porque es lo que el servidor guardó en la BD tras el POST SNMP).
///   - `device_id` (slug local, ej: "fortinet_1") → clave opcional útil para
///     logs.
///   - `ip_address` → para que el servidor pueda resolver el dispositivo por
///     IP como fallback cuando el sysName difiere (NAT, DNS dinámico, etc.).
///
/// El orden de prioridad de `device_hostname` es:
///   1. Si el payload ya trae `device_hostname`, se respeta (el audit_runner
///      pudo haberlo extraído del SSH).
///   2. Si NO trae, se inyecta `real_hostname` (sysName SNMP).
///   3. Como última red de seguridad, `device_id` (slug local).
fn ensure_device_identification(
    payload: &mut Value,
    device_id: &str,
    real_hostname: &str,
    ip_address: &str,
) {
    let obj = match payload.as_object_mut() {
        Some(o) => o,
        None => return,
    };
    if !obj.contains_key("device_hostname") || obj["device_hostname"].is_null() {
        let chosen = if !real_hostname.is_empty() {
            real_hostname.to_string()
        } else {
            device_id.to_string()
        };
        obj.insert("device_hostname".to_string(), Value::String(chosen));
    }
    if !obj.contains_key("ip_address") {
        obj.insert(
            "ip_address".to_string(),
            Value::String(ip_address.to_string()),
        );
    }
    if !obj.contains_key("device_id") {
        obj.insert(
            "device_id".to_string(),
            Value::String(device_id.to_string()),
        );
    }
}

/// Envía un payload de auditoría al endpoint dedicado correspondiente.
///
/// `payload` debe ser el JSON crudo que retorna `audit_runner::run_audit_phases()`,
/// esto es, un objeto con al menos la clave `findings` (lista de hallazgos).
///
/// Si el payload está vacío o no tiene findings, esta función retorna `Ok(())`
/// sin enviar nada (es un caso normal cuando no hay credenciales SSH).
///
/// # Errores
/// - Devuelve `Err` sólo en errores de red o del servidor 5xx.
/// - Para respuestas 4xx (payload inválido, dispositivo no encontrado) **no**
///   propaga el error; sólo loguea un warning, ya que la auditoría es
///   best-effort y no debe romper el ciclo de recolección principal.
pub async fn send_audit_payload(
    server_url: &str,
    api_token: &str,
    kind: AuditKind,
    payload: &Value,
    device_id: &str,
    real_hostname: &str,
    ip_address: &str,
) -> Result<()> {
    // 1. Si el payload no es un objeto JSON o no tiene `findings`, skip.
    if !payload.is_object() {
        debug!(
            "[{}] payload audit no es objeto JSON, omitiendo envío",
            device_id
        );
        return Ok(());
    }
    let findings = payload
        .get("findings")
        .and_then(|f| f.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    // Si no hay hallazgos y no hay started_at (es decir, no se ejecutó el audit),
    // también saltamos para no ensuciar el servidor con payloads vacíos.
    if findings == 0 && !payload.get("started_at").is_some() {
        debug!(
            "[{}] audit {} omitido (sin credenciales SSH o sin hallazgos)",
            device_id,
            kind.label()
        );
        return Ok(());
    }

    // 2. Clonar el payload y garantizar `device_hostname` (= sysName SNMP),
    //    `ip_address` y `device_id` (= slug local).
    let mut payload_clone = payload.clone();
    ensure_device_identification(
        &mut payload_clone,
        device_id,
        real_hostname,
        ip_address,
    );

    // 3. Construir URL y enviar.
    let url = build_audit_url(server_url, kind);
    info!(
        "[{} → sysName='{}' ip={}] Enviando {} hallazgos de {} a {}",
        device_id,
        real_hostname,
        ip_address,
        findings,
        kind.label(),
        url
    );

    match send(&url, api_token, &payload_clone).await {
        Ok(()) => {
            info!(
                "[{} → '{}'] {} audit {} enviado correctamente",
                device_id,
                real_hostname,
                findings,
                kind.label()
            );
            Ok(())
        }
        Err(e) => {
            // Si es 4xx (e.g. 404 dispositivo no encontrado), no rompemos el
            // flujo principal. Sólo warn. Los 5xx sí los propagamos como
            // error para que el operador los vea.
            let msg = e.to_string();
            if msg.contains("HTTP 4") {
                warn!(
                    "[{} → '{}'] Servidor rechazó audit {}: {} (continuando)",
                    device_id,
                    real_hostname,
                    kind.label(),
                    msg
                );
                Ok(())
            } else {
                Err(anyhow!(
                    "Fallo enviando audit {} a {}: {}",
                    kind.label(),
                    url,
                    msg
                ))
            }
        }
    }
}

/// Helper para construir un payload "vacío" cuando el audit no se ejecutó
/// (e.g. sin credenciales SSH). Mantiene compatibilidad con el servidor.
#[allow(dead_code)]
pub fn empty_audit_payload(kind: AuditKind) -> Value {
    let key = match kind {
        AuditKind::Vulnerabilities => "vulnerabilities",
        AuditKind::Cis => "cis_compliance",
    };
    serde_json::json!({
        key: {
            "schema": match kind {
                AuditKind::Vulnerabilities => "ness-relay/vulnerabilities/v1",
                AuditKind::Cis => "ness-relay/cis-compliance/v1",
            },
            "findings": [],
            "is_clean": true,
            "counts": {
                "total": 0,
                "critical": 0, "high": 0, "medium": 0, "low": 0, "info": 0,
                "passed": 0, "failed": 0, "manual": 0, "errors": 0,
                "total_checks": 0, "compliance_score": 100,
            },
        }
    })
}