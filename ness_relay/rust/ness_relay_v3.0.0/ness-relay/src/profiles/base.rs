// ==============================================================================
// NESS Relay v2.0.0 — Trait base de perfiles de dispositivo
// Equivalente Python: profiles/base_profile.py (BaseDeviceProfile ABC)
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use crate::snmp::SnmpClient;
use crate::snmp::types::SnmpValue;

// ==============================================================================
// TRAIT DeviceProfile
// ==============================================================================

/// Trait base para todos los perfiles de dispositivo.
/// Equivale al ABC BaseDeviceProfile de Python.
///
/// Cada vendor implementa este trait para definir:
///  - OIDs de CPU, memoria, disco y vendor-específicos
///  - Normalización de datos SNMP a formato estándar
///  - Recolección de datos específicos del vendor
#[async_trait]
pub trait DeviceProfile: Send + Sync {
    // -----------------------------------------------------------------------
    // Identidad del perfil
    // -----------------------------------------------------------------------

    /// Nombre del vendor en minúsculas (ej. "pfsense", "fortinet").
    fn vendor(&self) -> &str;

    /// Nombre del vendor para mostrar (ej. "pfSense", "Fortinet FortiGate").
    fn vendor_display_name(&self) -> &str;

    /// Tipo de dispositivo: "firewall", "router", "switch", "ap", "generic".
    fn device_type(&self) -> &str;

    // -----------------------------------------------------------------------
    // OIDs de rendimiento
    // -----------------------------------------------------------------------

    /// OIDs específicos del vendor para CPU.
    fn get_cpu_oids(&self, sys_object_id: &str) -> HashMap<String, String>;
    
    /// Devuelve los OIDs específicos para uso de memoria
    fn get_memory_oids(&self, sys_object_id: &str) -> HashMap<String, String>;
    
    /// Devuelve los OIDs específicos para uso de disco/particiones
    fn get_disk_oids(&self, sys_object_id: &str) -> HashMap<String, String>;

    /// Devuelve OIDs propietarios del vendor (temperatura, sesiones, etc.)
    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        HashMap::new()
    }

    // -----------------------------------------------------------------------
    // Normalización de datos
    // -----------------------------------------------------------------------

    /// Convierte raw SNMP data de CPU a formato estándar:
    /// { "cpu_usage_percent": f64, "cpu_cores": [{ "core": N, "usage": f64 }] }
    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value;

    /// Convierte raw SNMP data de memoria a formato estándar:
    /// { "total_gb": f64, "used_gb": f64, "free_gb": f64, "usage_percent": f64 }
    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value;

    /// Convierte raw SNMP data de discos a formato estándar:
    /// [ { "mount": str, "total_gb": f64, "used_gb": f64, "usage_percent": f64 } ]
    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value;

    // -----------------------------------------------------------------------
    // Recolección vendor-específica (async)
    // -----------------------------------------------------------------------

    /// Recolecta datos específicos del vendor (estados de firewall, VPN, etc.).
    /// Retorna un JSON object con los datos recolectados.
    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value;

    // -----------------------------------------------------------------------
    // Hooks opcionales de post-procesado
    // -----------------------------------------------------------------------

    /// Hook llamado después de recolectar datos de rendimiento.
    /// Útil para vendors donde el CPU se calcula de los datos de memoria, etc.
    fn post_process_performance(&self, data: serde_json::Value) -> serde_json::Value {
        data
    }

    /// Hook llamado al final de toda la recolección.
    /// Permite añadir o modificar campos antes de exportar.
    fn finalize_collected_data(&self, data: serde_json::Value) -> serde_json::Value {
        data
    }

    /// Verifica si el perfil corresponde al sysObjectID del dispositivo.
    /// Útil para auto-detección de vendor.
    fn matches_sys_object_id(&self, _sys_object_id: &str) -> bool {
        false
    }
}
