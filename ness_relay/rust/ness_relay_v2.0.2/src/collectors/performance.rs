// ==============================================================================
// NESS Relay v2.0.0 — Colector de rendimiento (CPU, Memoria, Disco)
// Equivalente Python: collectors/performance_collector.py
// ==============================================================================
//
// Delega los OIDs y la normalización al perfil del vendor.
// Soporta tablas SNMP (GETBULK) para discos.
// ==============================================================================

use std::collections::HashMap;
use std::sync::Arc;
use serde_json::json;
use tracing::{debug, warn};

use crate::profiles::base::DeviceProfile;
use crate::snmp::SnmpClient;
use crate::snmp::types::SnmpValue;
use crate::utils::helpers::now_iso;

/// Recolecta datos de CPU, memoria y disco del dispositivo.
/// Usa los OIDs definidos por el perfil del vendor.
pub async fn collect(
    client: &SnmpClient,
    profile: &Arc<dyn DeviceProfile>,
) -> serde_json::Value {
    let mut result = serde_json::Map::new();
    result.insert("collection_timestamp".into(), json!(now_iso()));

    // -----------------------------------------------------------------------
    // CPU
    // -----------------------------------------------------------------------
    let cpu_oids = profile.get_cpu_oids();
    let mut cpu_raw: HashMap<String, SnmpValue> = HashMap::new();

    for (name, oid) in &cpu_oids {
        if name.to_lowercase().contains("table") {
            // Es una tabla → usar GETBULK
            let (entries, err) = client.bulk(oid, 32).await;
            if let Some(e) = err {
                warn!("CPU table GETBULK error para {}: {}", name, e);
            }
            for (resp_oid, value) in entries {
                // Extraer índice numérico del OID
                let idx = resp_oid.rsplit('.').next().unwrap_or("0").to_string();
                cpu_raw.insert(idx, value);
            }
        } else {
            let res = client.get(oid).await;
            if let Some(v) = res.value {
                cpu_raw.insert(name.clone(), v);
            }
        }
    }
    let cpu_data = profile.normalize_cpu_data(&cpu_raw);
    result.insert("cpu".into(), cpu_data);

    // -----------------------------------------------------------------------
    // Memoria
    // -----------------------------------------------------------------------
    let mem_oids = profile.get_memory_oids();
    let mut mem_raw: HashMap<String, SnmpValue> = HashMap::new();

    for (name, oid) in &mem_oids {
        if name.to_lowercase().contains("table") {
            let (entries, _) = client.bulk(oid, 32).await;
            for (resp_oid, value) in entries {
                let idx = resp_oid.rsplit('.').next().unwrap_or("0").to_string();
                mem_raw.insert(idx, value);
            }
        } else {
            let res = client.get(oid).await;
            if let Some(v) = res.value {
                mem_raw.insert(name.clone(), v);
            }
        }
    }
    let mem_data = profile.normalize_memory_data(&mem_raw);
    result.insert("memory".into(), mem_data);

    // -----------------------------------------------------------------------
    // Disco (tabla hrStorageTable u equivalente, o OIDs escalares)
    // -----------------------------------------------------------------------
    let disk_oids = profile.get_disk_oids();
    let mut disk_tables: HashMap<String, HashMap<String, SnmpValue>> = HashMap::new();

    // Detectar si hay un OID de tabla (clave que contiene "Table")
    let has_table = disk_oids.iter().any(|(k, _)| k.to_lowercase().contains("table"));

    if has_table {
        // Modo tabla: hacer GETBULK sobre cada sub-OID de la tabla
        if let Some(table_oid) = disk_oids.values().find(|oid| {
            disk_oids.iter().any(|(k, v)| k.to_lowercase().contains("table") && v == *oid)
        }).or_else(|| disk_oids.get("hrStorageTable")).or_else(|| disk_oids.get("dskTable")) {
            let _ = table_oid; // used only for conditional entry
            for (col_name, col_oid) in &disk_oids {
                if col_name.to_lowercase().contains("table") {
                    continue; // Saltar el OID de la tabla en sí
                }
                let (entries, _) = client.bulk(col_oid, 32).await;
                for (resp_oid, value) in entries {
                    let idx = resp_oid.rsplit('.').next().unwrap_or("0").to_string();
                    disk_tables
                        .entry(idx)
                        .or_default()
                        .insert(col_name.clone(), value);
                }
            }
        }
    } else if !disk_oids.is_empty() {
        // Modo escalar: OIDs individuales (ej. Fortinet fgSysDiskUsage/fgSysDiskCapacity)
        let mut scalar_entry: HashMap<String, SnmpValue> = HashMap::new();
        for (name, oid) in &disk_oids {
            let res = client.get(oid).await;
            if let Some(v) = res.value {
                scalar_entry.insert(name.clone(), v);
            }
        }
        if !scalar_entry.is_empty() {
            disk_tables.insert("0".to_string(), scalar_entry);
        }
    }

    let disk_data = profile.normalize_disk_data(&disk_tables);
    result.insert("disks".into(), disk_data);

    debug!("Rendimiento recolectado — CPU: {:?}, Memoria: {:?}, Discos: {} entradas",
        result.get("cpu").and_then(|v| v.get("cpu_usage_percent")),
        result.get("memory").and_then(|v| v.get("usage_percent")),
        disk_tables.len()
    );

    // Aplicar post-proceso del perfil (ej. corregir memoria de vendor data)
    let mut out = json!(result);
    out = profile.post_process_performance(out);
    out
}
