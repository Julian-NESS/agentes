use std::collections::HashMap;
use async_trait::async_trait;
use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};

pub struct DatacomProfile;

#[async_trait]
impl DeviceProfile for DatacomProfile {
    fn vendor(&self) -> &str { "datacomm" }
    fn vendor_display_name(&self) -> &str { "Datacom" }
    fn device_type(&self) -> &str { "switch" }

    fn get_cpu_oids(&self, sys_object_id: &str) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        match sys_object_id {
            // Excepciones para modelos específicos de Datacom
            _ => {
                // OID estándar para CPU en Datacom
                oids.insert("cpu_usage".to_string(), ".1.3.6.1.4.1.3709.3.5.2.1.1.1.1.0".to_string());
            }
        }
        oids
    }

    fn get_memory_oids(&self, sys_object_id: &str) -> HashMap<String, String> {
        let mut oids = HashMap::new();
        match sys_object_id {
            // Excepciones para modelos específicos de Datacom
            _ => {
                // OID estándar para Memoria en Datacom
                oids.insert("mem_usage".to_string(), ".1.3.6.1.4.1.3709.3.5.2.1.1.1.2.0".to_string());
            }
        }
        oids
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> { HashMap::new() }

    async fn collect_vendor_specific_data(&self, _client: &SnmpClient) -> serde_json::Value {
        serde_json::json!({ "vendor": "Datacom" })
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