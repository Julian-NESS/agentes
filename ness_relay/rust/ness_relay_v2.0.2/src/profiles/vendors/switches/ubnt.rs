// ==============================================================================
// NESS Relay v2.0.0 — Perfil Ubiquiti (UniFi / EdgeSwitch / EdgeRouter)
// Equivalente Python: profiles/vendors/ubnt.py
// ==============================================================================
//
// MIBs usados:
//   - HOST-RESOURCES-MIB: CPU, disco
//   - UBNT-MIB / UBNT-UniFi-MIB: modelo, temperatura
//   - EdgeSwitch MIB: CPU/memoria propios
//   - POWER-ETHERNET-MIB: PoE
//   - Q-BRIDGE-MIB: VLANs
//   - BRIDGE-MIB: tabla MAC
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{bytes_to_gb, calculate_percentage};

pub struct UbntProfile;

impl UbntProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for UbntProfile {
    fn vendor(&self) -> &str { "ubnt" }
    fn vendor_display_name(&self) -> &str { "Ubiquiti" }
    fn device_type(&self) -> &str { "switch" }

    fn get_cpu_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HOST-RESOURCES-MIB (genérico)
        m.insert("hrProcessorLoad".into(), "1.3.6.1.2.1.25.3.3.1.2".into()); // tabla
        // EdgeSwitch específico
        m.insert("edgeCpuUtil".into(),     "1.3.6.1.4.1.4413.1.1.1.1.4.1.0".into());
        m
    }

    fn get_memory_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // EdgeSwitch específico
        m.insert("edgeMemTotal".into(), "1.3.6.1.4.1.4413.1.1.1.1.4.2.0".into());
        m.insert("edgeMemFree".into(),  "1.3.6.1.4.1.4413.1.1.1.1.4.3.0".into());
        m
    }

    fn get_disk_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("hrStorageTable".into(),            "1.3.6.1.2.1.25.2.3".into());
        m.insert("hrStorageDescr".into(),            "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(),  "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),             "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),             "1.3.6.1.2.1.25.2.3.1.6".into());
        m
    }

    fn get_vendor_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Paridad de OIDs con Python (migración completa)
        m.insert("py_migrated_oid_01".into(), "1.3.6.1.2.1.105.1.1.1.10".into());
        m.insert("py_migrated_oid_02".into(), "1.3.6.1.2.1.105.1.1.1.4".into());
        m.insert("py_migrated_oid_03".into(), "1.3.6.1.2.1.105.1.1.1.7".into());
        m.insert("py_migrated_oid_04".into(), "1.3.6.1.2.1.105.1.3.1.1.2".into());
        m.insert("py_migrated_oid_05".into(), "1.3.6.1.2.1.105.1.3.1.1.3".into());
        m.insert("py_migrated_oid_06".into(), "1.3.6.1.2.1.105.1.3.1.1.4".into());
        m.insert("py_migrated_oid_07".into(), "1.3.6.1.2.1.17.4.3.1.1".into());
        m.insert("py_migrated_oid_08".into(), "1.3.6.1.2.1.17.4.3.1.2".into());
        m.insert("py_migrated_oid_09".into(), "1.3.6.1.2.1.17.4.3.1.3".into());
        m.insert("py_migrated_oid_10".into(), "1.3.6.1.2.1.17.7.1.4.2.1.3".into());
        m.insert("py_migrated_oid_11".into(), "1.3.6.1.2.1.17.7.1.4.3.1.2".into());
        m.insert("py_migrated_oid_12".into(), "1.3.6.1.2.1.17.7.1.4.5.1.1".into());
        m.insert("py_migrated_oid_13".into(), "1.3.6.1.2.1.2.2.1.10".into());
        m.insert("py_migrated_oid_14".into(), "1.3.6.1.2.1.2.2.1.14".into());
        m.insert("py_migrated_oid_15".into(), "1.3.6.1.2.1.2.2.1.16".into());
        m.insert("py_migrated_oid_16".into(), "1.3.6.1.2.1.2.2.1.2".into());
        m.insert("py_migrated_oid_17".into(), "1.3.6.1.2.1.2.2.1.20".into());
        m.insert("py_migrated_oid_18".into(), "1.3.6.1.2.1.2.2.1.3".into());
        m.insert("py_migrated_oid_19".into(), "1.3.6.1.2.1.2.2.1.5".into());
        m.insert("py_migrated_oid_20".into(), "1.3.6.1.2.1.2.2.1.7".into());
        m.insert("py_migrated_oid_21".into(), "1.3.6.1.2.1.2.2.1.8".into());
        m.insert("py_migrated_oid_22".into(), "1.3.6.1.2.1.31.1.1.1.1".into());
        m.insert("py_migrated_oid_23".into(), "1.3.6.1.2.1.31.1.1.1.10".into());
        m.insert("py_migrated_oid_24".into(), "1.3.6.1.2.1.31.1.1.1.15".into());
        m.insert("py_migrated_oid_25".into(), "1.3.6.1.2.1.31.1.1.1.18".into());
        m.insert("py_migrated_oid_26".into(), "1.3.6.1.2.1.31.1.1.1.6".into());
        m.insert("py_migrated_oid_27".into(), "1.3.6.1.4.1.41112.1.6.2.1.1".into());
        m.insert("py_migrated_oid_28".into(), "1.3.6.1.4.1.41112.1.6.3.2.0".into());
        m.insert("py_migrated_oid_29".into(), "1.3.6.1.4.1.41112.1.6.3.4.0".into());
        m.insert("py_migrated_oid_30".into(), "1.3.6.1.4.1.41112.1.6.3.5.0".into());
        m.insert("py_migrated_oid_31".into(), "1.3.6.1.4.1.41112.1.6.3.6.0".into());
        m.insert("py_migrated_oid_32".into(), "1.3.6.1.4.1.41112.1.6.3.7.0".into());
        m.insert("py_migrated_oid_33".into(), "1.3.6.1.4.1.41112.1.6.3.8.0".into());
        m.insert("py_migrated_oid_34".into(), "1.3.6.1.4.1.4413.1.1.1.1.4.9.0".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        // EdgeSwitch específico tiene un solo valor
        if let Some(cpu) = raw.get("edgeCpuUtil").and_then(|v| v.as_i64()) {
            return json!({
                "cpu_usage_percent": cpu as f64,
                "cpu_cores": [{ "core": 0, "usage": cpu }],
            });
        }
        // Fallback: hrProcessorLoad
        let mut cores = Vec::new();
        let mut total = 0u64;
        let mut count = 0u64;
        for (k, v) in raw {
            if k.contains("hrProcessorLoad") || k.parse::<u32>().is_ok() {
                if let Some(u) = v.as_i64() {
                    cores.push(json!({ "core": k, "usage": u }));
                    total += u as u64;
                    count += 1;
                }
            }
        }
        let avg = if count > 0 { total as f64 / count as f64 } else { 0.0 };
        json!({
            "cpu_usage_percent": (avg * 100.0).round() / 100.0,
            "cpu_cores": cores,
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let total = raw.get("edgeMemTotal").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let free  = raw.get("edgeMemFree").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let used  = (total - free).max(0.0);
        json!({
            "total_gb": bytes_to_gb(total),
            "used_gb":  bytes_to_gb(used),
            "free_gb":  bytes_to_gb(free),
            "usage_percent": calculate_percentage(used, total),
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
            let units = entry.get("hrStorageAllocationUnits")
                .and_then(|v| v.as_i64()).unwrap_or(1024) as f64;
            let size  = entry.get("hrStorageSize").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used  = entry.get("hrStorageUsed").and_then(|v| v.as_i64()).unwrap_or(0) as f64;

            let total_b = size * units;
            let used_b  = used * units;
            let free_b  = (total_b - used_b).max(0.0);
            if total_b > 0.0 {
                disks.push(json!({
                    "mount": descr,
                    "total_gb": bytes_to_gb(total_b),
                    "used_gb":  bytes_to_gb(used_b),
                    "free_gb":  bytes_to_gb(free_b),
                    "usage_percent": calculate_percentage(used_b, total_b),
                }));
            }
        }
        json!(disks)
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Información del dispositivo (UBNT-MIB)
        // -----------------------------------------------------------------------
        let model   = client.get("1.3.6.1.4.1.41112.1.4.1.2.0").await;
        let version = client.get("1.3.6.1.4.1.41112.1.4.1.3.0").await;
        let mac     = client.get("1.3.6.1.4.1.41112.1.4.1.1.0").await;

        for (key, res) in [("model", &model), ("firmware_version", &version), ("mac_address", &mac)] {
            if let Some(v) = res.value.as_ref() {
                let s = v.as_string();
                if !s.is_empty() {
                    data.insert(key.into(), json!(s));
                }
            }
        }

        // -----------------------------------------------------------------------
        // Temperatura (UBNT-MIB)
        // -----------------------------------------------------------------------
        let temp = client.get("1.3.6.1.4.1.41112.1.4.1.4.0").await;
        if let Some(v) = temp.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("temperature_c".into(), json!(v));
        }

        // -----------------------------------------------------------------------
        // PoE (POWER-ETHERNET-MIB) — si está disponible
        // -----------------------------------------------------------------------
        let (poe_port_status, _) = client.bulk("1.3.6.1.2.1.105.1.1.1.3", 30).await;
        let (poe_port_class, _)  = client.bulk("1.3.6.1.2.1.105.1.1.1.3", 30).await;
        let (poe_port_power, _)  = client.bulk("1.3.6.1.2.1.105.1.1.1.6", 30).await;

        if !poe_port_status.is_empty() {
            let pwr_map: HashMap<String, i64> = poe_port_power.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();

            let poe_ports: Vec<serde_json::Value> = poe_port_status.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some(json!({
                        "port": idx,
                        "status": v.as_i64().unwrap_or(0),
                        "power_mw": pwr_map.get(&idx).copied().unwrap_or(0),
                    }))
                })
                .collect();
            data.insert("poe_ports".into(), json!(poe_ports));

            let total_poe: i64 = pwr_map.values().sum();
            data.insert("poe_total_power_mw".into(), json!(total_poe));
        }

        // -----------------------------------------------------------------------
        // VLANs (Q-BRIDGE-MIB)
        // -----------------------------------------------------------------------
        let (vlan_names, _) = client.bulk("1.3.6.1.2.1.17.7.1.4.3.1.1", 50).await;
        if !vlan_names.is_empty() {
            let vlans: Vec<serde_json::Value> = vlan_names.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some(json!({ "vlan_id": idx, "name": v.as_string() }))
                })
                .collect();
            data.insert("vlans".into(), json!(vlans));
        }

        debug!("UBNT: model={:?}, PoE={} ports",
            data.get("model"),
            data.get("poe_ports").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.41112") || // Ubiquiti
        sys_oid.starts_with("1.3.6.1.4.1.4413")     // EdgeCore/Ubiquiti EdgeSwitch
    }
}
