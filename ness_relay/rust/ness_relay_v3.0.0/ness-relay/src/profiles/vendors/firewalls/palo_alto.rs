// ==============================================================================
// NESS Relay v2.0.3 — Perfil Palo Alto Networks (PAN-OS)
// ==============================================================================
//
// MIBs usados:
//   - PAN-COMMON-MIB (1.3.6.1.4.1.25461): sesiones, GlobalProtect, sistema
//   - HOST-RESOURCES-MIB: CPU, memoria, disco como fallback
//   - PAN-OS soporta hrProcessorLoad y hrStorageTable
//
// OIDs globales — compatibles con PA-220, PA-440, PA-820, PA-3200,
// PA-5200, PA-7000 series bajo PAN-OS 10.x / 11.x.
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{bytes_to_gb, calculate_percentage};

pub struct PaloAltoProfile;

impl PaloAltoProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for PaloAltoProfile {
    fn vendor(&self) -> &str { "palo_alto" }
    fn vendor_display_name(&self) -> &str { "Palo Alto Networks" }
    fn device_type(&self) -> &str { "firewall" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HOST-RESOURCES-MIB: PAN-OS expone hrProcessorLoad
        m.insert("hrProcessorLoad".into(), "1.3.6.1.2.1.25.3.3.1.2".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HOST-RESOURCES-MIB: hrStorageTable (PAN-OS lo soporta para RAM)
        m.insert("hrStorageDescr".into(),           "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(), "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),            "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),            "1.3.6.1.2.1.25.2.3.1.6".into());
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
        // PAN-COMMON-MIB: sesiones
        m.insert("panSessionActive".into(),      "1.3.6.1.4.1.25461.2.1.2.3.3.0".into());
        m.insert("panSessionMax".into(),          "1.3.6.1.4.1.25461.2.1.2.3.2.0".into());
        m.insert("panSessionUtilization".into(),  "1.3.6.1.4.1.25461.2.1.2.3.1.0".into());
        m.insert("panSessionThroughput".into(),   "1.3.6.1.4.1.25461.2.1.2.3.4.0".into());
        m.insert("panSessionConnectionsPerSecond".into(), "1.3.6.1.4.1.25461.2.1.2.3.5.0".into());
        // Sistema
        m.insert("panSysSwVersion".into(),  "1.3.6.1.4.1.25461.2.1.2.1.1.0".into());
        m.insert("panSysHwVersion".into(),  "1.3.6.1.4.1.25461.2.1.2.1.2.0".into());
        m.insert("panSysSerialNumber".into(), "1.3.6.1.4.1.25461.2.1.2.1.3.0".into());
        // GlobalProtect
        m.insert("panGPGWUtilizationActiveTunnels".into(), "1.3.6.1.4.1.25461.2.1.2.5.1.3.0".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        // hrProcessorLoad — tabla, calcular promedio
        let mut cores = Vec::new();
        let mut total = 0u64;
        let mut count = 0u64;
        for (k, v) in raw {
            if let Some(u) = v.as_i64() {
                cores.push(json!({ "core": k, "usage": u }));
                total += u as u64;
                count += 1;
            }
        }
        let avg = if count > 0 { total as f64 / count as f64 } else { 0.0 };
        json!({
            "cpu_usage_percent": (avg * 100.0).round() / 100.0,
            "cpu_cores": cores,
            "cpu_core_count": count,
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        // PAN-OS expone RAM a través de hrStorageTable
        // El colector de performance hará el bulk sobre hrStorageTable
        json!({
            "total_gb": 0.0, "used_gb": 0.0, "free_gb": 0.0, "usage_percent": 0.0
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
                    "total_gb": bytes_to_gb(total_b),
                    "used_gb":  bytes_to_gb(used_b),
                    "free_gb":  bytes_to_gb((total_b - used_b).max(0.0)),
                    "usage_percent": calculate_percentage(used_b, total_b),
                }));
            }
        }
        json!(disks)
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Sesiones
        // -----------------------------------------------------------------------
        let active   = client.get("1.3.6.1.4.1.25461.2.1.2.3.3.0").await;
        let max      = client.get("1.3.6.1.4.1.25461.2.1.2.3.2.0").await;
        let util     = client.get("1.3.6.1.4.1.25461.2.1.2.3.1.0").await;
        let throughput = client.get("1.3.6.1.4.1.25461.2.1.2.3.4.0").await;
        let cps      = client.get("1.3.6.1.4.1.25461.2.1.2.3.5.0").await;

        if let Some(v) = active.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("sessions_active".into(), json!(v));
        }
        if let Some(v) = max.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("sessions_max".into(), json!(v));
        }
        if let Some(v) = util.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("session_utilization_pct".into(), json!(v));
        }
        if let Some(v) = throughput.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("session_throughput_kbps".into(), json!(v));
        }
        if let Some(v) = cps.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("connections_per_second".into(), json!(v));
        }

        // -----------------------------------------------------------------------
        // Información del sistema (PAN-COMMON-MIB)
        // -----------------------------------------------------------------------
        let sw_ver  = client.get("1.3.6.1.4.1.25461.2.1.2.1.1.0").await;
        let hw_ver  = client.get("1.3.6.1.4.1.25461.2.1.2.1.2.0").await;
        let serial  = client.get("1.3.6.1.4.1.25461.2.1.2.1.3.0").await;

        for (key, res) in [("sw_version", &sw_ver), ("hw_version", &hw_ver), ("serial_number", &serial)] {
            if let Some(v) = res.value.as_ref() {
                let s = v.as_string();
                if !s.is_empty() {
                    data.insert(key.into(), json!(s));
                }
            }
        }

        // -----------------------------------------------------------------------
        // GlobalProtect tunnels
        // -----------------------------------------------------------------------
        let gp_tunnels = client.get("1.3.6.1.4.1.25461.2.1.2.5.1.3.0").await;
        if let Some(v) = gp_tunnels.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("globalprotect_active_tunnels".into(), json!(v));
        }

        debug!("Palo Alto: sessions={}/{}, GP tunnels={}, version={:?}",
            data.get("sessions_active").and_then(|v| v.as_i64()).unwrap_or(0),
            data.get("sessions_max").and_then(|v| v.as_i64()).unwrap_or(0),
            data.get("globalprotect_active_tunnels").and_then(|v| v.as_i64()).unwrap_or(0),
            data.get("sw_version"));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.25461")  // Palo Alto Networks enterprise OID
    }
}
