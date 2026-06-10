// ==============================================================================
// NESS Relay v2.0.3 — Perfil Juniper MX/SRX (Routers/Firewalls)
// ==============================================================================
//
// MIBs usados:
//   - JUNIPER-MIB (1.3.6.1.4.1.2636): jnxOperatingTable para CPU, memoria, temp
//   - JUNIPER-SRX-FLOW-MIB: sesiones de flujo (SRX)
//   - HOST-RESOURCES-MIB: disco como fallback
//
// OIDs globales — compatibles con MX (MX204, MX240, MX480, MX960) y
// SRX (SRX300, SRX1500, SRX4600) bajo Junos OS.
//
// NOTA: Comparte el mismo JUNIPER-MIB que JuniperExProfile, pero el
// device_type es "router" y añade métricas de flujo propias de SRX.
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::calculate_percentage;

pub struct JuniperMxProfile;

impl JuniperMxProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for JuniperMxProfile {
    fn vendor(&self) -> &str { "juniper_mx" }
    fn vendor_display_name(&self) -> &str { "Juniper MX/SRX" }
    fn device_type(&self) -> &str { "router" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // JUNIPER-MIB: jnxOperatingCPU del Routing Engine (RE0)
        m.insert("jnxOperatingCPU".into(), "1.3.6.1.4.1.2636.3.1.13.1.8.9.1.0.0".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // JUNIPER-MIB: jnxOperatingBuffer (porcentaje) y jnxOperatingMemory (MB total)
        m.insert("jnxOperatingBuffer".into(), "1.3.6.1.4.1.2636.3.1.13.1.11.9.1.0.0".into());
        m.insert("jnxOperatingMemory".into(), "1.3.6.1.4.1.2636.3.1.13.1.15.9.1.0.0".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
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
        // Temperatura del RE
        m.insert("jnxOperatingTemp".into(), "1.3.6.1.4.1.2636.3.1.13.1.7.9.1.0.0".into());
        // Alarmas
        m.insert("jnxYellowAlarmCount".into(), "1.3.6.1.4.1.2636.3.4.1.2.1.0".into());
        m.insert("jnxRedAlarmCount".into(),    "1.3.6.1.4.1.2636.3.4.1.1.1.0".into());
        // SRX Flow sessions
        m.insert("jnxJsSPUMonitoringSPUIndex".into(),   "1.3.6.1.4.1.2636.3.39.1.12.1.1.1.3".into());
        m.insert("jnxJsSPUMonitoringCurrentFlowSession".into(), "1.3.6.1.4.1.2636.3.39.1.12.1.1.1.6".into());
        m.insert("jnxJsSPUMonitoringMaxFlowSession".into(),     "1.3.6.1.4.1.2636.3.39.1.12.1.1.1.7".into());
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
        // SRX Flow Sessions (si aplica — solo en equipos SRX)
        // -----------------------------------------------------------------------
        let current_sessions = client.get("1.3.6.1.4.1.2636.3.39.1.12.1.1.1.6.0").await;
        let max_sessions     = client.get("1.3.6.1.4.1.2636.3.39.1.12.1.1.1.7.0").await;

        if let Some(current) = current_sessions.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("flow_sessions_current".into(), json!(current));
            let max = max_sessions.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0);
            data.insert("flow_sessions_max".into(), json!(max));
            if max > 0 {
                data.insert("flow_sessions_percent".into(),
                    json!(calculate_percentage(current as f64, max as f64)));
            }
        }

        // -----------------------------------------------------------------------
        // Componentes (FPCs/MICs/PICs)
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

        debug!("Juniper MX/SRX: temp={:?}°C, sessions={:?}/{:?}, components={}",
            data.get("temperature_re_c"),
            data.get("flow_sessions_current"),
            data.get("flow_sessions_max"),
            data.get("components").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        // Juniper MX series: 1.3.6.1.4.1.2636.1.1.1.2.29.* (MX240, etc.)
        // Juniper SRX series: 1.3.6.1.4.1.2636.1.1.1.2.36.* (SRX300, etc.)
        // Usamos el prefijo común de Juniper pero excluimos EX/QFX (ya cubiertos)
        if sys_oid.starts_with("1.3.6.1.4.1.2636") {
            // No matchear si ya es EX o QFX
            if sys_oid.starts_with("1.3.6.1.4.1.2636.1.51")
                || sys_oid.starts_with("1.3.6.1.4.1.2636.1.62")
            {
                return false;
            }
            return true;
        }
        false
    }
}
