use std::collections::HashMap;
use async_trait::async_trait;
use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};

pub struct TpLinkProfile;

#[async_trait]
impl DeviceProfile for TpLinkProfile {
    fn vendor(&self) -> &str { "tp_link" }
    fn vendor_display_name(&self) -> &str { "TP-Link" }
    fn device_type(&self) -> &str { "switch" }

    fn get_cpu_oids(&self, sys_object_id: &str) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        match sys_object_id {
            // Excepciones para modelos específicos de TP-Link
            _ => {
                // OID global por defecto para series JetStream
                oids.insert("cpu_usage".to_string(), ".1.3.6.1.4.1.11863.6.1.1.1.1.1.0".to_string());
            }
        }
        oids
    }

    fn get_memory_oids(&self, sys_object_id: &str) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        match sys_object_id {
            // Excepciones para modelos específicos de TP-Link
            _ => {
                // OID global por defecto
                oids.insert("mem_usage".to_string(), ".1.3.6.1.4.1.11863.6.1.1.1.1.2.0".to_string());
            }
        }
        oids
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> { HashMap::new() }

    async fn collect_vendor_specific_data(&self, _client: &SnmpClient) -> serde_json::Value {
        serde_json::json!({ "vendor": "TP-Link" })
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let usage = raw.get("cpu_usage").and_then(|v| v.as_f64()).unwrap_or(0.0);
        serde_json::json!({ "usage_percent": usage })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let usage = raw.get("mem_usage").and_then(|v| v.as_f64()).unwrap_or(0.0);
        serde_json::json!({ "usage_percent": usage })
    }

    fn normalize_disk_data(&self, _raw: &HashMap<String, HashMap<String, SnmpValue>>) -> serde_json::Value {
        serde_json::json!([])
    }
}