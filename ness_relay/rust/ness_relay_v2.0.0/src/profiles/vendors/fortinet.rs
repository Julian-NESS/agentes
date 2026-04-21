// ==============================================================================
// NESS Relay v2.0.0 — Perfil Fortinet FortiGate
// Equivalente Python: profiles/vendors/fortinet.py
// ==============================================================================
//
// MIBs usados:
//   - FORTINET-FORTIGATE-MIB (1.3.6.1.4.1.12356)
//   - Incluye: CPU, memoria, sesiones, HA, VPN, AV/IPS, SD-WAN
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{kb_to_gb, calculate_percentage};

pub struct FortinetProfile;

impl FortinetProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for FortinetProfile {
    fn vendor(&self) -> &str { "fortinet" }
    fn vendor_display_name(&self) -> &str { "Fortinet FortiGate" }
    fn device_type(&self) -> &str { "firewall" }

    fn get_cpu_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // FORTINET-FORTIGATE-MIB: fgSysCpuUsage = uso total de CPU en %
        m.insert("fgSysCpuUsage".into(), "1.3.6.1.4.1.12356.101.4.1.3.0".into());
        m
    }

    fn get_memory_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // FORTINET-FORTIGATE-MIB: memoria
        m.insert("fgSysMemUsage".into(),    "1.3.6.1.4.1.12356.101.4.1.4.0".into()); // % de uso
        m.insert("fgSysMemCapacity".into(), "1.3.6.1.4.1.12356.101.4.1.5.0".into()); // total en KB
        m
    }

    fn get_disk_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // FORTINET-FORTIGATE-MIB: disco en MB
        m.insert("fgSysDiskUsage".into(),    "1.3.6.1.4.1.12356.101.4.1.6.0".into()); // usado MB
        m.insert("fgSysDiskCapacity".into(), "1.3.6.1.4.1.12356.101.4.1.7.0".into()); // total MB
        m
    }

    fn get_vendor_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Paridad de OIDs con Python (migración completa)
        m.insert("py_migrated_oid_01".into(), "1.3.6.1.2.1.2.2.1.10".into());
        m.insert("py_migrated_oid_02".into(), "1.3.6.1.2.1.2.2.1.11".into());
        m.insert("py_migrated_oid_03".into(), "1.3.6.1.2.1.2.2.1.13".into());
        m.insert("py_migrated_oid_04".into(), "1.3.6.1.2.1.2.2.1.14".into());
        m.insert("py_migrated_oid_05".into(), "1.3.6.1.2.1.2.2.1.16".into());
        m.insert("py_migrated_oid_06".into(), "1.3.6.1.2.1.2.2.1.17".into());
        m.insert("py_migrated_oid_07".into(), "1.3.6.1.2.1.2.2.1.19".into());
        m.insert("py_migrated_oid_08".into(), "1.3.6.1.2.1.2.2.1.2".into());
        m.insert("py_migrated_oid_09".into(), "1.3.6.1.2.1.2.2.1.20".into());
        m.insert("py_migrated_oid_10".into(), "1.3.6.1.2.1.2.2.1.3".into());
        m.insert("py_migrated_oid_11".into(), "1.3.6.1.2.1.2.2.1.5".into());
        m.insert("py_migrated_oid_12".into(), "1.3.6.1.2.1.2.2.1.7".into());
        m.insert("py_migrated_oid_13".into(), "1.3.6.1.2.1.2.2.1.8".into());
        m.insert("py_migrated_oid_14".into(), "1.3.6.1.2.1.31.1.1.1.1".into());
        m.insert("py_migrated_oid_15".into(), "1.3.6.1.2.1.31.1.1.1.10".into());
        m.insert("py_migrated_oid_16".into(), "1.3.6.1.2.1.31.1.1.1.11".into());
        m.insert("py_migrated_oid_17".into(), "1.3.6.1.2.1.31.1.1.1.15".into());
        m.insert("py_migrated_oid_18".into(), "1.3.6.1.2.1.31.1.1.1.18".into());
        m.insert("py_migrated_oid_19".into(), "1.3.6.1.2.1.31.1.1.1.6".into());
        m.insert("py_migrated_oid_20".into(), "1.3.6.1.2.1.31.1.1.1.7".into());
        m.insert("py_migrated_oid_21".into(), "1.3.6.1.4.1.12356.100.1.1.1.0".into());
        m.insert("py_migrated_oid_22".into(), "1.3.6.1.4.1.12356.101.1".into());
        m.insert("py_migrated_oid_23".into(), "1.3.6.1.4.1.12356.101.12.2.2.1.21".into());
        m.insert("py_migrated_oid_24".into(), "1.3.6.1.4.1.12356.101.4.1.1.0".into());
        m.insert("py_migrated_oid_25".into(), "1.3.6.1.4.1.12356.101.4.1.14.0".into());
        m.insert("py_migrated_oid_26".into(), "1.3.6.1.4.1.12356.101.4.1.6".into());
        m.insert("py_migrated_oid_27".into(), "1.3.6.1.4.1.12356.101.4.1.7".into());
        m.insert("py_migrated_oid_28".into(), "1.3.6.1.4.1.12356.101.4.9.1.1.2".into());
        m.insert("py_migrated_oid_29".into(), "1.3.6.1.4.1.12356.101.4.9.1.1.3".into());
        m.insert("py_migrated_oid_30".into(), "1.3.6.1.4.1.12356.101.4.9.2.1.13".into());
        m.insert("py_migrated_oid_31".into(), "1.3.6.1.4.1.12356.101.4.9.2.1.14".into());
        m.insert("py_migrated_oid_32".into(), "1.3.6.1.4.1.12356.101.4.9.2.1.4".into());
        m.insert("py_migrated_oid_33".into(), "1.3.6.1.4.1.12356.101.4.9.2.1.8".into());
        m.insert("py_migrated_oid_34".into(), "1.3.6.1.4.1.12356.101.4.9.3.1.2".into());
        m.insert("py_migrated_oid_35".into(), "1.3.6.1.4.1.12356.101.4.9.3.1.4".into());
        m.insert("py_migrated_oid_36".into(), "1.3.6.1.4.1.12356.101.4.9.3.1.5".into());
        m.insert("py_migrated_oid_37".into(), "1.3.6.1.4.1.12356.101.4.9.3.1.6".into());
        m.insert("py_migrated_oid_38".into(), "1.3.6.1.4.1.12356.101.4.9.3.1.7".into());
        m.insert("py_migrated_oid_39".into(), "1.3.6.1.4.1.12356.101.8.2.1.1.0".into());
        m.insert("py_migrated_oid_40".into(), "1.3.6.1.4.1.12356.101.9.2.1.2.0".into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let cpu_pct = raw.get("fgSysCpuUsage")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        json!({
            "cpu_usage_percent": cpu_pct,
            "cpu_cores": [{ "core": 0, "usage": cpu_pct }],
        })
    }

    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        let usage_pct = raw.get("fgSysMemUsage")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        let total_kb = raw.get("fgSysMemCapacity")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as f64;
        let used_kb = total_kb * usage_pct / 100.0;
        let free_kb = (total_kb - used_kb).max(0.0);

        json!({
            "total_gb": kb_to_gb(total_kb),
            "used_gb":  kb_to_gb(used_kb),
            "free_gb":  kb_to_gb(free_kb),
            "usage_percent": usage_pct,
        })
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        // Fortinet usa el dict genérico "disk" con fgSysDiskUsage y fgSysDiskCapacity
        // Como son globales, los buscamos en el primer entry
        if let Some(disk) = raw.values().next() {
            let used_mb = disk.get("fgSysDiskUsage")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as f64;
            let total_mb = disk.get("fgSysDiskCapacity")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as f64;
            let free_mb = (total_mb - used_mb).max(0.0);

            return json!([{
                "mount": "/",
                "total_gb": (total_mb / 1024.0 * 100.0).round() / 100.0,
                "used_gb":  (used_mb  / 1024.0 * 100.0).round() / 100.0,
                "free_gb":  (free_mb  / 1024.0 * 100.0).round() / 100.0,
                "usage_percent": calculate_percentage(used_mb, total_mb),
            }]);
        }
        json!([])
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();

        // -----------------------------------------------------------------------
        // Sesiones IPv4/IPv6
        // -----------------------------------------------------------------------
        let ipv4_sessions = client.get("1.3.6.1.4.1.12356.101.4.1.8.0").await;
        let ipv6_sessions = client.get("1.3.6.1.4.1.12356.101.4.1.9.0").await;
        let session_rate  = client.get("1.3.6.1.4.1.12356.101.4.1.11.0").await;

        data.insert("sessions_ipv4".into(), json!(
            ipv4_sessions.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0)
        ));
        data.insert("sessions_ipv6".into(), json!(
            ipv6_sessions.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0)
        ));
        data.insert("session_rate".into(), json!(
            session_rate.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0)
        ));

        // -----------------------------------------------------------------------
        // HA (High Availability)
        // -----------------------------------------------------------------------
        let ha_mode      = client.get("1.3.6.1.4.1.12356.101.13.1.1.0").await;
        let ha_group     = client.get("1.3.6.1.4.1.12356.101.13.1.2.0").await;
        let ha_priority  = client.get("1.3.6.1.4.1.12356.101.13.1.3.0").await;

        let ha_mode_val = ha_mode.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0);
        if ha_mode_val > 0 {
            data.insert("ha_mode".into(), json!(ha_mode_val));
            data.insert("ha_group".into(), json!(
                ha_group.value.as_ref().map(|v| v.as_string()).unwrap_or_default()
            ));
            data.insert("ha_priority".into(), json!(
                ha_priority.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0)
            ));
        }

        // -----------------------------------------------------------------------
        // VPN Tunnels (fgVpnTunTable)
        // -----------------------------------------------------------------------
        let (tun_names, _)   = client.bulk("1.3.6.1.4.1.12356.101.12.2.2.1.3", 50).await;
        let (tun_status, _)  = client.bulk("1.3.6.1.4.1.12356.101.12.2.2.1.20", 50).await;
        let (tun_in_oct, _)  = client.bulk("1.3.6.1.4.1.12356.101.12.2.2.1.18", 50).await;
        let (tun_out_oct, _) = client.bulk("1.3.6.1.4.1.12356.101.12.2.2.1.19", 50).await;

        let name_map: HashMap<String, String> = tun_names.into_iter()
            .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_string())))
            .collect();
        let status_map: HashMap<String, i64> = tun_status.into_iter()
            .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
            .collect();
        let in_map: HashMap<String, u64> = tun_in_oct.into_iter()
            .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
            .collect();
        let out_map: HashMap<String, u64> = tun_out_oct.into_iter()
            .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
            .collect();

        let mut vpn_tunnels = Vec::new();
        for (idx, name) in &name_map {
            vpn_tunnels.push(json!({
                "name": name,
                "status": status_map.get(idx).copied().unwrap_or(0),
                "status_text": if status_map.get(idx).copied().unwrap_or(0) == 1 { "up" } else { "down" },
                "in_octets": in_map.get(idx).copied().unwrap_or(0),
                "out_octets": out_map.get(idx).copied().unwrap_or(0),
            }));
        }
        data.insert("vpn_tunnels".into(), json!(vpn_tunnels));
        data.insert("vpn_tunnel_count".into(), json!(vpn_tunnels.len()));

        // -----------------------------------------------------------------------
        // AV/IPS detections
        // -----------------------------------------------------------------------
        let av_detections  = client.get("1.3.6.1.4.1.12356.101.8.2.1.2.0").await;
        let ips_detections = client.get("1.3.6.1.4.1.12356.101.9.2.1.1.0").await;

        if let Some(v) = av_detections.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("av_detections".into(), json!(v));
        }
        if let Some(v) = ips_detections.value.as_ref().and_then(|v| v.as_i64()) {
            data.insert("ips_detections".into(), json!(v));
        }

        // -----------------------------------------------------------------------
        // SD-WAN (fgVWanLinkHealthMonitorTable)
        // -----------------------------------------------------------------------
        let (sdwan_names, _)        = client.bulk("1.3.6.1.4.1.12356.101.4.9.2.1.2", 30).await;
        let (sdwan_latency, _)      = client.bulk("1.3.6.1.4.1.12356.101.4.9.2.1.5", 30).await;
        let (sdwan_jitter, _)       = client.bulk("1.3.6.1.4.1.12356.101.4.9.2.1.6", 30).await;
        let (sdwan_pkt_loss, _)     = client.bulk("1.3.6.1.4.1.12356.101.4.9.2.1.7", 30).await;
        let (sdwan_bw_in, _)        = client.bulk("1.3.6.1.4.1.12356.101.4.9.2.1.9", 30).await;
        let (sdwan_bw_out, _)       = client.bulk("1.3.6.1.4.1.12356.101.4.9.2.1.10", 30).await;

        if !sdwan_names.is_empty() {
            let lat_map: HashMap<String, i64> = sdwan_latency.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();
            let jit_map: HashMap<String, i64> = sdwan_jitter.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();
            let pkt_map: HashMap<String, i64> = sdwan_pkt_loss.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_i64().unwrap_or(0))))
                .collect();
            let bw_in_map: HashMap<String, u64> = sdwan_bw_in.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
                .collect();
            let bw_out_map: HashMap<String, u64> = sdwan_bw_out.into_iter()
                .filter_map(|(oid, v)| Some((oid.rsplit('.').next()?.to_string(), v.as_u64().unwrap_or(0))))
                .collect();

            let mut sdwan_links = Vec::new();
            for (idx, name) in sdwan_names {
                let idx_key = idx.rsplit('.').next().unwrap_or("").to_string();
                sdwan_links.push(json!({
                    "name": name.as_string(),
                    "latency_ms": lat_map.get(&idx_key).copied().unwrap_or(0),
                    "jitter_ms": jit_map.get(&idx_key).copied().unwrap_or(0),
                    "packet_loss_pct": pkt_map.get(&idx_key).copied().unwrap_or(0),
                    "bandwidth_in_kbps": bw_in_map.get(&idx_key).copied().unwrap_or(0),
                    "bandwidth_out_kbps": bw_out_map.get(&idx_key).copied().unwrap_or(0),
                }));
            }
            data.insert("sdwan_links".into(), json!(sdwan_links));
        }

        debug!("Fortinet: {} VPN tunnels, {} SD-WAN links",
            vpn_tunnels.len(),
            data.get("sdwan_links").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.12356")
    }
}
