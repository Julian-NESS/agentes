// ==============================================================================
// NESS Relay v2.0.0 — Colector de información del sistema
// Equivalente Python: collectors/system_collector.py
// ==============================================================================
//
// Consulta OIDs del sistema (sysDescr, sysName, sysUpTime, etc.)
// via SNMP GET individual.
// ==============================================================================

use serde_json::json;
use tracing::debug;

use crate::snmp::SnmpClient;
use crate::utils::conversions::format_uptime;
use crate::utils::helpers::{now_iso, now_iso_utc};
use crate::profiles::standard_oids::system_oids;

/// Recolecta información básica del sistema vía SNMP.
/// Retorna un JSON object con los campos del sistema.
pub async fn collect(client: &SnmpClient) -> serde_json::Value {
    let oids = system_oids();
    let mut data = serde_json::Map::new();

    data.insert("timestamp".into(), json!(now_iso()));
    data.insert("collection_time_utc".into(), json!(now_iso_utc()));

    // Consultar todos los OIDs del sistema
    let sys_descr = client.get(oids["sysDescr"]).await;
    let sys_name  = client.get(oids["sysName"]).await;
    let sys_loc   = client.get(oids["sysLocation"]).await;
    let sys_cont  = client.get(oids["sysContact"]).await;
    let sys_up    = client.get(oids["sysUpTime"]).await;
    let sys_oid   = client.get(oids["sysObjectID"]).await;

    data.insert("sys_descr".into(), json!(
        sys_descr.value.as_ref().map(|v| v.as_string()).unwrap_or_default()
    ));
    data.insert("sys_name".into(), json!(
        sys_name.value.as_ref().map(|v| v.as_string()).unwrap_or_default()
    ));
    data.insert("sys_location".into(), json!(
        sys_loc.value.as_ref().map(|v| v.as_string()).unwrap_or_default()
    ));
    data.insert("sys_contact".into(), json!(
        sys_cont.value.as_ref().map(|v| v.as_string()).unwrap_or_default()
    ));
    data.insert("sys_object_id".into(), json!(
        sys_oid.value.as_ref().map(|v| v.as_string()).unwrap_or_default()
    ));

    // Uptime: TimeTicks (centisegundos) → formato estructurado
    let uptime_raw = sys_up.value.as_ref().and_then(|v| v.as_u64()).unwrap_or(0);
    data.insert("uptime_raw".into(), json!(uptime_raw));
    data.insert("uptime".into(), format_uptime(uptime_raw));

    debug!("Sistema: name={}, uptime={}s",
        data.get("sys_name").and_then(|v| v.as_str()).unwrap_or(""),
        uptime_raw / 100
    );

    json!(data)
}
