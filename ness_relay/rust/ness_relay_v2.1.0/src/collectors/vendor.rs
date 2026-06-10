// ==============================================================================
// NESS Relay v2.0.0 — Colector de datos vendor-específicos
// Equivalente Python: collectors/vendor_collector.py
// ==============================================================================

use std::sync::Arc;
use crate::profiles::base::DeviceProfile;
use crate::snmp::SnmpClient;

/// Delega la recolección de datos vendor-específicos al perfil.
pub async fn collect(
    client: &SnmpClient,
    profile: &Arc<dyn DeviceProfile>,
    sys_object_id: &str,
) -> serde_json::Value {
    // Delegar al perfil para la lógica específica (e.g. tablas propietarias)
    profile.collect_vendor_specific_data(client).await
}
