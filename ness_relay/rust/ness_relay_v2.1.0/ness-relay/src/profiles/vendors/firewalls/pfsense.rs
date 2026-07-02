// ==============================================================================
// NESS Relay v2.0.0 — Perfil pfSense
// Equivalente Python: profiles/vendors/pfsense.py
// ==============================================================================
//
// MIBs usados:
//   - UCD-SNMP-MIB: CPU, memoria, disco (formato Linux/UCD)
//   - IF-MIB: interfaces
//   - PF-MIB: estados de firewall, logs, bloqueos
// ==============================================================================

use async_trait::async_trait;
use std::cmp::Ordering;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{kb_to_gb, calculate_percentage};
use crate::utils::helpers::now_iso;

// Patrones de nombres de interfaces WAN en pfSense
const WAN_INTERFACE_PATTERNS: &[&str] = &[
    "wan", "opt1", "pppoe", "etb", "tigo", "claro",
    "une", "movistar", "igb0", "em0", "vtnet0",
    "ix0", "bxe0", "vmx0",
];

pub struct PfSenseProfile;

impl PfSenseProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for PfSenseProfile {
    fn vendor(&self) -> &str { "pfsense" }
    fn vendor_display_name(&self) -> &str { "pfSense (FreeBSD)" }
    fn device_type(&self) -> &str { "firewall" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // UCD-SNMP-MIB: indicadores de carga
        m.insert("ssUserProc".into(),   "1.3.6.1.4.1.2021.11.9.0".into());
        m.insert("ssSystemProc".into(), "1.3.6.1.4.1.2021.11.10.0".into());
        m.insert("ssIdleProc".into(),   "1.3.6.1.4.1.2021.11.11.0".into());
        m.insert("ssCpuRawInterrupt".into(), "1.3.6.1.4.1.2021.11.56.0".into());
        m.insert("hrProcessorLoad".into(), "1.3.6.1.2.1.25.3.3.1.2".into());
        m.insert("laLoad1".into(),      "1.3.6.1.4.1.2021.10.1.3.1".into());
        m.insert("laLoad5".into(),      "1.3.6.1.4.1.2021.10.1.3.2".into());
        m.insert("laLoad15".into(),     "1.3.6.1.4.1.2021.10.1.3.3".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // UCD-SNMP-MIB: memoria en KB
        m.insert("memTotalReal".into(),  "1.3.6.1.4.1.2021.4.5.0".into());
        m.insert("memAvailReal".into(),  "1.3.6.1.4.1.2021.4.6.0".into());
        m.insert("memTotalFree".into(),  "1.3.6.1.4.1.2021.4.11.0".into());
        m.insert("memBuffer".into(),     "1.3.6.1.4.1.2021.4.14.0".into());
        m.insert("memCached".into(),     "1.3.6.1.4.1.2021.4.15.0".into());
        m.insert("memTotalSwap".into(),  "1.3.6.1.4.1.2021.4.3.0".into());
        m.insert("memAvailSwap".into(),  "1.3.6.1.4.1.2021.4.4.0".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // UCD-SNMP-MIB dskTable
        m.insert("dskTable".into(),      "1.3.6.1.4.1.2021.9".into());
        m.insert("dskPath".into(),       "1.3.6.1.4.1.2021.9.1.2".into());
        m.insert("dskDevice".into(),     "1.3.6.1.4.1.2021.9.1.3".into());
        m.insert("dskTotal".into(),      "1.3.6.1.4.1.2021.9.1.6".into());  // KB
        m.insert("dskUsed".into(),       "1.3.6.1.4.1.2021.9.1.8".into());  // KB
        m.insert("dskAvail".into(),      "1.3.6.1.4.1.2021.9.1.7".into());  // KB
        m.insert("dskPercent".into(),    "1.3.6.1.4.1.2021.9.1.9".into());  // %
        m
    }

    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // PF-MIB: estados de firewall (pfSense specific)
        m.insert("pfStateCount".into(),    "1.3.6.1.4.1.12325.1.200.1.3.1.0".into());
        m.insert("pfStateLimit".into(),    "1.3.6.1.4.1.12325.1.200.1.3.2.0".into());
        m.insert("pfLogIfIn".into(),       "1.3.6.1.4.1.12325.1.200.1.8.4.0".into());
        m.insert("pfLogIfOut".into(),      "1.3.6.1.4.1.12325.1.200.1.8.5.0".into());
        m.insert("pfRuleEval".into(),      "1.3.6.1.4.1.12325.1.200.1.1.1.0".into());
        m.insert("pfIfRef".into(),         "1.3.6.1.4.1.12325.1.200.1.8.1.0".into());
        m.insert("pfIfIn4PassPkts".into(), "1.3.6.1.4.1.12325.1.200.1.8.3.1.0".into());
        m.insert("pfIfOut4PassPkts".into(),"1.3.6.1.4.1.12325.1.200.1.8.5.1.0".into());
        // OIDs adicionales de paridad Python
        m.insert("pfCounterMatch".into(),  "1.3.6.1.4.1.12325.1.200.1.2.1.0".into());
        m.insert("pfCounterBadOffset".into(), "1.3.6.1.4.1.12325.1.200.1.2.2.0".into());
        m.insert("pfStateTableInserts".into(), "1.3.6.1.4.1.12325.1.200.1.3.3.0".into());
        m.insert("pfStateTableRemovals".into(), "1.3.6.1.4.1.12325.1.200.1.3.4.0".into());
        m.insert("pfLogInterfaceBytesIn".into(), "1.3.6.1.4.1.12325.1.200.1.5.2.0".into());
        m.insert("pfLogInterfaceBytesOut".into(), "1.3.6.1.4.1.12325.1.200.1.5.3.0".into());
        m.insert("ifDescr".into(),         "1.3.6.1.2.1.2.2.1.2".into());
        m.insert("ifType".into(),          "1.3.6.1.2.1.2.2.1.3".into());
        m.insert("ifSpeed".into(),         "1.3.6.1.2.1.2.2.1.5".into());
        m.insert("ifAdminStatus".into(),   "1.3.6.1.2.1.2.2.1.7".into());
        m.insert("ifOperStatus".into(),    "1.3.6.1.2.1.2.2.1.8".into());
        m.insert("ifInOctets".into(),      "1.3.6.1.2.1.2.2.1.10".into());
        m.insert("ifInUcastPkts".into(),   "1.3.6.1.2.1.2.2.1.11".into());
        m.insert("ifInDiscards".into(),    "1.3.6.1.2.1.2.2.1.13".into());
        m.insert("ifInErrors".into(),      "1.3.6.1.2.1.2.2.1.14".into());
        m.insert("ifOutOctets".into(),     "1.3.6.1.2.1.2.2.1.16".into());
        m.insert("ifOutUcastPkts".into(),  "1.3.6.1.2.1.2.2.1.17".into());
        m.insert("ifOutDiscards".into(),   "1.3.6.1.2.1.2.2.1.19".into());
        m.insert("ifOutErrors".into(),     "1.3.6.1.2.1.2.2.1.20".into());
        m.insert("ifHCInUcastPkts".into(), "1.3.6.1.2.1.31.1.1.1.7".into());
        m.insert("ifHCOutUcastPkts".into(), "1.3.6.1.2.1.31.1.1.1.11".into());
        m.insert("ifAlias".into(),         "1.3.6.1.2.1.31.1.1.1.18".into());
        m.insert("netSnmpSysObjectPrefix".into(), "1.3.6.1.4.1.8072.3.2".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let user = raw.get("ssUserProc").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let system = raw.get("ssSystemProc").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let idle = raw.get("ssIdleProc").and_then(|v| v.as_i64()).unwrap_or(100) as f64;

        // Alinear comportamiento con Python:
        // - Camino principal: cpu_usage = 100 - idle
        // - Fallback #1: user + system
        // - Fallback #2: laLoad1 normalizado
        let cpu_usage = if raw.get("ssIdleProc").is_some() {
            (100.0 - idle).clamp(0.0, 100.0)
        } else if user > 0.0 || system > 0.0 {
            (user + system).min(100.0)
        } else {
            let la = raw.get("laLoad1")
                .map(|v| v.as_string())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            (la * 10.0).min(100.0)
        };

        json!({
            "cpu_usage_percent": (cpu_usage * 100.0).round() / 100.0,
            "cpu_user_percent": user,
            "cpu_system_percent": system,
            "cpu_idle_percent": idle,
            "load_avg_1": raw.get("laLoad1").map(|v| v.as_string()).unwrap_or_default(),
            "load_avg_5": raw.get("laLoad5").map(|v| v.as_string()).unwrap_or_default(),
            "load_avg_15": raw.get("laLoad15").map(|v| v.as_string()).unwrap_or_default(),
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let total_kb = raw.get("memTotalReal").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let avail_kb = raw.get("memAvailReal").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let total_free_kb = raw.get("memTotalFree").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let cached_kb = raw.get("memCached").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let buffer_kb = raw.get("memBuffer").and_then(|v| v.as_i64()).unwrap_or(0) as f64;

        // Paridad con Python: preferir memAvailReal; si falta, usar free+cached+buffer.
        let free_kb = if avail_kb > 0.0 {
            avail_kb
        } else {
            total_free_kb + cached_kb + buffer_kb
        };
        let used_kb  = (total_kb - free_kb).max(0.0);

        let swap_total_kb = raw.get("memTotalSwap").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let swap_free_kb  = raw.get("memAvailSwap").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let swap_used_kb  = (swap_total_kb - swap_free_kb).max(0.0);

        json!({
            "total_gb": kb_to_gb(total_kb),
            "used_gb":  kb_to_gb(used_kb),
            "free_gb":  kb_to_gb(free_kb),
            "cached_gb": kb_to_gb(cached_kb),
            "buffer_gb": kb_to_gb(buffer_kb),
            "usage_percent": calculate_percentage(used_kb, total_kb),
            "swap_total_gb": kb_to_gb(swap_total_kb),
            "swap_used_gb":  kb_to_gb(swap_used_kb),
            "swap_usage_percent": calculate_percentage(swap_used_kb, swap_total_kb),
        })
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        let mut disks = Vec::new();
        let mut ordered: Vec<(&String, &HashMap<String, SnmpValue>)> = raw.iter().collect();
        ordered.sort_by(|(idx_a, _), (idx_b, _)| {
            let a = idx_a.parse::<u64>().ok();
            let b = idx_b.parse::<u64>().ok();
            match (a, b) {
                (Some(na), Some(nb)) => na.cmp(&nb),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => idx_a.cmp(idx_b),
            }
        });

        for (idx, disk_data) in ordered {
            let path = disk_data.get("dskPath")
                .map(|v| v.as_string())
                .unwrap_or_else(|| format!("/disk{}", idx));
            let total_kb = disk_data.get("dskTotal").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used_kb  = disk_data.get("dskUsed").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let pct      = disk_data.get("dskPercent").and_then(|v| v.as_i64()).unwrap_or(0) as f64;

            // Paridad con Python: free = total - used (no depender de dskAvail).
            let total_gb = kb_to_gb(total_kb);
            let used_gb = kb_to_gb(used_kb);
            let free_gb = (total_gb - used_gb).max(0.0);

            disks.push(json!({
                "index": idx,
                "mount": path,
                "total_gb": total_gb,
                "used_gb":  used_gb,
                "free_gb":  free_gb,
                "usage_percent": pct,
            }));
        }
        json!(disks)
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // Estado de firewall (PF-MIB)
        let firewall_state_count = client.get("1.3.6.1.4.1.12325.1.200.1.3.1.0").await;
        let firewall_state_limit = client.get("1.3.6.1.4.1.12325.1.200.1.3.2.0").await;

        let state_count = firewall_state_count.value.as_ref()
            .and_then(|v| v.as_i64()).unwrap_or(0);
        let state_limit = firewall_state_limit.value.as_ref()
            .and_then(|v| v.as_i64()).unwrap_or(0);

        data.insert("pf_state_count".into(), json!(state_count));
        data.insert("pf_state_limit".into(), json!(state_limit));
        if state_limit > 0 {
            let usage = (state_count as f64 / state_limit as f64) * 100.0;
            data.insert("pf_state_usage_percent".into(), json!((usage * 100.0).round() / 100.0));
        }

        // Interfaces WAN (detección por nombre)
        let (if_names, _) = client.bulk("1.3.6.1.2.1.31.1.1.1.1", 50).await;
        let (if_hc_in, _)  = client.bulk("1.3.6.1.2.1.31.1.1.1.6", 50).await;
        let (if_hc_out, _) = client.bulk("1.3.6.1.2.1.31.1.1.1.10", 50).await;
        let (if_speed, _)  = client.bulk("1.3.6.1.2.1.31.1.1.1.15", 50).await;

        // Indexar por SNMP index
        let names_map: HashMap<String, String> = if_names.into_iter()
            .filter_map(|(oid, v)| {
                let idx = oid.rsplit('.').next()?.to_string();
                Some((idx, v.as_string()))
            })
            .collect();

        let in_map: HashMap<String, u64> = if_hc_in.into_iter()
            .filter_map(|(oid, v)| {
                let idx = oid.rsplit('.').next()?.to_string();
                Some((idx, v.as_u64().unwrap_or(0)))
            })
            .collect();

        let out_map: HashMap<String, u64> = if_hc_out.into_iter()
            .filter_map(|(oid, v)| {
                let idx = oid.rsplit('.').next()?.to_string();
                Some((idx, v.as_u64().unwrap_or(0)))
            })
            .collect();

        let speed_map: HashMap<String, u64> = if_speed.into_iter()
            .filter_map(|(oid, v)| {
                let idx = oid.rsplit('.').next()?.to_string();
                Some((idx, v.as_u64().unwrap_or(0)))
            })
            .collect();

        let mut wan_interfaces = Vec::new();
        for (idx, name) in &names_map {
            let name_lower = name.to_lowercase();
            let is_wan = WAN_INTERFACE_PATTERNS.iter()
                .any(|pat| name_lower.contains(pat));
            if is_wan {
                let in_bytes = in_map.get(idx).copied().unwrap_or(0);
                let out_bytes = out_map.get(idx).copied().unwrap_or(0);
                let speed_mbps = speed_map.get(idx).copied().unwrap_or(0);
                wan_interfaces.push(json!({
                    "name": name,
                    "index": idx,
                    "in_bytes": in_bytes,
                    "out_bytes": out_bytes,
                    "speed_mbps": speed_mbps,
                }));
            }
        }
        data.insert("wan_interfaces".into(), json!(wan_interfaces));
        data.insert("total_interfaces".into(), json!(names_map.len()));
        data.insert("collection_timestamp".into(), json!(now_iso()));

        debug!("pfSense: {} interfaces, {} WAN detectadas, {} estados firewall",
            names_map.len(), wan_interfaces.len(), state_count);

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.contains("1.3.6.1.4.1.12325")  // pfSense enterprise OID
    }
}
