// ==============================================================================
// NESS Relay v2.0.3 — Perfil Aruba (HPE) Switch
// ==============================================================================
//
// MIBs usados:
//   - HP-SWITCH-MIB (1.3.6.1.4.1.11.2.14.11.5): CPU, memoria, temperatura
//   - ARUBA-MIB (1.3.6.1.4.1.14823): para ArubaOS-CX si se detecta
//   - POWER-ETHERNET-MIB: PoE
//
// OIDs globales — compatibles con ArubaOS-Switch (ProVision) y AOS-S.
// Aplica a: Aruba 2530, 2540, 2930F, 3810M, 6300, 6400, CX series.
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::calculate_percentage;

pub struct ArubaProfile;

impl ArubaProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for ArubaProfile {
    fn vendor(&self) -> &str { "aruba" }
    fn vendor_display_name(&self) -> &str { "Aruba (HPE)" }
    fn device_type(&self) -> &str { "switch" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HP-SWITCH-MIB: hpSwitchCpuStat (porcentaje global)
        m.insert("hpSwitchCpuStat".into(), "1.3.6.1.4.1.11.2.14.11.5.1.9.6.1.0".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HP-SWITCH-MIB: memoria total y libre (bytes)
        m.insert("hpLocalMemTotalBytes".into(), "1.3.6.1.4.1.11.2.14.11.5.1.1.2.1.1.1.5.1".into());
        m.insert("hpLocalMemFreeBytes".into(),  "1.3.6.1.4.1.11.2.14.11.5.1.1.2.1.1.1.6.1".into());
        m.insert("hpLocalMemAllocBytes".into(), "1.3.6.1.4.1.11.2.14.11.5.1.1.2.1.1.1.7.1".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        // Switches Aruba no tienen disco — datos efímeros en flash
        HashMap::new()
    }

    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Temperatura del sistema
        m.insert("hpSwitchTempStatus".into(), "1.3.6.1.4.1.11.2.14.11.5.1.9.7.1.0".into());
        // Fan status
        m.insert("hpicfSensorStatus".into(), "1.3.6.1.4.1.11.2.14.11.1.2.6.1.4".into());
        // PoE (POWER-ETHERNET-MIB)
        m.insert("pethPsePortDetectionStatus".into(), "1.3.6.1.2.1.105.1.1.1.6".into());
        m.insert("pethPsePortPowerPairs".into(),      "1.3.6.1.2.1.105.1.1.1.10".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let cpu_pct = raw.get("hpSwitchCpuStat")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        json!({
            "cpu_usage_percent": cpu_pct,
            "cpu_cores": [{ "core": 0, "usage": cpu_pct }],
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let total = raw.get("hpLocalMemTotalBytes").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let free  = raw.get("hpLocalMemFreeBytes").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let used  = (total - free).max(0.0);
        json!({
            "total_gb": (total / 1_073_741_824.0 * 100.0).round() / 100.0,
            "used_gb":  (used  / 1_073_741_824.0 * 100.0).round() / 100.0,
            "free_gb":  (free  / 1_073_741_824.0 * 100.0).round() / 100.0,
            "usage_percent": calculate_percentage(used, total),
        })
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
        // Temperatura
        // -----------------------------------------------------------------------
        let temp = client.get("1.3.6.1.4.1.11.2.14.11.5.1.9.7.1.0").await;
        if let Some(v) = temp.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("temperature_c".into(), json!(v));
        }

        // -----------------------------------------------------------------------
        // Fan status (hpicfSensorTable)
        // -----------------------------------------------------------------------
        let (fan_status, _) = client.bulk("1.3.6.1.4.1.11.2.14.11.1.2.6.1.4", 20).await;
        if !fan_status.is_empty() {
            let fans: Vec<serde_json::Value> = fan_status.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    let status = v.as_i64().unwrap_or(0);
                    // hpicfSensorStatus: 1=unknown, 2=bad, 3=warning, 4=good
                    let status_text = match status {
                        2 => "bad",
                        3 => "warning",
                        4 => "good",
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
        // PoE (POWER-ETHERNET-MIB)
        // -----------------------------------------------------------------------
        let (poe_status, _) = client.bulk("1.3.6.1.2.1.105.1.1.1.6", 50).await;
        let (poe_power, _)  = client.bulk("1.3.6.1.2.1.105.1.1.1.10", 50).await;

        if !poe_status.is_empty() {
            let pwr_map: HashMap<String, i64> = poe_power.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();

            let poe_ports: Vec<serde_json::Value> = poe_status.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some(json!({
                        "port": idx,
                        "detection_status": v.as_i64().unwrap_or(0),
                        "power_mw": pwr_map.get(&idx).copied().unwrap_or(0),
                    }))
                })
                .collect();
            data.insert("poe_ports".into(), json!(poe_ports));

            let total_poe: i64 = pwr_map.values().sum();
            data.insert("poe_total_power_mw".into(), json!(total_poe));
        }

        debug!("Aruba: temp={:?}°C, fans={}, poe_ports={}",
            data.get("temperature_c"),
            data.get("fans").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            data.get("poe_ports").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.11.2.3.7") ||  // HP ProCurve / ProVision
        sys_oid.starts_with("1.3.6.1.4.1.14823")        // Aruba Networks
    }
}
