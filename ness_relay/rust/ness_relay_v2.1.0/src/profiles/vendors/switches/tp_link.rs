// ==============================================================================
// NESS Relay v2.0.3 — Perfil TP-Link (JetStream / TL-SG series)
// ==============================================================================
//
// MIBs usados (TP-Link Enterprise MIB, OID base: 1.3.6.1.4.1.11863):
//   tpSysCpuUsage   = 1.3.6.1.4.1.11863.6.1.1.1.1.1.0  → % CPU escalar
//   tpSysMemUsage   = 1.3.6.1.4.1.11863.6.1.1.1.1.2.0  → % Memoria escalar
//
// Aplica a:
//   - TL-SG3428 / TL-SG3452 / TL-SG3428X (1.3.6.1.4.1.11863.5.x)
//   - T2600G / T3700G / T2700G (JetStream)
//   - SX3016F / TL-SX3008F (fiber series)
// ==============================================================================

use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{bytes_to_gb, calculate_percentage};

pub struct TpLinkProfile;

impl TpLinkProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for TpLinkProfile {
    fn vendor(&self) -> &str { "tp_link" }
    fn vendor_display_name(&self) -> &str { "TP-Link" }
    fn device_type(&self) -> &str { "switch" }

    // -----------------------------------------------------------------------
    // CPU — Scalar OID (TP-Link Enterprise MIB)
    // -----------------------------------------------------------------------
    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // tpSysCpuUsage: % de uso de CPU del sistema (INTEGER 0–100)
        m.insert(
            "cpu_usage".to_string(),
            "1.3.6.1.4.1.11863.6.1.1.1.1.1.0".to_string(),
        );
        m
    }

    // -----------------------------------------------------------------------
    // Memoria — Scalar OID (TP-Link Enterprise MIB)
    // -----------------------------------------------------------------------
    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // tpSysMemUsage: % de uso de memoria del sistema (INTEGER 0–100)
        m.insert(
            "mem_usage".to_string(),
            "1.3.6.1.4.1.11863.6.1.1.1.1.2.0".to_string(),
        );
        m
    }

    // -----------------------------------------------------------------------
    // Disco — TP-Link switches no exponen disco por SNMP estándar
    // -----------------------------------------------------------------------
    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("hrStorageTable".into(), "1.3.6.1.2.1.25.2.3".into());
        m.insert("hrStorageDescr".into(), "1.3.6.1.2.1.25.2.3.1.3".into());
        m.insert("hrStorageAllocationUnits".into(), "1.3.6.1.2.1.25.2.3.1.4".into());
        m.insert("hrStorageSize".into(),  "1.3.6.1.2.1.25.2.3.1.5".into());
        m.insert("hrStorageUsed".into(),  "1.3.6.1.2.1.25.2.3.1.6".into());
        m
    }

    // -----------------------------------------------------------------------
    // Normalización de CPU
    // -----------------------------------------------------------------------
    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        if raw.is_empty() {
            return json!({
                "cpu_usage_percent": 0.0,
                "cpu_cores": [],
                "error": "No se recibieron datos de CPU. Verifique OID tpSysCpuUsage o credenciales SNMPv3."
            });
        }

        let pct = raw.get("cpu_usage")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        json!({
            "cpu_usage_percent": pct,
            "cpu_cores": [{ "core": 0, "usage": pct }],
        })
    }

    // -----------------------------------------------------------------------
    // Normalización de Memoria
    // -----------------------------------------------------------------------
    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        if raw.is_empty() {
            return json!({
                "mem_usage_percent": 0.0,
                "error": "No se recibieron datos de memoria. Verifique OID tpSysMemUsage o credenciales SNMPv3."
            });
        }

        let pct = raw.get("mem_usage")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        json!({
            "mem_usage_percent": pct,
            "mem_total_mb": null,
            "mem_used_mb": null,
            "mem_free_mb": null,
        })
    }

    // -----------------------------------------------------------------------
    // Disco — siempre vacío en TP-Link switches
    // -----------------------------------------------------------------------
    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        let mut disks = Vec::new();
        for (idx, entry) in raw {
            let descr = entry.get("hrStorageDescr")
                .map(|v| v.as_string())
                .unwrap_or_else(|| format!("storage-{}", idx));
            let units = entry.get("hrStorageAllocationUnits")
                .and_then(|v| v.as_i64()).unwrap_or(1024) as f64;
            let size = entry.get("hrStorageSize")
                .and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used = entry.get("hrStorageUsed")
                .and_then(|v| v.as_i64()).unwrap_or(0) as f64;

            let total_bytes = size * units;
            let used_bytes  = used * units;
            let free_bytes  = (total_bytes - used_bytes).max(0.0);

            if total_bytes > 0.0 {
                disks.push(json!({
                    "mount": descr,
                    "total_gb": bytes_to_gb(total_bytes),
                    "used_gb":  bytes_to_gb(used_bytes),
                    "free_gb":  bytes_to_gb(free_bytes),
                    "usage_percent": calculate_percentage(used_bytes, total_bytes),
                }));
            }
        }
        json!(disks)
    }

    // -----------------------------------------------------------------------
    // Datos vendor-específicos: temperatura (si disponible), vlans básico
    // -----------------------------------------------------------------------
    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();
        data.insert("vendor".into(), json!("TP-Link"));

        let fw_res = client.get("1.3.6.1.4.1.11863.6.1.1.6.0").await;
        if let Some(val) = fw_res.value {
            let s = val.as_string();
            if !s.is_empty() {
                data.insert("firmware_version".into(), json!(s));
            }
        }

        // 1) Intentar serie antigua (.6.1.1...)
        let mut cpu_res = client.get("1.3.6.1.4.1.11863.6.1.1.1.1.1.0").await;
        let mut mem_res = client.get("1.3.6.1.4.1.11863.6.1.1.1.1.2.0").await;

        // 2) Si falla o no hay datos, intentar serie JetStream (.6.4.1...)
        if cpu_res.value.is_none() {
            // tpSysMonitorCpu1Minute (Unidad 1 = .1)
            cpu_res = client.get("1.3.6.1.4.1.11863.6.4.1.1.1.1.3.1").await;
            if cpu_res.value.is_none() {
                // tpSysMonitorCpu5Seconds (Unidad 1 = .1)
                cpu_res = client.get("1.3.6.1.4.1.11863.6.4.1.1.1.1.2.1").await;
            }
        }
        if mem_res.value.is_none() {
            // tpSysMonitorMemoryUtilization (Unidad 1 = .1)
            mem_res = client.get("1.3.6.1.4.1.11863.6.4.1.2.1.1.2.1").await;
        }

        if let Some(cpu_val) = cpu_res.value {
            if let Some(pct) = cpu_val.as_f64() {
                data.insert("cpu_usage_percent".into(), json!(pct));
            } else if let Some(pct) = cpu_val.as_i64() {
                data.insert("cpu_usage_percent".into(), json!(pct as f64));
            }
        }

        if let Some(mem_val) = mem_res.value {
            if let Some(pct) = mem_val.as_f64() {
                data.insert("mem_usage_percent".into(), json!(pct));
            } else if let Some(pct) = mem_val.as_i64() {
                data.insert("mem_usage_percent".into(), json!(pct as f64));
            }
        }

        debug!(
            "TP-Link vendor data: cpu={:?}%, mem={:?}%",
            data.get("cpu_usage_percent"),
            data.get("mem_usage_percent"),
        );

        json!(data)
    }

    // -----------------------------------------------------------------------
    // Post-proceso: unificar datos de vendor con performance si se obtuvieron
    // -----------------------------------------------------------------------
    fn finalize_collected_data(&self, mut data: serde_json::Value) -> serde_json::Value {
        // Si vendor_specific tiene CPU/mem y performance tiene 0.0, promover.
        if let Some(vendor) = data.get("tp_link_specific").cloned() {
            let cpu_pct = vendor.get("cpu_usage_percent").cloned();
            let mem_pct = vendor.get("mem_usage_percent").cloned();

            if let Some(perf) = data.get_mut("performance").and_then(|v| v.as_object_mut()) {
                if let Some(cpu) = perf.get_mut("cpu").and_then(|v| v.as_object_mut()) {
                    if cpu.get("cpu_usage_percent").and_then(|v| v.as_f64()).unwrap_or(0.0) == 0.0 {
                        if let Some(p) = cpu_pct { cpu.insert("cpu_usage_percent".into(), p); }
                    }
                }
                if let Some(mem) = perf.get_mut("memory").and_then(|v| v.as_object_mut()) {
                    if mem.get("mem_usage_percent").and_then(|v| v.as_f64()).unwrap_or(0.0) == 0.0 {
                        if let Some(p) = mem_pct { mem.insert("mem_usage_percent".into(), p); }
                    }
                }
                
                // TP-Link no expone memoria flash por SNMP. 
                // Insertamos un disco simulado en 0% para sobreescribir cualquier valor viejo pegado en el backend
                let mut is_empty = false;
                if let Some(disk) = perf.get("disk").and_then(|v| v.as_object()) {
                    is_empty = disk.is_empty();
                } else if let Some(disks) = perf.get("disks").and_then(|v| v.as_array()) {
                    is_empty = disks.is_empty();
                } else {
                    is_empty = true;
                }
                
                if is_empty {
                    perf.insert("disks".into(), json!([
                        {
                            "mount": "System Flash (N/A)",
                            "total_gb": 0.0,
                            "used_gb": 0.0,
                            "free_gb": 0.0,
                            "usage_percent": 0.0
                        }
                    ]));
                }
            }
        }
        data
    }

    // -----------------------------------------------------------------------
    // Detección por sysObjectID
    // -----------------------------------------------------------------------
    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        // Prefijo empresarial de TP-Link: 1.3.6.1.4.1.11863
        // Incluye: JetStream (11863.5.x), TL-SG (11863.5.x), T-series
        sys_oid.starts_with("1.3.6.1.4.1.11863")
    }
}