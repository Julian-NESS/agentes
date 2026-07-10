// ==============================================================================
// NESS Relay v2.0.0 — Analizador de seguridad
// Equivalente Python: analyzers/security_analyzer.py
// ==============================================================================
//
// Genera alertas de seguridad basadas en:
//   - Conexiones TCP excesivas
//   - Alta tasa de retransmisiones TCP
//   - Intentos de autenticación SNMP fallidos
//   - Tasa de fragmentación IP alta
//   - ICMP echos excesivos (posible reconocimiento/escaneo)
// ==============================================================================

use serde_json::json;

// Umbrales TCP
const TCP_CONN_WARNING: i64   = 5_000;
const TCP_CONN_CRITICAL: i64  = 10_000;
const TCP_FAIL_CRITICAL: i64  = 1_000;
const TCP_RETRANS_WARNING: f64  = 5.0;   // %
const TCP_RETRANS_CRITICAL: f64 = 10.0;  // %

// Umbrales SNMP (autenticación)
const SNMP_BAD_COM_WARNING: i64  = 0;   // cualquier fallo es warning
const SNMP_BAD_COM_CRITICAL: i64 = 10;

// Umbrales IP fragmentación
const IP_FRAG_WARNING: f64  = 5.0;  // % de paquetes fragmentados
const IP_FRAG_CRITICAL: f64 = 10.0;

// Umbrales ICMP (reconocimiento)
const ICMP_ECHO_CRITICAL: i64 = 1_000;

/// Analiza los datos de seguridad y genera alertas.
/// `security_data` — salida del security_collector
/// Retorna diccionario con el análisis (mismo formato que Python).
pub fn analyze(security_data: &serde_json::Value) -> serde_json::Value {
    let mut alerts: Vec<serde_json::Value> = Vec::new();
    let mut warnings: Vec<serde_json::Value> = Vec::new();

    // -----------------------------------------------------------------------
    // Conexiones TCP activas
    // -----------------------------------------------------------------------
    let tcp = security_data.get("tcp").cloned().unwrap_or_default();
    let tcp_curr = tcp.get("curr_estab").and_then(|v| v.as_i64()).unwrap_or(0);

    if tcp_curr >= TCP_CONN_CRITICAL {
        alerts.push(json!({
            "type": "tcp_connections",
            "level": "critical",
            "message": format!("Conexiones TCP críticas: {}", tcp_curr),
            "value": tcp_curr,
            "threshold": TCP_CONN_CRITICAL,
        }));
    } else if tcp_curr >= TCP_CONN_WARNING {
        warnings.push(json!({
            "type": "tcp_connections",
            "level": "warning",
            "message": format!("Conexiones TCP elevadas: {}", tcp_curr),
            "value": tcp_curr,
            "threshold": TCP_CONN_WARNING,
        }));
    }

    // Fallos de conexión TCP
    let tcp_fail = tcp.get("attempt_fails").and_then(|v| v.as_i64()).unwrap_or(0);
    if tcp_fail >= TCP_FAIL_CRITICAL {
        alerts.push(json!({
            "type": "tcp_failures",
            "level": "critical",
            "message": format!("Fallos de conexión TCP: {}", tcp_fail),
            "value": tcp_fail,
            "threshold": TCP_FAIL_CRITICAL,
        }));
    }

    // Tasa de retransmisión TCP
    let retrans_rate = tcp.get("retransmission_rate_pct")
        .and_then(|v| v.as_f64()).unwrap_or(0.0);
    if retrans_rate >= TCP_RETRANS_CRITICAL {
        alerts.push(json!({
            "type": "tcp_retransmission",
            "level": "critical",
            "message": format!("Tasa de retransmisión TCP crítica: {:.2}%", retrans_rate),
            "value": retrans_rate,
            "threshold": TCP_RETRANS_CRITICAL,
        }));
    } else if retrans_rate >= TCP_RETRANS_WARNING {
        warnings.push(json!({
            "type": "tcp_retransmission",
            "level": "warning",
            "message": format!("Tasa de retransmisión TCP elevada: {:.2}%", retrans_rate),
            "value": retrans_rate,
            "threshold": TCP_RETRANS_WARNING,
        }));
    }

    // -----------------------------------------------------------------------
    // SNMP — intentos de autenticación fallidos
    // -----------------------------------------------------------------------
    let snmp_stats = security_data.get("snmp_stats").cloned().unwrap_or_default();
    let bad_community = snmp_stats.get("bad_community_names")
        .and_then(|v| v.as_i64()).unwrap_or(0);

    if bad_community >= SNMP_BAD_COM_CRITICAL {
        alerts.push(json!({
            "type": "snmp_security",
            "level": "critical",
            "message": format!("Fallos de autenticación SNMP críticos: {}", bad_community),
            "value": bad_community,
            "threshold": SNMP_BAD_COM_CRITICAL,
        }));
    } else if bad_community > SNMP_BAD_COM_WARNING {
        warnings.push(json!({
            "type": "snmp_security",
            "level": "warning",
            "message": format!("Fallos de autenticación SNMP: {}", bad_community),
            "value": bad_community,
            "threshold": SNMP_BAD_COM_WARNING,
        }));
    }

    // -----------------------------------------------------------------------
    // IP — fragmentación
    // -----------------------------------------------------------------------
    let ip = security_data.get("ip").cloned().unwrap_or_default();
    let frag_rate = ip.get("fragmentation_rate_pct")
        .and_then(|v| v.as_f64()).unwrap_or(0.0);

    if frag_rate >= IP_FRAG_CRITICAL {
        alerts.push(json!({
            "type": "ip_fragmentation",
            "level": "critical",
            "message": format!("Tasa de fragmentación IP crítica: {:.2}%", frag_rate),
            "value": frag_rate,
            "threshold": IP_FRAG_CRITICAL,
        }));
    } else if frag_rate >= IP_FRAG_WARNING {
        warnings.push(json!({
            "type": "ip_fragmentation",
            "level": "warning",
            "message": format!("Tasa de fragmentación IP elevada: {:.2}%", frag_rate),
            "value": frag_rate,
            "threshold": IP_FRAG_WARNING,
        }));
    }

    // -----------------------------------------------------------------------
    // ICMP — posible escaneo/reconocimiento
    // -----------------------------------------------------------------------
    let icmp = security_data.get("icmp").cloned().unwrap_or_default();
    let icmp_echos = icmp.get("in_echos").and_then(|v| v.as_i64()).unwrap_or(0);

    if icmp_echos >= ICMP_ECHO_CRITICAL {
        alerts.push(json!({
            "type": "icmp_reconnaissance",
            "level": "critical",
            "message": format!("ICMP Echo requests excesivos: {} (posible escaneo)", icmp_echos),
            "value": icmp_echos,
            "threshold": ICMP_ECHO_CRITICAL,
        }));
    }

    let status = if !alerts.is_empty() {
        "critical"
    } else if !warnings.is_empty() {
        "warning"
    } else {
        "ok"
    };

    json!({
        "timestamp": crate::utils::helpers::now_iso(),
        "total_alerts": alerts.len(),
        "total_warnings": warnings.len(),
        "alerts": alerts,
        "warnings": warnings,
        "security_status": status,
    })
}
