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
) -> serde_json::Value {
    profile.collect_vendor_specific_data(client).await
}
