// ==============================================================================
// NESS Relay v2.0.3 — Perfil Sophos XGS (SFOS)
// ==============================================================================
//
// MIBs usados:
//   - SFOS-MIB (1.3.6.1.4.1.2604.5): CPU, memoria, sesiones, servicios
//   - Aplica a: Sophos XGS 2300, XGS 3100, XGS 4300, XG series
//
// OIDs globales — compatibles con todas las series XG/XGS bajo SFOS.
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};

pub struct SophosProfile;

impl SophosProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for SophosProfile {
    fn vendor(&self) -> &str { "sophos" }
    fn vendor_display_name(&self) -> &str { "Sophos Firewall" }
    fn device_type(&self) -> &str { "firewall" }

    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // SFOS-MIB: sfosDeviceCPUPercentage (global, aplica a todas las series XG/XGS)
        m.insert("sfosCpuPercentage".into(), "1.3.6.1.4.1.2604.5.1.1.0".into());
        m
    }

    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // SFOS-MIB: sfosDeviceMemoryPercentage (global)
        m.insert("sfosMemPercentage".into(), "1.3.6.1.4.1.2604.5.1.2.0".into());
        m
    }

    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // SFOS-MIB: sfosDeviceDiskPercentage (global)
        m.insert("sfosDiskPercentage".into(), "1.3.6.1.4.1.2604.5.1.3.0".into());
        m
    }

    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Sesiones y usuarios
        m.insert("sfosLiveUsers".into(),        "1.3.6.1.4.1.2604.5.1.4.0".into());
        m.insert("sfosHTTPHits".into(),         "1.3.6.1.4.1.2604.5.1.5.0".into());
        m.insert("sfosActiveConnections".into(), "1.3.6.1.4.1.2604.5.1.6.0".into());
        // Servicios
        m.insert("sfosPoP3Service".into(),   "1.3.6.1.4.1.2604.5.1.7.0".into());
        m.insert("sfosIMAPService".into(),   "1.3.6.1.4.1.2604.5.1.8.0".into());
        m.insert("sfosSMTPService".into(),   "1.3.6.1.4.1.2604.5.1.9.0".into());
        m.insert("sfosFTPService".into(),    "1.3.6.1.4.1.2604.5.1.10.0".into());
        m.insert("sfosHTTPService".into(),   "1.3.6.1.4.1.2604.5.1.11.0".into());
        m.insert("sfosAVService".into(),     "1.3.6.1.4.1.2604.5.1.12.0".into());
        m.insert("sfosASService".into(),     "1.3.6.1.4.1.2604.5.1.13.0".into());
        m.insert("sfosDNSService".into(),    "1.3.6.1.4.1.2604.5.1.14.0".into());
        m.insert("sfosHAService".into(),     "1.3.6.1.4.1.2604.5.1.15.0".into());
        m.insert("sfosIPSService".into(),    "1.3.6.1.4.1.2604.5.1.16.0".into());
        m.insert("sfosApacheService".into(), "1.3.6.1.4.1.2604.5.1.17.0".into());
        m.insert("sfosSSLVPNService".into(), "1.3.6.1.4.1.2604.5.1.21.0".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let cpu_pct = raw.get("sfosCpuPercentage")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        json!({
            "cpu_usage_percent": cpu_pct,
            "cpu_cores": [{ "core": 0, "usage": cpu_pct }],
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let mem_pct = raw.get("sfosMemPercentage")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        json!({
            "usage_percent": mem_pct,
        })
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        // Sophos reporta un solo porcentaje global de disco
        if let Some(entry) = raw.values().next() {
            let disk_pct = entry.get("sfosDiskPercentage")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as f64;
            return json!([{
                "mount": "/",
                "usage_percent": disk_pct,
            }]);
        }
        json!([])
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Sesiones y usuarios activos
        // -----------------------------------------------------------------------
        let live_users = client.get("1.3.6.1.4.1.2604.5.1.4.0").await;
        let http_hits  = client.get("1.3.6.1.4.1.2604.5.1.5.0").await;
        let active_conn = client.get("1.3.6.1.4.1.2604.5.1.6.0").await;

        if let Some(v) = live_users.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("live_users".into(), json!(v));
        }
        if let Some(v) = http_hits.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("http_hits".into(), json!(v));
        }
        if let Some(v) = active_conn.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("active_connections".into(), json!(v));
        }

        // -----------------------------------------------------------------------
        // Estado de servicios
        // -----------------------------------------------------------------------
        let service_oids = [
            ("pop3",    "1.3.6.1.4.1.2604.5.1.7.0"),
            ("imap",    "1.3.6.1.4.1.2604.5.1.8.0"),
            ("smtp",    "1.3.6.1.4.1.2604.5.1.9.0"),
            ("ftp",     "1.3.6.1.4.1.2604.5.1.10.0"),
            ("http",    "1.3.6.1.4.1.2604.5.1.11.0"),
            ("av",      "1.3.6.1.4.1.2604.5.1.12.0"),
            ("as",      "1.3.6.1.4.1.2604.5.1.13.0"),
            ("dns",     "1.3.6.1.4.1.2604.5.1.14.0"),
            ("ha",      "1.3.6.1.4.1.2604.5.1.15.0"),
            ("ips",     "1.3.6.1.4.1.2604.5.1.16.0"),
            ("apache",  "1.3.6.1.4.1.2604.5.1.17.0"),
            ("ssl_vpn", "1.3.6.1.4.1.2604.5.1.21.0"),
        ];

        let mut services = serde_json::Map::new();
        for (name, oid) in &service_oids {
            let res = client.get(oid).await;
            if let Some(v) = res.value.as_ref() {
                let status_text = v.as_string();
                let is_running = status_text.to_lowercase().contains("running")
                    || status_text == "1";
                services.insert(
                    (*name).into(),
                    json!({
                        "status": status_text,
                        "running": is_running,
                    }),
                );
            }
        }
        if !services.is_empty() {
            data.insert("services".into(), json!(services));
        }

        debug!("Sophos: live_users={}, active_conn={}, services={}",
            data.get("live_users").and_then(|v| v.as_i64()).unwrap_or(0),
            data.get("active_connections").and_then(|v| v.as_i64()).unwrap_or(0),
            services.len());

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.2604")  // Sophos enterprise OID
    }
}
