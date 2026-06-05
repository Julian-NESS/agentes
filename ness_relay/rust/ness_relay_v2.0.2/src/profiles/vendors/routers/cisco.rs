// ==============================================================================
// NESS Relay v2.0.0 — Perfil Cisco (stub — Phase 2)
// Equivalente Python: profiles/vendors/cisco.py (TODO placeholder)
// ==============================================================================
//
// TODO Phase 2: Implementar con:
//   - CISCO-PROCESS-MIB: cpmCPUTotal5minRev
//   - CISCO-MEMORY-POOL-MIB: ciscoMemoryPoolUsed, ciscoMemoryPoolFree
//   - CISCO-ENVMON-MIB: ciscoEnvMonTemperatureStatusValue
//   - CISCO-IF-EXTENSION-MIB
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::profiles::vendors::shared::generic::GenericProfile;

/// Perfil Cisco — actualmente delega en GenericProfile.
/// Se actualizará en la Phase 2 con los MIBs propietarios de Cisco.
pub struct CiscoProfile {
    generic: GenericProfile,
}

impl CiscoProfile {
    pub fn new() -> Self {
        Self {
            generic: GenericProfile::new("cisco"),
        }
    }
}

#[async_trait]
impl DeviceProfile for CiscoProfile {
    fn vendor(&self) -> &str { "cisco" }
    fn vendor_display_name(&self) -> &str { "Cisco" }
    fn device_type(&self) -> &str { "router" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        // Cisco genérico usa HOST-RESOURCES-MIB
        // TODO: MIBs específicos de Cisco IOS
        self.generic.get_cpu_oids(_sys_object_id)
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        // Cisco genérico usa HOST-RESOURCES-MIB
        self.generic.get_memory_oids(_sys_object_id)
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        self.generic.get_disk_oids(_sys_object_id)
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        self.generic.normalize_cpu_data(raw)
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        self.generic.normalize_memory_data(raw)
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        self.generic.normalize_disk_data(raw)
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        // TODO Phase 2: recolectar datos Cisco específicos
        json!({
            "_notice": "Perfil Cisco en Phase 2 — usando datos genéricos"
        })
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.9.")  // Cisco enterprise OID
    }
}
