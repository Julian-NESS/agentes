use std::collections::HashMap;
use async_trait::async_trait;
use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};

pub struct DellProfile;

#[async_trait]
impl DeviceProfile for DellProfile {
    fn vendor(&self) -> &str { "dell" }
    fn vendor_display_name(&self) -> &str { "Dell Networking" }
    fn device_type(&self) -> &str { "switch" }

    fn get_cpu_oids(&self) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        oids.insert("cpu_usage".to_string(), ".1.3.6.1.4.1.6027.3.26.1.4.4.1.5".to_string());
        oids
    }

    fn get_memory_oids(&self) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        oids.insert("mem_usage".to_string(), ".1.3.6.1.4.1.6027.3.26.1.4.4.1.6".to_string());
        oids
    }

    fn get_disk_oids(&self) -> HashMap<String, String> { HashMap::new() }

    async fn collect_vendor_specific_data(&self, _client: &SnmpClient) -> serde_json::Value {
        serde_json::json!({ "vendor": "Dell" })
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let val = raw.get("cpu_usage").and_then(|v| v.as_f64()).unwrap_or(0.0);
        serde_json::json!({ "usage_percent": val })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let val = raw.get("mem_usage").and_then(|v| v.as_f64()).unwrap_or(0.0);
        serde_json::json!({ "usage_percent": val })
    }

    fn normalize_disk_data(&self, _raw: &HashMap<String, HashMap<String, SnmpValue>>) -> serde_json::Value {
        serde_json::json!([])
    }
}