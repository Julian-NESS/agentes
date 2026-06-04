// ==============================================================================
// NESS Relay v2.0.0 — Perfil MikroTik (RouterOS — router/switch)
// Equivalente Python: profiles/vendors/mikrotik.py
// ==============================================================================
//
// MIBs usados:
//   - HOST-RESOURCES-MIB: CPU (hrProcessorLoad), disco (hrStorageTable)
//   - MIKROTIK-MIB (1.3.6.1.4.1.14988): memoria, health, wireless, system
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{bytes_to_gb, calculate_percentage};

pub struct MikroTikProfile;

impl MikroTikProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for MikroTikProfile {
    fn vendor(&self) -> &str { "mikrotik" }
    fn vendor_display_name(&self) -> &str { "MikroTik RouterOS" }
    fn device_type(&self) -> &str { "router" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HOST-RESOURCES-MIB para CPU por núcleo
        m.insert("hrProcessorTable".into(), "1.3.6.1.2.1.25.3.3.1.2".into()); // hrProcessorLoad
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // MIKROTIK-MIB: memoria total y libre en bytes
        m.insert("mtxrHlTotalMemory".into(), "1.3.6.1.4.1.14988.1.1.1.17.0".into());
        m.insert("mtxrHlFreeMemory".into(),  "1.3.6.1.4.1.14988.1.1.1.18.0".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HOST-RESOURCES-MIB: tabla de almacenamiento
        m.insert("hrStorageTable".into(), "1.3.6.1.2.1.25.2.3".into());
        m.insert("hrStorageDescr".into(), "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(), "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),  "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),  "1.3.6.1.2.1.25.2.3.1.6".into());
        m
    }

    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // MIKROTIK-MIB: health e información del sistema
        m.insert("mtxrHlTemperature".into(),   "1.3.6.1.4.1.14988.1.1.1.3.0".into());
        m.insert("mtxrHlVoltage".into(),        "1.3.6.1.4.1.14988.1.1.1.19.0".into());
        m.insert("mtxrHlCurrent".into(),        "1.3.6.1.4.1.14988.1.1.1.20.0".into());
        m.insert("mtxrHlProcessorTemperature".into(), "1.3.6.1.4.1.14988.1.1.1.7.0".into());
        m.insert("mtxrHlFanSpeed1".into(),      "1.3.6.1.4.1.14988.1.1.1.9.0".into());
        m.insert("mtxrHlFanSpeed2".into(),      "1.3.6.1.4.1.14988.1.1.1.10.0".into());
        m.insert("mtxrFirmwareVersion".into(),  "1.3.6.1.4.1.14988.1.1.7.7.0".into());
        m.insert("mtxrSerialNumber".into(),     "1.3.6.1.4.1.14988.1.1.7.3.0".into());
        m.insert("mtxrBoardName".into(),        "1.3.6.1.4.1.14988.1.1.7.8.0".into());
        m.insert("mtxrLicVersion".into(),       "1.3.6.1.4.1.14988.1.1.7.4.0".into());
        // OIDs adicionales de paridad Python
        m.insert("py_migrated_oid_01".into(), "1.3.6.1.2.1.2.1.0".into());
        m.insert("py_migrated_oid_02".into(), "1.3.6.1.4.1.14988.1".into());
        m.insert("py_migrated_oid_03".into(), "1.3.6.1.4.1.14988.1.1.1.2.1.1".into());
        m.insert("py_migrated_oid_04".into(), "1.3.6.1.4.1.14988.1.1.1.3.1.6".into());
        m.insert("py_migrated_oid_05".into(), "1.3.6.1.4.1.14988.1.1.3.1.0".into());
        m.insert("py_migrated_oid_06".into(), "1.3.6.1.4.1.14988.1.1.3.10.0".into());
        m.insert("py_migrated_oid_07".into(), "1.3.6.1.4.1.14988.1.1.3.11.0".into());
        m.insert("py_migrated_oid_08".into(), "1.3.6.1.4.1.14988.1.1.3.12.0".into());
        m.insert("py_migrated_oid_09".into(), "1.3.6.1.4.1.14988.1.1.3.13.0".into());
        m.insert("py_migrated_oid_10".into(), "1.3.6.1.4.1.14988.1.1.3.17.0".into());
        m.insert("py_migrated_oid_11".into(), "1.3.6.1.4.1.14988.1.1.3.18.0".into());
        m.insert("py_migrated_oid_12".into(), "1.3.6.1.4.1.14988.1.1.3.2.0".into());
        m.insert("py_migrated_oid_13".into(), "1.3.6.1.4.1.14988.1.1.3.8.0".into());
        m.insert("py_migrated_oid_14".into(), "1.3.6.1.4.1.14988.1.1.3.9.0".into());
        m.insert("py_migrated_oid_15".into(), "1.3.6.1.4.1.14988.1.1.4.3.0".into());
        m.insert("py_migrated_oid_16".into(), "1.3.6.1.4.1.14988.1.1.4.4.0".into());
        m.insert("py_migrated_oid_17".into(), "1.3.6.1.4.1.14988.1.1.4.7.0".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        // raw contiene múltiples entradas de hrProcessorLoad (una por núcleo)
        let mut cores = Vec::new();
        let mut total = 0u64;
        let mut count = 0u64;

        // Las entradas de hrProcessorLoad están indexadas por SNMP (no por nombre directo)
        // La clave en raw podría ser "1", "2", etc. dependiendo de cómo se recolectan
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
        let total_bytes = raw.get("mtxrHlTotalMemory")
            .and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let free_bytes = raw.get("mtxrHlFreeMemory")
            .and_then(|v| v.as_i64()).unwrap_or(0) as f64;
        let used_bytes = (total_bytes - free_bytes).max(0.0);

        json!({
            "total_gb": bytes_to_gb(total_bytes),
            "used_gb":  bytes_to_gb(used_bytes),
            "free_gb":  bytes_to_gb(free_bytes),
            "usage_percent": calculate_percentage(used_bytes, total_bytes),
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
            let units = entry.get("hrStorageAllocationUnits")
                .and_then(|v| v.as_i64()).unwrap_or(1024) as f64;
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

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Health: temperatura, voltaje, fans
        // -----------------------------------------------------------------------
        let temp      = client.get("1.3.6.1.4.1.14988.1.1.1.3.0").await;
        let voltage   = client.get("1.3.6.1.4.1.14988.1.1.1.19.0").await;
        let fan1      = client.get("1.3.6.1.4.1.14988.1.1.1.9.0").await;
        let fw_ver    = client.get("1.3.6.1.4.1.14988.1.1.7.7.0").await;
        let serial    = client.get("1.3.6.1.4.1.14988.1.1.7.3.0").await;
        let board     = client.get("1.3.6.1.4.1.14988.1.1.7.8.0").await;
        let lic_ver   = client.get("1.3.6.1.4.1.14988.1.1.7.4.0").await;

        if let Some(v) = temp.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("temperature_c".into(), json!(v as f64 / 10.0));
        }
        if let Some(v) = voltage.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("voltage_v".into(), json!(v as f64 / 10.0));
        }
        if let Some(v) = fan1.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("fan_speed_rpm".into(), json!(v));
        }
        for (key, result) in [
            ("firmware_version", &fw_ver),
            ("serial_number", &serial),
            ("board_name", &board),
            ("license_version", &lic_ver),
        ] {
            if let Some(v) = result.value.as_ref() {
                let s = v.as_string();
                if !s.is_empty() {
                    data.insert(key.to_string(), json!(s));
                }
            }
        }

        // -----------------------------------------------------------------------
        // Wireless (mtxrWlStatTable — clientes WiFi)
        // -----------------------------------------------------------------------
        let (wl_clients, _) = client.bulk("1.3.6.1.4.1.14988.1.1.1.3.1.2", 30).await;
        if !wl_clients.is_empty() {
            let total_clients: u64 = wl_clients.iter()
                .filter_map(|(_, v)| v.as_u64())
                .sum();
            data.insert("wireless_clients".into(), json!(total_clients));
        }

        debug!("MikroTik: firmware={:?}, board={:?}",
            data.get("firmware_version"), data.get("board_name"));

        json!(data)
    }

    fn finalize_collected_data(&self, mut data: serde_json::Value) -> serde_json::Value {
        // Si hay datos vendor con temperatura, copiarlos al nivel raíz para el análisis
        if let Some(vendor_data) = data.get("vendor_data").cloned() {
            if let Some(temp) = vendor_data.get("temperature_c") {
                if let serde_json::Value::Object(ref mut map) = data {
                    map.insert("system_temperature_c".to_string(), temp.clone());
                }
            }
        }
        data
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.14988")
    }
}
