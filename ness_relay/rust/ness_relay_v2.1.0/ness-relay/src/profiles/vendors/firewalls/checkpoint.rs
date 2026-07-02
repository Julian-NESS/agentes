// ==============================================================================
// NESS Relay v2.0.3 — Perfil Check Point (Gaia OS)
// ==============================================================================
//
// MIBs usados:
//   - CHECKPOINT-MIB (1.3.6.1.4.1.2620): CPU, memoria, sesiones, HA, políticas
//   - Aplica a: todos los appliances Check Point bajo Gaia OS (R80+, R81+)
//     incluyendo 3200, 5200, 6700, 15000 series
//
// OIDs globales — compatibles con toda la familia Gaia OS.
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{kb_to_gb, calculate_percentage};

pub struct CheckPointProfile;

impl CheckPointProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for CheckPointProfile {
    fn vendor(&self) -> &str { "checkpoint" }
    fn vendor_display_name(&self) -> &str { "Check Point" }
    fn device_type(&self) -> &str { "firewall" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // CHECKPOINT-MIB: procUsage — porcentaje CPU global
        m.insert("procUsage".into(), "1.3.6.1.4.1.2620.1.6.7.2.7.0".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // CHECKPOINT-MIB: memoria total y usada (KB en 64-bit)
        m.insert("memTotalReal64".into(),  "1.3.6.1.4.1.2620.1.6.7.4.3.0".into());
        m.insert("memActiveReal64".into(), "1.3.6.1.4.1.2620.1.6.7.4.4.0".into());
        m.insert("memFreeReal64".into(),   "1.3.6.1.4.1.2620.1.6.7.4.5.0".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // CHECKPOINT-MIB: multiDiskTable
        m.insert("multiDiskName".into(),       "1.3.6.1.4.1.2620.1.6.7.3.6.1.2".into());
        m.insert("multiDiskSize".into(),       "1.3.6.1.4.1.2620.1.6.7.3.6.1.3".into());
        m.insert("multiDiskUsed".into(),       "1.3.6.1.4.1.2620.1.6.7.3.6.1.4".into());
        m.insert("multiDiskFreeTotalBytes".into(), "1.3.6.1.4.1.2620.1.6.7.3.6.1.5".into());
        m.insert("multiDiskFreeAvailableBytes".into(), "1.3.6.1.4.1.2620.1.6.7.3.6.1.6".into());
        m
    }

    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Firewall — conexiones y política
        m.insert("fwNumConn".into(),     "1.3.6.1.4.1.2620.1.1.25.3.0".into());
        m.insert("fwPeakNumConn".into(), "1.3.6.1.4.1.2620.1.1.25.4.0".into());
        m.insert("fwPolicyName".into(),  "1.3.6.1.4.1.2620.1.1.2.0".into());
        m.insert("fwInstallTime".into(), "1.3.6.1.4.1.2620.1.1.4.0".into());
        // HA
        m.insert("haState".into(),        "1.3.6.1.4.1.2620.1.5.6.0".into());
        m.insert("haStatShort".into(),    "1.3.6.1.4.1.2620.1.5.7.0".into());
        // SVN / versión
        m.insert("svnVersion".into(),     "1.3.6.1.4.1.2620.1.6.4.1.0".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let cpu_pct = raw.get("procUsage")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        json!({
            "cpu_usage_percent": cpu_pct,
            "cpu_cores": [{ "core": 0, "usage": cpu_pct }],
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let total_kb = raw.get("memTotalReal64")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        let active_kb = raw.get("memActiveReal64")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        let free_kb = raw.get("memFreeReal64")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;

        let used_kb = if active_kb > 0.0 { active_kb } else { (total_kb - free_kb).max(0.0) };

        json!({
            "total_gb": kb_to_gb(total_kb),
            "used_gb":  kb_to_gb(used_kb),
            "free_gb":  kb_to_gb(free_kb),
            "usage_percent": calculate_percentage(used_kb, total_kb),
        })
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        let mut disks = Vec::new();
        for (_idx, entry) in raw {
            let name = entry.get("multiDiskName").map(|v| v.as_string())
                .unwrap_or_else(|| "/".to_string());
            let total = entry.get("multiDiskSize")
                .and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used = entry.get("multiDiskUsed")
                .and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let free = (total - used).max(0.0);
            if total > 0.0 {
                disks.push(json!({
                    "mount": name,
                    "total_gb": (total / 1_073_741_824.0 * 100.0).round() / 100.0,
                    "used_gb":  (used  / 1_073_741_824.0 * 100.0).round() / 100.0,
                    "free_gb":  (free  / 1_073_741_824.0 * 100.0).round() / 100.0,
                    "usage_percent": calculate_percentage(used, total),
                }));
            }
        }
        json!(disks)
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Firewall: conexiones y política
        // -----------------------------------------------------------------------
        let num_conn      = client.get("1.3.6.1.4.1.2620.1.1.25.3.0").await;
        let peak_conn     = client.get("1.3.6.1.4.1.2620.1.1.25.4.0").await;
        let policy_name   = client.get("1.3.6.1.4.1.2620.1.1.2.0").await;
        let install_time  = client.get("1.3.6.1.4.1.2620.1.1.4.0").await;

        if let Some(v) = num_conn.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("fw_connections".into(), json!(v));
        }
        if let Some(v) = peak_conn.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("fw_peak_connections".into(), json!(v));
        }
        if let Some(v) = policy_name.value.as_ref() {
            let s = v.as_string();
            if !s.is_empty() {
                data.insert("fw_policy_name".into(), json!(s));
            }
        }
        if let Some(v) = install_time.value.as_ref() {
            data.insert("fw_policy_install_time".into(), json!(v.as_string()));
        }

        // -----------------------------------------------------------------------
        // HA (High Availability)
        // -----------------------------------------------------------------------
        let ha_state      = client.get("1.3.6.1.4.1.2620.1.5.6.0").await;
        let ha_stat_short = client.get("1.3.6.1.4.1.2620.1.5.7.0").await;

        if let Some(v) = ha_state.value.as_ref() {
            let state = v.as_string();
            if !state.is_empty() {
                data.insert("ha_state".into(), json!(state));
                if let Some(short) = ha_stat_short.value.as_ref() {
                    data.insert("ha_state_short".into(), json!(short.as_string()));
                }
            }
        }

        // -----------------------------------------------------------------------
        // Versión SVN
        // -----------------------------------------------------------------------
        let svn_ver = client.get("1.3.6.1.4.1.2620.1.6.4.1.0").await;
        if let Some(v) = svn_ver.value.as_ref() {
            data.insert("version".into(), json!(v.as_string()));
        }

        debug!("Check Point: connections={}, policy={:?}, ha={:?}",
            data.get("fw_connections").and_then(|v| v.as_i64()).unwrap_or(0),
            data.get("fw_policy_name"),
            data.get("ha_state"));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.2620")  // Check Point enterprise OID
    }
}
