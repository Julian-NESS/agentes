// ==============================================================================
// NESS Relay v2.0.3 — Perfil Juniper EX/QFX (Switches)
// ==============================================================================
//
// MIBs usados:
//   - JUNIPER-MIB (1.3.6.1.4.1.2636): jnxOperatingTable para CPU, memoria, temp
//   - HOST-RESOURCES-MIB: como fallback para CPU
//
// OIDs globales — compatibles con toda la familia EX (EX2300, EX3400, EX4300,
// EX4600) y QFX (QFX5100, QFX5200, QFX10002) bajo Junos OS.
//
// NOTA: jnxOperatingTable indexa por slot. El índice 9.1.0.0 corresponde
// al Routing Engine (RE0) que es el dato global del equipo.
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::calculate_percentage;

pub struct JuniperExProfile;

impl JuniperExProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for JuniperExProfile {
    fn vendor(&self) -> &str { "juniper_ex" }
    fn vendor_display_name(&self) -> &str { "Juniper EX/QFX" }
    fn device_type(&self) -> &str { "switch" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // JUNIPER-MIB: jnxOperatingCPU — porcentaje uso CPU del Routing Engine
        // Índice 9.1.0.0 = RE0 (slot principal)
        m.insert("jnxOperatingCPU".into(), "1.3.6.1.4.1.2636.3.1.13.1.8.9.1.0.0".into());
        // Tabla completa para walk si hay múltiples slots
        m.insert("jnxOperatingCPUTable".into(), "1.3.6.1.4.1.2636.3.1.13.1.8".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // JUNIPER-MIB: jnxOperatingBuffer — porcentaje memoria usada del RE
        m.insert("jnxOperatingBuffer".into(), "1.3.6.1.4.1.2636.3.1.13.1.11.9.1.0.0".into());
        // jnxOperatingMemory — memoria total instalada en MB
        m.insert("jnxOperatingMemory".into(), "1.3.6.1.4.1.2636.3.1.13.1.15.9.1.0.0".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        // hrStorageTable para almacenamiento — Junos lo soporta
        let mut m = HashMap::new();
        m.insert("hrStorageTable".into(),           "1.3.6.1.2.1.25.2.3".into());
        m.insert("hrStorageDescr".into(),           "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(), "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),            "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),            "1.3.6.1.2.1.25.2.3.1.6".into());
        m
    }

    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Temperatura del Routing Engine
        m.insert("jnxOperatingTemp".into(), "1.3.6.1.4.1.2636.3.1.13.1.7.9.1.0.0".into());
        // Estado de alarmas
        m.insert("jnxYellowAlarmCount".into(), "1.3.6.1.4.1.2636.3.4.1.2.1.0".into());
        m.insert("jnxRedAlarmCount".into(),    "1.3.6.1.4.1.2636.3.4.1.1.1.0".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let cpu_pct = raw.get("jnxOperatingCPU")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        json!({
            "cpu_usage_percent": cpu_pct,
            "cpu_cores": [{ "core": "RE0", "usage": cpu_pct }],
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let mem_pct = raw.get("jnxOperatingBuffer")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        let total_mb = raw.get("jnxOperatingMemory")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        let used_mb = total_mb * mem_pct / 100.0;
        let free_mb = (total_mb - used_mb).max(0.0);

        json!({
            "total_gb": (total_mb / 1024.0 * 100.0).round() / 100.0,
            "used_gb":  (used_mb  / 1024.0 * 100.0).round() / 100.0,
            "free_gb":  (free_mb  / 1024.0 * 100.0).round() / 100.0,
            "usage_percent": mem_pct,
        })
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        let mut disks = Vec::new();
        for (idx, entry) in raw {
            let descr = entry.get("hrStorageDescr").map(|v| v.as_string())
                .unwrap_or_else(|| format!("storage-{}", idx));
            let descr_lower = descr.to_lowercase();
            if descr_lower.contains("memory") || descr_lower.contains("swap") {
                continue;
            }
            let units = entry.get("hrStorageAllocationUnits")
                .and_then(|v| v.as_i64()).unwrap_or(4096) as f64;
            let size = entry.get("hrStorageSize").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used = entry.get("hrStorageUsed").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let total_b = size * units;
            let used_b  = used * units;
            if total_b > 0.0 {
                disks.push(json!({
                    "mount": descr,
                    "total_gb": (total_b / 1_073_741_824.0 * 100.0).round() / 100.0,
                    "used_gb":  (used_b  / 1_073_741_824.0 * 100.0).round() / 100.0,
                    "free_gb":  ((total_b - used_b) / 1_073_741_824.0 * 100.0).round() / 100.0,
                    "usage_percent": calculate_percentage(used_b, total_b),
                }));
            }
        }
        json!(disks)
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Temperatura del RE
        // -----------------------------------------------------------------------
        let temp = client.get("1.3.6.1.4.1.2636.3.1.13.1.7.9.1.0.0").await;
        if let Some(v) = temp.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("temperature_re_c".into(), json!(v));
        }

        // -----------------------------------------------------------------------
        // Alarmas
        // -----------------------------------------------------------------------
        let yellow = client.get("1.3.6.1.4.1.2636.3.4.1.2.1.0").await;
        let red    = client.get("1.3.6.1.4.1.2636.3.4.1.1.1.0").await;

        data.insert("yellow_alarms".into(), json!(
            yellow.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0)
        ));
        data.insert("red_alarms".into(), json!(
            red.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0)
        ));

        // -----------------------------------------------------------------------
        // Tabla de componentes (FPCs/PICs — temperaturas y CPU por slot)
        // -----------------------------------------------------------------------
        let (slot_descr, _) = client.bulk("1.3.6.1.4.1.2636.3.1.13.1.5", 30).await;
        let (slot_temp, _)  = client.bulk("1.3.6.1.4.1.2636.3.1.13.1.7", 30).await;
        let (slot_cpu, _)   = client.bulk("1.3.6.1.4.1.2636.3.1.13.1.8", 30).await;

        if !slot_descr.is_empty() {
            let temp_map: HashMap<String, i64> = slot_temp.into_iter()
                .filter_map(|(oid, v)| {
                    let suffix = oid.strip_prefix("1.3.6.1.4.1.2636.3.1.13.1.7.")?.to_string();
                    Some((suffix, v.as_i64().unwrap_or(0)))
                })
                .collect();
            let cpu_map: HashMap<String, i64> = slot_cpu.into_iter()
                .filter_map(|(oid, v)| {
                    let suffix = oid.strip_prefix("1.3.6.1.4.1.2636.3.1.13.1.8.")?.to_string();
                    Some((suffix, v.as_i64().unwrap_or(0)))
                })
                .collect();

            let components: Vec<serde_json::Value> = slot_descr.into_iter()
                .filter_map(|(oid, v)| {
                    let suffix = oid.strip_prefix("1.3.6.1.4.1.2636.3.1.13.1.5.")?.to_string();
                    Some(json!({
                        "name": v.as_string(),
                        "temperature_c": temp_map.get(&suffix).copied().unwrap_or(0),
                        "cpu_percent": cpu_map.get(&suffix).copied().unwrap_or(0),
                    }))
                })
                .collect();
            data.insert("components".into(), json!(components));
        }

        debug!("Juniper EX: temp={:?}°C, yellow_alarms={}, red_alarms={}, components={}",
            data.get("temperature_re_c"),
            data.get("yellow_alarms").and_then(|v| v.as_i64()).unwrap_or(0),
            data.get("red_alarms").and_then(|v| v.as_i64()).unwrap_or(0),
            data.get("components").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        // Juniper enterprise OID — para switches EX/QFX
        // EX series: 1.3.6.1.4.1.2636.1.51.* (EX2300, etc.)
        // QFX series: 1.3.6.1.4.1.2636.1.62.*
        sys_oid.starts_with("1.3.6.1.4.1.2636.1.51") ||
        sys_oid.starts_with("1.3.6.1.4.1.2636.1.62")
    }
}
