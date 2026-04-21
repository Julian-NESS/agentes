// ==============================================================================
// NESS Relay v2.0.0 — Perfil MikroTik Firewall (RouterOS — modo firewall/gateway)
// Equivalente Python: profiles/vendors/mikrotik_fw.py
// ==============================================================================
//
// Diferencias con MikroTik router:
//   - device_type = "firewall"
//   - Memoria se extrae de hrStorageTable buscando "main memory"
//   - Agrega: Netwatch (monitoring de hosts), Queue Simple table
//   - Agrega: detección de interfaces WAN
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{bytes_to_gb, calculate_percentage};

const WAN_INTERFACE_PATTERNS: &[&str] = &[
    "ether1", "sfp1", "sfp-sfpplus1", "wan", "ether-wan",
    "pppoe", "pptp", "l2tp", "vlan999", "bridge-wan",
];

pub struct MikroTikFwProfile;

impl MikroTikFwProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for MikroTikFwProfile {
    fn vendor(&self) -> &str { "mikrotik_fw" }
    fn vendor_display_name(&self) -> &str { "MikroTik FirewallOS" }
    fn device_type(&self) -> &str { "firewall" }

    fn get_cpu_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("hrProcessorTable".into(), "1.3.6.1.2.1.25.3.3.1.2".into());
        m
    }

    fn get_memory_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Usar hrStorageTable (el de "main memory") ya que MIKROTIK-MIB puede no estar disponible
        m.insert("hrStorageDescr".into(),            "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(),  "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),             "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),             "1.3.6.1.2.1.25.2.3.1.6".into());
        // También intentar MIKROTIK-MIB como alternativa
        m.insert("mtxrHlTotalMemory".into(), "1.3.6.1.4.1.14988.1.1.1.17.0".into());
        m.insert("mtxrHlFreeMemory".into(),  "1.3.6.1.4.1.14988.1.1.1.18.0".into());
        m
    }

    fn get_disk_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("hrStorageTable".into(),             "1.3.6.1.2.1.25.2.3".into());
        m.insert("hrStorageDescr".into(),             "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(),   "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),              "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),              "1.3.6.1.2.1.25.2.3.1.6".into());
        m
    }

    fn get_vendor_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Paridad de OIDs con Python (migración completa)
        m.insert("py_migrated_oid_01".into(), "1.3.6.1.2.1.2.1.0".into());
        m.insert("py_migrated_oid_02".into(), "1.3.6.1.2.1.2.2.1.10".into());
        m.insert("py_migrated_oid_03".into(), "1.3.6.1.2.1.2.2.1.13".into());
        m.insert("py_migrated_oid_04".into(), "1.3.6.1.2.1.2.2.1.14".into());
        m.insert("py_migrated_oid_05".into(), "1.3.6.1.2.1.2.2.1.16".into());
        m.insert("py_migrated_oid_06".into(), "1.3.6.1.2.1.2.2.1.19".into());
        m.insert("py_migrated_oid_07".into(), "1.3.6.1.2.1.2.2.1.2".into());
        m.insert("py_migrated_oid_08".into(), "1.3.6.1.2.1.2.2.1.20".into());
        m.insert("py_migrated_oid_09".into(), "1.3.6.1.2.1.2.2.1.3".into());
        m.insert("py_migrated_oid_10".into(), "1.3.6.1.2.1.2.2.1.5".into());
        m.insert("py_migrated_oid_11".into(), "1.3.6.1.2.1.2.2.1.7".into());
        m.insert("py_migrated_oid_12".into(), "1.3.6.1.2.1.2.2.1.8".into());
        m.insert("py_migrated_oid_13".into(), "1.3.6.1.2.1.31.1.1.1.11".into());
        m.insert("py_migrated_oid_14".into(), "1.3.6.1.2.1.31.1.1.1.15".into());
        m.insert("py_migrated_oid_15".into(), "1.3.6.1.2.1.31.1.1.1.18".into());
        m.insert("py_migrated_oid_16".into(), "1.3.6.1.2.1.31.1.1.1.7".into());
        m.insert("py_migrated_oid_17".into(), "1.3.6.1.4.1.14988.1".into());
        m.insert("py_migrated_oid_18".into(), "1.3.6.1.4.1.14988.1.1.2.1".into());
        m.insert("py_migrated_oid_19".into(), "1.3.6.1.4.1.14988.1.1.2.1.1.10".into());
        m.insert("py_migrated_oid_20".into(), "1.3.6.1.4.1.14988.1.1.2.1.1.11".into());
        m.insert("py_migrated_oid_21".into(), "1.3.6.1.4.1.14988.1.1.2.1.1.12".into());
        m.insert("py_migrated_oid_22".into(), "1.3.6.1.4.1.14988.1.1.2.1.1.3".into());
        m.insert("py_migrated_oid_23".into(), "1.3.6.1.4.1.14988.1.1.2.1.1.4".into());
        m.insert("py_migrated_oid_24".into(), "1.3.6.1.4.1.14988.1.1.2.1.1.5".into());
        m.insert("py_migrated_oid_25".into(), "1.3.6.1.4.1.14988.1.1.2.1.1.7".into());
        m.insert("py_migrated_oid_26".into(), "1.3.6.1.4.1.14988.1.1.3.1.0".into());
        m.insert("py_migrated_oid_27".into(), "1.3.6.1.4.1.14988.1.1.3.10.0".into());
        m.insert("py_migrated_oid_28".into(), "1.3.6.1.4.1.14988.1.1.3.11.0".into());
        m.insert("py_migrated_oid_29".into(), "1.3.6.1.4.1.14988.1.1.3.12.0".into());
        m.insert("py_migrated_oid_30".into(), "1.3.6.1.4.1.14988.1.1.3.17.0".into());
        m.insert("py_migrated_oid_31".into(), "1.3.6.1.4.1.14988.1.1.3.18.0".into());
        m.insert("py_migrated_oid_32".into(), "1.3.6.1.4.1.14988.1.1.3.2.0".into());
        m.insert("py_migrated_oid_33".into(), "1.3.6.1.4.1.14988.1.1.3.8.0".into());
        m.insert("py_migrated_oid_34".into(), "1.3.6.1.4.1.14988.1.1.3.9.0".into());
        m.insert("py_migrated_oid_35".into(), "1.3.6.1.4.1.14988.1.1.4.3.0".into());
        m.insert("py_migrated_oid_36".into(), "1.3.6.1.4.1.14988.1.1.4.4.0".into());
        m.insert("py_migrated_oid_37".into(), "1.3.6.1.4.1.14988.1.1.4.7.0".into());
        m.insert("py_migrated_oid_38".into(), "1.3.6.1.4.1.14988.1.1.8".into());
        m.insert("py_migrated_oid_39".into(), "1.3.6.1.4.1.14988.1.1.8.1.1.2".into());
        m.insert("py_migrated_oid_40".into(), "1.3.6.1.4.1.14988.1.1.8.1.1.3".into());
        m.insert("py_migrated_oid_41".into(), "1.3.6.1.4.1.14988.1.1.8.1.1.4".into());
        m.insert("py_migrated_oid_42".into(), "1.3.6.1.4.1.14988.1.1.8.1.1.5".into());
        m.insert("py_migrated_oid_43".into(), "1.3.6.1.4.1.14988.1.1.8.1.1.6".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let mut cores = Vec::new();
        let mut total = 0u64;
        let mut count = 0u64;
        for (key, val) in raw {
            if let Some(usage) = val.as_i64() {
                cores.push(json!({ "core": key, "usage": usage }));
                total += usage as u64;
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
        // Preferir MIKROTIK-MIB si está disponible
        let mtxr_total = raw.get("mtxrHlTotalMemory").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let mtxr_free  = raw.get("mtxrHlFreeMemory").and_then(|v| v.as_i64()).unwrap_or(0) as f64;

        if mtxr_total > 0.0 {
            let used = (mtxr_total - mtxr_free).max(0.0);
            return json!({
                "total_gb": bytes_to_gb(mtxr_total),
                "used_gb":  bytes_to_gb(used),
                "free_gb":  bytes_to_gb(mtxr_free),
                "usage_percent": calculate_percentage(used, mtxr_total),
            });
        }

        // Fallback: buscar "main memory" en hrStorageTable — se aplica en post_process_performance
        json!({
            "total_gb": 0.0,
            "used_gb":  0.0,
            "free_gb":  0.0,
            "usage_percent": 0.0,
            "_needs_post_process": true,
        })
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        let mut disks = Vec::new();
        for (idx, entry) in raw {
            let descr = entry.get("hrStorageDescr")
                .map(|v| v.as_string())
                .unwrap_or_else(|| format!("storage-{}", idx));
            // Saltar "Memory" entries — son para la memoria RAM
            let descr_lower = descr.to_lowercase();
            if descr_lower.contains("memory") || descr_lower.contains("ram") {
                continue;
            }
            let units = entry.get("hrStorageAllocationUnits")
                .and_then(|v| v.as_i64()).unwrap_or(512) as f64;
            let size = entry.get("hrStorageSize")
                .and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used = entry.get("hrStorageUsed")
                .and_then(|v| v.as_i64()).unwrap_or(0) as f64;

            let total_bytes = size * units;
            let used_bytes  = used * units;
            let free_bytes  = (total_bytes - used_bytes).max(0.0);
            if total_bytes > 0.0 {
                disks.push(json!({
                    "mount": descr,
                    "total_gb": bytes_to_gb(total_bytes),
                    "used_gb":  bytes_to_gb(used_bytes),
                    "free_gb":  bytes_to_gb(free_bytes),
                    "usage_percent": calculate_percentage(used_bytes, total_bytes),
                }));
            }
        }
        json!(disks)
    }

    fn post_process_performance(&self, mut data: serde_json::Value) -> serde_json::Value {
        // Si memory._needs_post_process == true, intentar extraer de vendor_data
        let needs_post = data.get("memory")
            .and_then(|m| m.get("_needs_post_process"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if needs_post {
            // La memoria se buscará en el vendor_data.storage entries con "main memory"
            if let Some(storage) = data.get("vendor_data")
                .and_then(|vd| vd.get("storage_entries"))
                .and_then(|s| s.as_array())
            {
                for entry in storage {
                    let descr = entry.get("descr").and_then(|v| v.as_str()).unwrap_or("");
                    if descr.to_lowercase().contains("main") || descr.to_lowercase().contains("memory") {
                        let total_gb = entry.get("total_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let used_gb  = entry.get("used_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let usage    = entry.get("usage_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        if let Some(mem) = data.get_mut("memory") {
                            *mem = json!({
                                "total_gb": total_gb,
                                "used_gb":  used_gb,
                                "free_gb":  (total_gb - used_gb).max(0.0),
                                "usage_percent": usage,
                            });
                        }
                        break;
                    }
                }
            }
        }
        data
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Health (igual que MikroTik router)
        // -----------------------------------------------------------------------
        let temp = client.get("1.3.6.1.4.1.14988.1.1.1.3.0").await;
        let fw_ver = client.get("1.3.6.1.4.1.14988.1.1.7.7.0").await;
        let board  = client.get("1.3.6.1.4.1.14988.1.1.7.8.0").await;
        let serial = client.get("1.3.6.1.4.1.14988.1.1.7.3.0").await;

        if let Some(v) = temp.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("temperature_c".into(), json!(v as f64 / 10.0));
        }
        for (key, result) in [("firmware_version", &fw_ver), ("board_name", &board), ("serial_number", &serial)] {
            if let Some(v) = result.value.as_ref() {
                let s = v.as_string();
                if !s.is_empty() {
                    data.insert(key.into(), json!(s));
                }
            }
        }

        // -----------------------------------------------------------------------
        // Netwatch table (monitoreo de hosts)
        // -----------------------------------------------------------------------
        let (nw_names, _)    = client.bulk("1.3.6.1.4.1.14988.1.1.7.1.1.2", 30).await;
        let (nw_addrs, _)    = client.bulk("1.3.6.1.4.1.14988.1.1.7.1.1.3", 30).await;
        let (nw_status, _)   = client.bulk("1.3.6.1.4.1.14988.1.1.7.1.1.8", 30).await;

        if !nw_names.is_empty() {
            let addr_map: HashMap<String, String> = nw_addrs.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_string())))
                .collect();
            let status_map: HashMap<String, i64> = nw_status.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();

            let probes: Vec<serde_json::Value> = nw_names.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some(json!({
                        "name": v.as_string(),
                        "address": addr_map.get(&idx).cloned().unwrap_or_default(),
                        "status": if status_map.get(&idx).copied().unwrap_or(0) == 1 { "up" } else { "down" },
                    }))
                })
                .collect();
            data.insert("netwatch_probes".into(), json!(probes));
        }

        // -----------------------------------------------------------------------
        // Queue Simple Table (QoS)
        // -----------------------------------------------------------------------
        let (q_names, _)       = client.bulk("1.3.6.1.4.1.14988.1.1.2.1.1.2", 30).await;
        let (q_bytes_in, _)    = client.bulk("1.3.6.1.4.1.14988.1.1.2.1.1.8", 30).await;
        let (q_bytes_out, _)   = client.bulk("1.3.6.1.4.1.14988.1.1.2.1.1.9", 30).await;
        let (q_pkt_drop, _)    = client.bulk("1.3.6.1.4.1.14988.1.1.2.1.1.14", 30).await;

        if !q_names.is_empty() {
            let bi_map: HashMap<String, u64> = q_bytes_in.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
                .collect();
            let bo_map: HashMap<String, u64> = q_bytes_out.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
                .collect();
            let dr_map: HashMap<String, u64> = q_pkt_drop.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
                .collect();

            let queues: Vec<serde_json::Value> = q_names.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some(json!({
                        "name": v.as_string(),
                        "bytes_in": bi_map.get(&idx).copied().unwrap_or(0),
                        "bytes_out": bo_map.get(&idx).copied().unwrap_or(0),
                        "packets_dropped": dr_map.get(&idx).copied().unwrap_or(0),
                    }))
                })
                .collect();
            data.insert("queues".into(), json!(queues));
        }

        // -----------------------------------------------------------------------
        // Interfaces WAN
        // -----------------------------------------------------------------------
        let (if_names, _)  = client.bulk("1.3.6.1.2.1.31.1.1.1.1", 50).await;
        let (if_in, _)     = client.bulk("1.3.6.1.2.1.31.1.1.1.6", 50).await;
        let (if_out, _)    = client.bulk("1.3.6.1.2.1.31.1.1.1.10", 50).await;

        let in_map: HashMap<String, u64> = if_in.into_iter()
            .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
            .collect();
        let out_map: HashMap<String, u64> = if_out.into_iter()
            .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
            .collect();

        let wan_ifs: Vec<serde_json::Value> = if_names.into_iter()
            .filter_map(|(oid, v)| {
                let idx = oid.rsplit('.').next()?.to_string();
                let name = v.as_string();
                let name_lower = name.to_lowercase();
                let is_wan = WAN_INTERFACE_PATTERNS.iter().any(|p| name_lower.contains(p));
                if !is_wan { return None; }
                Some(json!({
                    "name": name,
                    "index": idx,
                    "in_bytes": in_map.get(&idx).copied().unwrap_or(0),
                    "out_bytes": out_map.get(&idx).copied().unwrap_or(0),
                }))
            })
            .collect();
        data.insert("wan_interfaces".into(), json!(wan_ifs));

        debug!("MikroTik FW: {} WAN, {} netwatch, {} queues",
            wan_ifs.len(),
            data.get("netwatch_probes").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            data.get("queues").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.14988")
    }
}
