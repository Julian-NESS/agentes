// ==============================================================================
// NESS Relay v2.0.0 — Perfil Cambium Networks (c_n)
// Equivalente Python: profiles/vendors/c_n.py
// ==============================================================================
//
// MIBs usados:
//   - HOST-RESOURCES-MIB: CPU (hrProcessorLoad), disco
//   - CAMBIUM-MIB (1.3.6.1.4.1.17713): CPU%, mem, radio, clients, SSSIDs, etc.
//   - Aplica a ePMP y cnPilot
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{kb_to_gb, calculate_percentage};

pub struct CambiumProfile;

impl CambiumProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for CambiumProfile {
    fn vendor(&self) -> &str { "c_n" }
    fn vendor_display_name(&self) -> &str { "Cambium Networks" }
    fn device_type(&self) -> &str { "ap" }

    fn get_cpu_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // CAMBIUM-MIB: CPU usage en porcentaje
        m.insert("cambiumCpuUsage".into(),  "1.3.6.1.4.1.17713.22.1.1.1.4.0".into());
        // HOST-RESOURCES-MIB como fallback
        m.insert("hrProcessorLoad".into(),  "1.3.6.1.2.1.25.3.3.1.2".into());
        m
    }

    fn get_memory_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // CAMBIUM-MIB: memoria en KB
        m.insert("cambiumMemTotal".into(), "1.3.6.1.4.1.17713.22.1.1.1.5.0".into());
        m.insert("cambiumMemFree".into(),  "1.3.6.1.4.1.17713.22.1.1.1.21.0".into());
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
        m.insert("py_migrated_oid_01".into(), "1.2.840.10036.1.1.1.1".into());
        m.insert("py_migrated_oid_02".into(), "1.2.840.10036.1.1.1.14".into());
        m.insert("py_migrated_oid_03".into(), "1.2.840.10036.1.1.1.9".into());
        m.insert("py_migrated_oid_04".into(), "1.2.840.10036.2.1.1.2".into());
        m.insert("py_migrated_oid_05".into(), "1.2.840.10036.4.2.1.1".into());
        m.insert("py_migrated_oid_06".into(), "1.3.6.1.2.1.2.2.1.10".into());
        m.insert("py_migrated_oid_07".into(), "1.3.6.1.2.1.2.2.1.16".into());
        m.insert("py_migrated_oid_08".into(), "1.3.6.1.2.1.2.2.1.2".into());
        m.insert("py_migrated_oid_09".into(), "1.3.6.1.2.1.2.2.1.3".into());
        m.insert("py_migrated_oid_10".into(), "1.3.6.1.2.1.2.2.1.7".into());
        m.insert("py_migrated_oid_11".into(), "1.3.6.1.2.1.2.2.1.8".into());
        m.insert("py_migrated_oid_12".into(), "1.3.6.1.2.1.31.1.1.1.1".into());
        m.insert("py_migrated_oid_13".into(), "1.3.6.1.2.1.31.1.1.1.10".into());
        m.insert("py_migrated_oid_14".into(), "1.3.6.1.2.1.31.1.1.1.6".into());
        m.insert("py_migrated_oid_15".into(), "1.3.6.1.4.1.17713.1.1.1.0".into());
        m.insert("py_migrated_oid_16".into(), "1.3.6.1.4.1.17713.1.1.2.0".into());
        m.insert("py_migrated_oid_17".into(), "1.3.6.1.4.1.17713.1.1.3.0".into());
        m.insert("py_migrated_oid_18".into(), "1.3.6.1.4.1.17713.1.1.4.0".into());
        m.insert("py_migrated_oid_19".into(), "1.3.6.1.4.1.17713.1.1.5.0".into());
        m.insert("py_migrated_oid_20".into(), "1.3.6.1.4.1.17713.1.2.1.0".into());
        m.insert("py_migrated_oid_21".into(), "1.3.6.1.4.1.17713.1.2.10.0".into());
        m.insert("py_migrated_oid_22".into(), "1.3.6.1.4.1.17713.1.2.11.0".into());
        m.insert("py_migrated_oid_23".into(), "1.3.6.1.4.1.17713.1.2.2.0".into());
        m.insert("py_migrated_oid_24".into(), "1.3.6.1.4.1.17713.1.2.3.0".into());
        m.insert("py_migrated_oid_25".into(), "1.3.6.1.4.1.17713.1.3.1.1".into());
        m.insert("py_migrated_oid_26".into(), "1.3.6.1.4.1.17713.1.3.1.2".into());
        m.insert("py_migrated_oid_27".into(), "1.3.6.1.4.1.17713.1.3.1.3".into());
        m.insert("py_migrated_oid_28".into(), "1.3.6.1.4.1.17713.1.3.1.4".into());
        m.insert("py_migrated_oid_29".into(), "1.3.6.1.4.1.17713.1.3.1.5".into());
        m.insert("py_migrated_oid_30".into(), "1.3.6.1.4.1.17713.1.3.2.1".into());
        m.insert("py_migrated_oid_31".into(), "1.3.6.1.4.1.17713.1.3.2.2".into());
        m.insert("py_migrated_oid_32".into(), "1.3.6.1.4.1.17713.1.3.2.3".into());
        m.insert("py_migrated_oid_33".into(), "1.3.6.1.4.1.17713.1.3.2.4".into());
        m.insert("py_migrated_oid_34".into(), "1.3.6.1.4.1.17713.1.3.2.5".into());
        m.insert("py_migrated_oid_35".into(), "1.3.6.1.4.1.17713.1.3.2.6".into());
        m.insert("py_migrated_oid_36".into(), "1.3.6.1.4.1.17713.1.3.3.1.0".into());
        m.insert("py_migrated_oid_37".into(), "1.3.6.1.4.1.17713.1.3.3.2.0".into());
        m.insert("py_migrated_oid_38".into(), "1.3.6.1.4.1.17713.1.3.3.3.0".into());
        m.insert("py_migrated_oid_39".into(), "1.3.6.1.4.1.17713.1.4.1.0".into());
        m.insert("py_migrated_oid_40".into(), "1.3.6.1.4.1.17713.1.4.2.1".into());
        m.insert("py_migrated_oid_41".into(), "1.3.6.1.4.1.17713.1.4.2.1.1".into());
        m.insert("py_migrated_oid_42".into(), "1.3.6.1.4.1.17713.1.4.2.1.2".into());
        m.insert("py_migrated_oid_43".into(), "1.3.6.1.4.1.17713.1.4.2.1.3".into());
        m.insert("py_migrated_oid_44".into(), "1.3.6.1.4.1.17713.1.4.2.1.4".into());
        m.insert("py_migrated_oid_45".into(), "1.3.6.1.4.1.17713.1.4.2.1.5".into());
        m.insert("py_migrated_oid_46".into(), "1.3.6.1.4.1.17713.1.4.2.1.6".into());
        m.insert("py_migrated_oid_47".into(), "1.3.6.1.4.1.17713.1.4.2.1.7".into());
        m.insert("py_migrated_oid_48".into(), "1.3.6.1.4.1.17713.1.5.1.1.1".into());
        m.insert("py_migrated_oid_49".into(), "1.3.6.1.4.1.17713.1.5.1.1.2".into());
        m.insert("py_migrated_oid_50".into(), "1.3.6.1.4.1.17713.1.5.1.1.3".into());
        m.insert("py_migrated_oid_51".into(), "1.3.6.1.4.1.17713.1.5.1.1.4".into());
        m.insert("py_migrated_oid_52".into(), "1.3.6.1.4.1.17713.1.5.1.1.5".into());
        m.insert("py_migrated_oid_53".into(), "1.3.6.1.4.1.17713.21".into());
        m.insert("py_migrated_oid_54".into(), "1.3.6.1.4.1.17713.21.1.2.1.0".into());
        m.insert("py_migrated_oid_55".into(), "1.3.6.1.4.1.17713.21.1.2.2.0".into());
        m.insert("py_migrated_oid_56".into(), "1.3.6.1.4.1.17713.21.1.2.3.0".into());
        m.insert("py_migrated_oid_57".into(), "1.3.6.1.4.1.17713.21.1.2.4.0".into());
        m.insert("py_migrated_oid_58".into(), "1.3.6.1.4.1.17713.22".into());
        m.insert("py_migrated_oid_59".into(), "1.3.6.1.4.1.17713.7".into());
        m.insert("py_migrated_oid_60".into(), "1.3.6.1.4.1.17713.7.1.4.1.0".into());
        m.insert("py_migrated_oid_61".into(), "1.3.6.1.4.1.17713.7.1.4.2.0".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        // Preferir CAMBIUM-MIB
        if let Some(cpu_pct) = raw.get("cambiumCpuUsage").and_then(|v| v.as_i64()) {
            return json!({
                "cpu_usage_percent": cpu_pct as f64,
                "cpu_cores": [{ "core": 0, "usage": cpu_pct }],
            });
        }
        // Fallback: hrProcessorLoad
        let mut total = 0u64;
        let mut count = 0u64;
        for (_, v) in raw {
            if let Some(u) = v.as_i64() {
                total += u as u64;
                count += 1;
            }
        }
        let avg = if count > 0 { total as f64 / count as f64 } else { 0.0 };
        json!({ "cpu_usage_percent": avg })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let total_kb = raw.get("cambiumMemTotal").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let free_kb  = raw.get("cambiumMemFree").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let used_kb  = (total_kb - free_kb).max(0.0);
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
        for (idx, entry) in raw {
            let descr = entry.get("hrStorageDescr").map(|v| v.as_string())
                .unwrap_or_else(|| format!("storage-{}", idx));
            let units = entry.get("hrStorageAllocationUnits")
                .and_then(|v| v.as_i64()).unwrap_or(1024) as f64;
            let size  = entry.get("hrStorageSize").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used  = entry.get("hrStorageUsed").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let total_b = size * units;
            let used_b  = used * units;
            if total_b > 0.0 {
                disks.push(json!({
                    "mount": descr,
                    "total_gb": (total_b / 1_073_741_824.0 * 100.0).round() / 100.0,
                    "used_gb":  (used_b / 1_073_741_824.0 * 100.0).round() / 100.0,
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
        // Temperatura (CAMBIUM-MIB)
        // -----------------------------------------------------------------------
        let temp_board = client.get("1.3.6.1.4.1.17713.22.1.1.1.7.0").await;
        let temp_cpu   = client.get("1.3.6.1.4.1.17713.22.1.1.1.8.0").await;

        if let Some(v) = temp_board.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("temperature_board_c".into(), json!(v));
        }
        if let Some(v) = temp_cpu.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("temperature_cpu_c".into(), json!(v));
        }

        // -----------------------------------------------------------------------
        // Radio (CAMBIUM-MIB — radio channel, tx_power, frequency, bandwidth)
        // -----------------------------------------------------------------------
        let radio_channel   = client.get("1.3.6.1.4.1.17713.22.1.2.1.1.0").await;
        let radio_tx_power  = client.get("1.3.6.1.4.1.17713.22.1.2.1.2.0").await;
        let radio_freq      = client.get("1.3.6.1.4.1.17713.22.1.2.1.3.0").await;
        let channel_util    = client.get("1.3.6.1.4.1.17713.22.1.2.1.14.0").await;
        let noise_floor     = client.get("1.3.6.1.4.1.17713.22.1.2.1.15.0").await;

        for (key, res) in [
            ("radio_channel", &radio_channel), ("radio_tx_power_dbm", &radio_tx_power),
            ("radio_freq_mhz", &radio_freq), ("channel_utilization_pct", &channel_util),
            ("noise_floor_dbm", &noise_floor),
        ] {
            if let Some(v) = res.value.as_ref().and_then(|v| v.as_i64()) {
                data.insert(key.into(), json!(v));
            }
        }

        // -----------------------------------------------------------------------
        // Clientes conectados y estadísticas
        // -----------------------------------------------------------------------
        let client_count = client.get("1.3.6.1.4.1.17713.22.1.2.1.10.0").await;
        if let Some(count) = client_count.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("connected_clients".into(), json!(count));
        }

        // Tabla de clientes (RSSI, SNR, tx_rate, rx_rate)
        let (client_rssi, _)    = client.bulk("1.3.6.1.4.1.17713.22.1.3.1.1.4", 30).await;
        let (client_snr, _)     = client.bulk("1.3.6.1.4.1.17713.22.1.3.1.1.5", 30).await;
        let (client_tx_rate, _) = client.bulk("1.3.6.1.4.1.17713.22.1.3.1.1.6", 30).await;
        let (client_rx_rate, _) = client.bulk("1.3.6.1.4.1.17713.22.1.3.1.1.7", 30).await;

        if !client_rssi.is_empty() {
            let snr_map: HashMap<String, i64> = client_snr.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();
            let txr_map: HashMap<String, i64> = client_tx_rate.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();
            let rxr_map: HashMap<String, i64> = client_rx_rate.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();

            let clients: Vec<serde_json::Value> = client_rssi.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some(json!({
                        "index": idx,
                        "rssi_dbm": v.as_i64().unwrap_or(0),
                        "snr_db": snr_map.get(&idx).copied().unwrap_or(0),
                        "tx_rate_mbps": txr_map.get(&idx).copied().unwrap_or(0),
                        "rx_rate_mbps": rxr_map.get(&idx).copied().unwrap_or(0),
                    }))
                })
                .collect();
            data.insert("clients".into(), json!(clients));
        }

        // -----------------------------------------------------------------------
        // SSIDs (CAMBIUM-MIB)
        // -----------------------------------------------------------------------
        let (ssid_names, _) = client.bulk("1.3.6.1.4.1.17713.22.1.4.1.1.2", 20).await;
        if !ssid_names.is_empty() {
            let ssids: Vec<serde_json::Value> = ssid_names.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some(json!({ "index": idx, "ssid": v.as_string() }))
                })
                .collect();
            data.insert("ssids".into(), json!(ssids));
        }

        debug!("Cambium: {} clients, {} SSIDs",
            data.get("connected_clients").and_then(|v| v.as_i64()).unwrap_or(0),
            data.get("ssids").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.17713")  // Cambium enterprise OID
    }
}
