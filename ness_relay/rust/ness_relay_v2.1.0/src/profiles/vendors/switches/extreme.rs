// ==============================================================================
// NESS Relay v2.0.3 — Perfil Extreme Networks (EXOS / VOSS)
// ==============================================================================
//
// MIBs usados:
//   - EXTREME-SYSTEM-MIB (1.3.6.1.4.1.1916): CPU, memoria, temperatura, fans
//   - HOST-RESOURCES-MIB: como fallback
//
// OIDs globales — compatibles con EXOS (X440, X460, X670, X870)
// y VOSS (VSP series) bajo Extreme OS.
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::calculate_percentage;

pub struct ExtremeProfile;

impl ExtremeProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for ExtremeProfile {
    fn vendor(&self) -> &str { "extreme" }
    fn vendor_display_name(&self) -> &str { "Extreme Networks" }
    fn device_type(&self) -> &str { "switch" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // EXTREME-SYSTEM-MIB: extremeCpuMonitorTotalUtilization (5 sec avg)
        m.insert("extremeCpuTotal".into(), "1.3.6.1.4.1.1916.1.32.1.4.1.5.1".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // EXTREME-SYSTEM-MIB: memoria del sistema
        m.insert("extremeMemTotal".into(), "1.3.6.1.4.1.1916.1.32.2.2.1.2.1".into());
        m.insert("extremeMemFree".into(),  "1.3.6.1.4.1.1916.1.32.2.2.1.3.1".into());
        // Alternativo: extremeMemoryMonitorSystemUsage (porcentaje)
        m.insert("extremeMemUsagePct".into(), "1.3.6.1.4.1.1916.1.32.2.2.1.4.1".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        // Switches Extreme — flash interna, no se expone normalmente como storage
        HashMap::new()
    }

    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Temperatura
        m.insert("extremeCurrentTemp".into(), "1.3.6.1.4.1.1916.1.1.1.8.0".into());
        // Fan table
        m.insert("extremeFanStatusTable".into(), "1.3.6.1.4.1.1916.1.1.1.9.1.2".into());
        // PSU table
        m.insert("extremePSUStatusTable".into(), "1.3.6.1.4.1.1916.1.1.1.10.1.2".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let cpu_pct = raw.get("extremeCpuTotal")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        json!({
            "cpu_usage_percent": cpu_pct,
            "cpu_cores": [{ "core": 0, "usage": cpu_pct }],
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        // Intentar primero total/free en KB
        let total_kb = raw.get("extremeMemTotal").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let free_kb  = raw.get("extremeMemFree").and_then(|v| v.as_i64()).unwrap_or(0) as f64;

        if total_kb > 0.0 {
            let used_kb = (total_kb - free_kb).max(0.0);
            return json!({
                "total_gb": (total_kb / 1_048_576.0 * 100.0).round() / 100.0,
                "used_gb":  (used_kb  / 1_048_576.0 * 100.0).round() / 100.0,
                "free_gb":  (free_kb  / 1_048_576.0 * 100.0).round() / 100.0,
                "usage_percent": calculate_percentage(used_kb, total_kb),
            });
        }

        // Fallback: solo porcentaje
        let mem_pct = raw.get("extremeMemUsagePct")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        json!({ "usage_percent": mem_pct })
    }

    fn normalize_disk_data(
        &self,
        _raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        json!([])
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Temperatura del sistema
        // -----------------------------------------------------------------------
        let temp = client.get("1.3.6.1.4.1.1916.1.1.1.8.0").await;
        if let Some(v) = temp.value.as_ref().and_then(|v| v.as_i64()) {
            // EXOS reporta en 0.5°C increments — dividir entre 2
            data.insert("temperature_c".into(), json!(v as f64 / 2.0));
        }

        // -----------------------------------------------------------------------
        // Fan status (extremeFanStatusTable)
        // -----------------------------------------------------------------------
        let (fan_status, _) = client.bulk("1.3.6.1.4.1.1916.1.1.1.9.1.2", 20).await;
        if !fan_status.is_empty() {
            let fans: Vec<serde_json::Value> = fan_status.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    let status = v.as_i64().unwrap_or(0);
                    // 1=operational, 2=not operational
                    let status_text = match status {
                        1 => "operational",
                        2 => "not operational",
                        _ => "unknown",
                    };
                    Some(json!({
                        "index": idx,
                        "status": status,
                        "status_text": status_text,
                    }))
                })
                .collect();
            data.insert("fans".into(), json!(fans));
        }

        // -----------------------------------------------------------------------
        // PSU status (extremePowerSupplyStatusTable)
        // -----------------------------------------------------------------------
        let (psu_status, _) = client.bulk("1.3.6.1.4.1.1916.1.1.1.10.1.2", 10).await;
        if !psu_status.is_empty() {
            let psus: Vec<serde_json::Value> = psu_status.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    let status = v.as_i64().unwrap_or(0);
                    // 1=present OK, 2=absent
                    let status_text = match status {
                        1 => "present_ok",
                        2 => "absent",
                        _ => "unknown",
                    };
                    Some(json!({
                        "index": idx,
                        "status": status,
                        "status_text": status_text,
                    }))
                })
                .collect();
            data.insert("power_supplies".into(), json!(psus));
        }

        debug!("Extreme: temp={:?}°C, fans={}, psus={}",
            data.get("temperature_c"),
            data.get("fans").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            data.get("power_supplies").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.1916")  // Extreme Networks enterprise OID
    }
}
