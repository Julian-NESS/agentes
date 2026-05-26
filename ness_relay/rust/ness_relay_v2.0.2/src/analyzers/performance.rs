// ==============================================================================
// NESS Relay v2.0.0 — Analizador de rendimiento
// Equivalente Python: analyzers/performance_analyzer.py
// ==============================================================================
//
// Genera alertas basadas en umbrales de CPU, memoria e interfaces.
// Niveles: "warning" y "critical"
// ==============================================================================

use serde_json::json;

// Umbrales de CPU
const CPU_WARNING: f64  = 80.0;
const CPU_CRITICAL: f64 = 90.0;

// Umbrales de memoria
const MEM_WARNING: f64  = 85.0;
const MEM_CRITICAL: f64 = 90.0;

// Umbrales de errores de interface (por contador)
const IF_ERR_WARNING: u64  = 100;
const IF_ERR_CRITICAL: u64 = 1000;

/// Analiza los datos de rendimiento y genera alertas.
/// `performance_data` — salida del performance_collector
/// `network_data`     — salida del network_collector  
/// Retorna diccionario con el análisis (mismo formato que Python).
pub fn analyze(
    performance_data: &serde_json::Value,
    network_data: &serde_json::Value,
) -> serde_json::Value {
    let mut alerts: Vec<serde_json::Value> = Vec::new();
    let mut warnings: Vec<serde_json::Value> = Vec::new();

    // -----------------------------------------------------------------------
    // CPU
    // -----------------------------------------------------------------------
    if let Some(cpu_pct) = performance_data
        .get("cpu")
        .and_then(|c| c.get("cpu_usage_percent"))
        .and_then(|v| v.as_f64())
    {
        if cpu_pct >= CPU_CRITICAL {
            alerts.push(json!({
                "type": "cpu_usage",
                "level": "critical",
                "message": format!("Uso de CPU crítico: {:.1}%", cpu_pct),
                "value": cpu_pct,
                "threshold": CPU_CRITICAL,
            }));
        } else if cpu_pct >= CPU_WARNING {
            warnings.push(json!({
                "type": "cpu_usage",
                "level": "warning",
                "message": format!("Uso de CPU elevado: {:.1}%", cpu_pct),
                "value": cpu_pct,
                "threshold": CPU_WARNING,
            }));
        }
    }

    // -----------------------------------------------------------------------
    // Memoria
    // -----------------------------------------------------------------------
    if let Some(mem_pct) = performance_data
        .get("memory")
        .and_then(|m| m.get("usage_percent"))
        .and_then(|v| v.as_f64())
    {
        if mem_pct >= MEM_CRITICAL {
            alerts.push(json!({
                "type": "memory_usage",
                "level": "critical",
                "message": format!("Uso de memoria crítico: {:.1}%", mem_pct),
                "value": mem_pct,
                "threshold": MEM_CRITICAL,
            }));
        } else if mem_pct >= MEM_WARNING {
            warnings.push(json!({
                "type": "memory_usage",
                "level": "warning",
                "message": format!("Uso de memoria elevado: {:.1}%", mem_pct),
                "value": mem_pct,
                "threshold": MEM_WARNING,
            }));
        }
    }

    // -----------------------------------------------------------------------
    // Interfaces con errores
    // -----------------------------------------------------------------------
    if let Some(interfaces) = network_data
        .get("interfaces")
        .and_then(|v| v.as_array())
    {
        for iface in interfaces {
            let name = iface.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let in_err  = iface.get("in_errors").and_then(|v| v.as_u64()).unwrap_or(0);
            let out_err = iface.get("out_errors").and_then(|v| v.as_u64()).unwrap_or(0);
            let total_err = in_err + out_err;

            if total_err >= IF_ERR_CRITICAL {
                alerts.push(json!({
                    "type": "interface_errors",
                    "level": "critical",
                    "message": format!("Interfaz {} con errores críticos: {} errores", name, total_err),
                    "interface": name,
                    "in_errors": in_err,
                    "out_errors": out_err,
                    "total_errors": total_err,
                    "threshold": IF_ERR_CRITICAL,
                }));
            } else if total_err >= IF_ERR_WARNING {
                warnings.push(json!({
                    "type": "interface_errors",
                    "level": "warning",
                    "message": format!("Interfaz {} con errores elevados: {} errores", name, total_err),
                    "interface": name,
                    "total_errors": total_err,
                    "threshold": IF_ERR_WARNING,
                }));
            }
        }
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
        "performance_status": status,
    })
}
