use std::collections::HashMap;
use async_trait::async_trait;
use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};

pub struct SophosProfile;

#[async_trait]
impl DeviceProfile for SophosProfile {
    fn vendor(&self) -> &str { "sophos" }
    fn vendor_display_name(&self) -> &str { "Sophos XGS" }
    fn device_type(&self) -> &str { "firewall" }

    fn get_cpu_oids(&self) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        oids.insert("cpu_usage".to_string(), ".1.3.6.1.4.1.21067.2.1.2.4.1.0".to_string());
        oids
    }

    fn get_memory_oids(&self) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        oids.insert("mem_usage".to_string(), ".1.3.6.1.4.1.21067.2.1.2.4.2.0".to_string());
        oids
    }

    fn get_disk_oids(&self) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        oids.insert("disk_usage".to_string(), ".1.3.6.1.4.1.21067.2.1.2.4.3.0".to_string());
        oids
    }

    async fn collect_vendor_specific_data(&self, _client: &SnmpClient) -> serde_json::Value {
        serde_json::json!({ "vendor": "Sophos" })
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let usage = raw.get("cpu_usage").and_then(|v| v.as_f64()).unwrap_or(0.0);
        serde_json::json!({ "usage_percent": usage })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let usage = raw.get("mem_usage").and_then(|v| v.as_f64()).unwrap_or(0.0);
        serde_json::json!({ "usage_percent": usage })
    }

    fn normalize_disk_data(&self, raw: &HashMap<String, HashMap<String, SnmpValue>>) -> serde_json::Value {
        // Sophos devuelve un valor directo de porcentaje
        serde_json::json!(raw.values().next().and_then(|d| d.get("disk_usage")).and_then(|v| v.as_f64()).unwrap_or(0.0))
    }
}