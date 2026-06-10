// ==============================================================================
// NESS Relay v2.0.0 — Conversiones y utilidades numéricas
// Equivalente Python: utils/conversions.py
// ==============================================================================

/// Convierte kilobytes a gigabytes, redondeado a 2 decimales.
pub fn kb_to_gb(kb: f64) -> f64 {
    ((kb / 1_048_576.0) * 100.0).round() / 100.0
}

/// Convierte megabytes a gigabytes.
pub fn mb_to_gb(mb: f64) -> f64 {
    ((mb / 1024.0) * 100.0).round() / 100.0
}

/// Convierte bytes a gigabytes.
pub fn bytes_to_gb(bytes: f64) -> f64 {
    ((bytes / 1_073_741_824.0) * 100.0).round() / 100.0
}

/// División segura que evita división por cero.
pub fn safe_division(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

/// Calcula porcentaje de forma segura.
/// Retorna 0.0 si el total es 0.
pub fn calculate_percentage(used: f64, total: f64) -> f64 {
    if total == 0.0 {
        return 0.0;
    }
    let pct = (used / total) * 100.0;
    (pct * 100.0).round() / 100.0
}

/// Formatea TimeTicks SNMP (centisegundos) a un dict con días/horas/min/seg.
/// Retorna un JSON Value con los campos.
pub fn format_uptime(centiseconds: u64) -> serde_json::Value {
    let total_seconds = centiseconds / 100;
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    serde_json::json!({
        "days": days,
        "hours": hours,
        "minutes": minutes,
        "seconds": seconds,
        "total_seconds": total_seconds,
        "human": format!(
            "{}d {}h {}m {}s",
            days, hours, minutes, seconds
        )
    })
}

/// Convierte un string a i64 de forma segura, retorna 0 si falla.
pub fn safe_int(value: &str) -> i64 {
    value.trim().parse::<i64>().unwrap_or(0)
}

/// Convierte un string a f64 de forma segura, retorna 0.0 si falla.
pub fn safe_float(value: &str) -> f64 {
    value.trim().parse::<f64>().unwrap_or(0.0)
}

/// Convierte un serde_json::Value a i64 usando el mejor método disponible.
pub fn json_to_i64(v: &serde_json::Value) -> i64 {
    match v {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
        serde_json::Value::String(s) => safe_int(s),
        _ => 0,
    }
}

/// Convierte un serde_json::Value a f64.
pub fn json_to_f64(v: &serde_json::Value) -> f64 {
    match v {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        serde_json::Value::String(s) => safe_float(s),
        _ => 0.0,
    }
}

/// Convierte velocidad de interface de bits/s a Mbps.
pub fn bps_to_mbps(bps: u64) -> f64 {
    (bps as f64) / 1_000_000.0
}

/// Formatea bytes a la mejor unidad (B, KB, MB, GB).
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1_048_576 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1_073_741_824 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    }
}

/// Calcula la tasa de error de interfaces (errores por total de paquetes).
pub fn interface_error_rate(errors: u64, total_packets: u64) -> f64 {
    if total_packets == 0 {
        return 0.0;
    }
    let rate = (errors as f64 / total_packets as f64) * 100.0;
    (rate * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kb_to_gb() {
        assert_eq!(kb_to_gb(1_048_576.0), 1.0);
    }

    #[test]
    fn test_format_uptime_zero() {
        let v = format_uptime(0);
        assert_eq!(v["days"], 0);
    }

    #[test]
    fn test_format_uptime_1day() {
        // 1 día = 86400 segundos = 8640000 centisegundos
        let v = format_uptime(8640000);
        assert_eq!(v["days"], 1);
        assert_eq!(v["hours"], 0);
    }

    #[test]
    fn test_calculate_percentage() {
        assert_eq!(calculate_percentage(75.0, 100.0), 75.0);
        assert_eq!(calculate_percentage(0.0, 0.0), 0.0);
    }
}
