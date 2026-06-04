// ==============================================================================
// NESS Relay v2.0.0 — Perfil genérico por categoría
// Equivalente Python: GenericProfile / fallback para vendors no específicos
// ==============================================================================
//
// Usa exclusivamente MIBs estándar RFC:
//   - HOST-RESOURCES-MIB: CPU, disco, memoria
//   - IF-MIB: interfaces
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{bytes_to_gb, calculate_percentage};

pub struct GenericProfile {
    vendor_name: String,
}

impl GenericProfile {
    pub fn new(name: &str) -> Self {
        Self { vendor_name: name.to_string() }
    }
}

#[async_trait]
impl DeviceProfile for GenericProfile {
    fn vendor(&self) -> &str { &self.vendor_name }
    fn vendor_display_name(&self) -> &str {
        match self.vendor_name.as_str() {
            "router" => "Router genérico SNMP",
            "switch" => "Switch genérico SNMP",
            "firewall" => "Firewall genérico SNMP",
            "ap" => "Access Point genérico SNMP",
            "printer" => "Impresora genérica SNMP",
            _ => "Generic SNMP",
        }
    }
    fn device_type(&self) -> &str {
        match self.vendor_name.as_str() {
            "router" => "router",
            "switch" => "switch",
            "firewall" => "firewall",
            "ap" => "ap",
            "printer" => "printer",
            _ => "generic",
        }
    }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HOST-RESOURCES-MIB
        m.insert("hrProcessorLoad".into(), "1.3.6.1.2.1.25.3.3.1.2".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // HOST-RESOURCES-MIB (hrStorageTable — buscar tipos RAM)
        m.insert("hrStorageDescr".into(),            "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(),  "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),             "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),             "1.3.6.1.2.1.25.2.3.1.6".into());
        // UCD-SNMP-MIB como alternativa (para Linux con snmpd)
        m.insert("memTotalReal".into(),  "1.3.6.1.4.1.2021.4.5.0".into());
        m.insert("memAvailReal".into(),  "1.3.6.1.4.1.2021.4.6.0".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("hrStorageTable".into(),            "1.3.6.1.2.1.25.2.3".into());
        m.insert("hrStorageDescr".into(),            "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(),  "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),             "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),             "1.3.6.1.2.1.25.2.3.1.6".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
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
        // Intentar primero UCD-SNMP-MIB (KB)
        let mem_total_ucd = raw.get("memTotalReal").and_then(|v| v.as_i64()).unwrap_or(0);
        if mem_total_ucd > 0 {
            let total_kb = mem_total_ucd as f64;
            let free_kb  = raw.get("memAvailReal").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used_kb  = (total_kb - free_kb).max(0.0);
            return json!({
                "total_gb": (total_kb / 1_048_576.0 * 100.0).round() / 100.0,
                "used_gb":  (used_kb  / 1_048_576.0 * 100.0).round() / 100.0,
                "free_gb":  (free_kb  / 1_048_576.0 * 100.0).round() / 100.0,
                "usage_percent": calculate_percentage(used_kb, total_kb),
            });
        }

        // Fallback: hrStorageTable (buscamos el entry con mayor size que contenga RAM/Physical)
        // En GenericProfile, se deja en 0 — el colector de performance
        // hará un bulk sobre hrStorageTable y lo indexará correctamente
        json!({
            "total_gb": 0.0, "used_gb": 0.0, "free_gb": 0.0, "usage_percent": 0.0
        })
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        let mut disks = Vec::new();
        for (idx, e) in raw {
            let descr = e.get("hrStorageDescr").map(|v| v.as_string())
                .unwrap_or_else(|| idx.clone());
            let descr_lower = descr.to_lowercase();
            // Excluir memoria RAM — solo queremos discos
            if descr_lower.contains("physical memory") || descr_lower.contains("virtual memory")
                || descr_lower.contains("swap") || descr_lower.contains("memory") {
                continue;
            }
            let units = e.get("hrStorageAllocationUnits")
                .and_then(|v| v.as_i64()).unwrap_or(4096) as f64;
            let size  = e.get("hrStorageSize").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used  = e.get("hrStorageUsed").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
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

    async fn collect_vendor_specific_data(&self, _client: &SnmpClient) -> serde_json::Value {
        json!({}) // Sin datos vendor-específicos para perfil genérico
    }
}
