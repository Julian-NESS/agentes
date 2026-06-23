// ==============================================================================
// NESS Relay v2.2.0 — Perfil Huawei (VRP) Switch / Router
// ==============================================================================
//
// MIBs usados:
//   - HUAWEI-ENTITY-EXTENT-MIB (1.3.6.1.4.1.2011.5.25.31):
//       hwEntityCpuUsage    (.1.1.1.1.5)  → Tabla: % CPU por slot/board
//       hwEntityMemUsage    (.1.1.1.1.7)  → Tabla: % Memoria usada por slot
//       hwEntityMemSize     (.1.1.1.1.8)  → Tabla: Tamaño total memoria (MB)
//       hwEntityTemperature (.1.1.1.1.11) → Tabla: temperatura por slot
//   - IF-MIB estándar para interfaces
//
// NOTA IMPORTANTE: Estos OIDs son TABLAS, no valores escalares.
// El motor debe usar GETBULK (client.bulk()) para iterar la tabla.
// Se utiliza el prefijo "cpu_table" / "mem_table" para indicarle al colector
// que aplique GETBULK en lugar de GET.
//
// Aplica a: Huawei S5700, S5731, S6700, S5720, S6720, CE series.
// ==============================================================================

use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::json;
use tracing::debug;

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{bytes_to_gb, calculate_percentage};

pub struct HuaweiProfile;

impl HuaweiProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for HuaweiProfile {
    fn vendor(&self) -> &str { "huawei" }
    fn vendor_display_name(&self) -> &str { "Huawei" }
    fn device_type(&self) -> &str { "switch" }

    // -----------------------------------------------------------------------
    // CPU — hwEntityCpuUsage (TABLA indexada por entPhysicalIndex)
    // -----------------------------------------------------------------------
    fn get_cpu_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Prefijo "cpu_table" → el colector usará GETBULK
        // hwEntityCpuUsage: porcentaje de CPU por board/slot (INTEGER 0–100)
        m.insert(
            "cpu_table".to_string(),
            "1.3.6.1.4.1.2011.5.25.31.1.1.1.1.5".to_string(),
        );
        m
    }

    // -----------------------------------------------------------------------
    // Memoria — hwEntityMemUsage + hwEntityMemSize (TABLA)
    // -----------------------------------------------------------------------
    fn get_memory_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // hwEntityMemUsage: % de memoria usada por slot/board (INTEGER 0–100)
        m.insert(
            "mem_usage_table".to_string(),
            "1.3.6.1.4.1.2011.5.25.31.1.1.1.1.7".to_string(),
        );
        // hwEntityMemSize: tamaño total de memoria en MB por slot/board
        m.insert(
            "mem_size_table".to_string(),
            "1.3.6.1.4.1.2011.5.25.31.1.1.1.1.8".to_string(),
        );
        m
    }

    // -----------------------------------------------------------------------
    // Disco — Huawei VRP switches no exponen disco por SNMP estándar
    // -----------------------------------------------------------------------
    fn get_disk_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        HashMap::new()
    }

    // -----------------------------------------------------------------------
    // OIDs vendor-específicos (temperatura, fans)
    // -----------------------------------------------------------------------
    fn get_vendor_oids(&self, _sys_object_id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // hwEntityTemperature: temperatura por slot en grados Celsius
        m.insert(
            "temperature_table".to_string(),
            "1.3.6.1.4.1.2011.5.25.31.1.1.1.1.11".to_string(),
        );
        m
    }

    // -----------------------------------------------------------------------
    // Normalización de CPU
    // Los datos vienen como HashMap<índice_slot, SnmpValue(% uso)>
    // -----------------------------------------------------------------------
    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        if raw.is_empty() {
            return json!({
                "cpu_usage_percent": 0.0,
                "cpu_cores": [],
                "error": "No se encontraron datos de CPU (tabla hwEntityCpuUsage vacía)"
            });
        }

        // Construir lista de cores y calcular promedio
        let mut cores: Vec<serde_json::Value> = Vec::new();
        let mut total_pct = 0.0f64;
        let mut count = 0u64;

        let mut sorted_keys: Vec<&String> = raw.keys().collect();
        sorted_keys.sort_by(|a, b| {
            let a_n: u64 = a.parse().unwrap_or(0);
            let b_n: u64 = b.parse().unwrap_or(0);
            a_n.cmp(&b_n)
        });

        for (i, key) in sorted_keys.iter().enumerate() {
            if let Some(pct) = raw.get(*key).and_then(|v| v.as_f64()) {
                total_pct += pct;
                count += 1;
                cores.push(json!({
                    "slot": key,
                    "core": i,
                    "usage": pct,
                }));
            }
        }

        let avg = if count > 0 { total_pct / count as f64 } else { 0.0 };
        let max_usage = cores.iter()
            .filter_map(|c| c.get("usage").and_then(|v| v.as_f64()))
            .fold(0.0f64, f64::max);

        json!({
            "cpu_usage_percent": (avg * 100.0).round() / 100.0,
            "cpu_usage_max_percent": (max_usage * 100.0).round() / 100.0,
            "cpu_cores": cores,
            "slot_count": count,
        })
    }

    // -----------------------------------------------------------------------
    // Normalización de Memoria
    // mem_usage_table → HashMap<índice, % uso>
    // mem_size_table  → HashMap<índice, tamaño MB>
    // -----------------------------------------------------------------------
    fn normalize_memory_data(&self, raw: &HashMap<String, SnmpValue>) -> serde_json::Value {
        // El colector mezcla todas las claves en un único HashMap.
        // Las claves tendrán el formato que puso el colector:
        //   Para tablas: el índice numérico del slot
        // Necesitamos separar los pct de uso del tamaño en MB.
        // Sin embargo, el colector actual no separa por OID, así que
        // la única forma de tener datos diferenciados es usando
        // collect_vendor_specific_data que llamamos abajo.
        // Si hay datos en raw, asumimos que son porcentajes de uso.

        if raw.is_empty() {
            return json!({
                "mem_usage_percent": 0.0,
                "mem_total_mb": 0.0,
                "mem_used_mb": 0.0,
                "mem_free_mb": 0.0,
                "error": "No se encontraron datos de memoria (tabla hwEntityMemUsage vacía)"
            });
        }

        // Calcular promedio de % uso de memoria de todos los slots
        let mut total_pct = 0.0f64;
        let mut count = 0u64;
        let mut slots: Vec<serde_json::Value> = Vec::new();

        let mut sorted_keys: Vec<&String> = raw.keys().collect();
        sorted_keys.sort_by(|a, b| {
            let a_n: u64 = a.parse().unwrap_or(u64::MAX);
            let b_n: u64 = b.parse().unwrap_or(u64::MAX);
            a_n.cmp(&b_n)
        });

        for key in sorted_keys {
            if let Some(pct) = raw.get(key).and_then(|v| v.as_f64()) {
                total_pct += pct;
                count += 1;
                slots.push(json!({ "slot": key, "usage_percent": pct }));
            }
        }

        let avg_pct = if count > 0 { total_pct / count as f64 } else { 0.0 };

        json!({
            "mem_usage_percent": (avg_pct * 100.0).round() / 100.0,
            "mem_total_mb": null,
            "mem_used_mb": null,
            "mem_free_mb": null,
            "slots": slots,
            "slot_count": count,
        })
    }

    // -----------------------------------------------------------------------
    // Disco — siempre vacío en Huawei switches VRP
    // -----------------------------------------------------------------------
    fn normalize_disk_data(
        &self,
        _raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> serde_json::Value {
        json!([])
    }

    // -----------------------------------------------------------------------
    // Datos vendor-específicos: temperatura, memoria detallada, fans
    // -----------------------------------------------------------------------
    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
        let mut data = serde_json::Map::new();
        data.insert("vendor".into(), json!("Huawei"));

        // Intentar extraer la versión del sysDescr (1.3.6.1.2.1.1.1.0)
        let sys_descr_res = client.get("1.3.6.1.2.1.1.1.0").await;
        if let Some(val) = sys_descr_res.value {
            let descr = val.as_string();
            let descr_lower = descr.to_lowercase();
            
            if let Some(idx) = descr_lower.find("software version ") {
                let start = idx + 17;
                let version_str = descr[start..].trim_start().split(|c: char| c.is_whitespace() || c == ',').next().unwrap_or("");
                if !version_str.is_empty() {
                    data.insert("firmware_version".into(), json!(version_str));
                }
            } else if let Some(idx) = descr_lower.find("version ") {
                let start = idx + 8;
                let version_str = descr[start..].trim_start().split(|c: char| c.is_whitespace() || c == ',').next().unwrap_or("");
                if !version_str.is_empty() {
                    data.insert("firmware_version".into(), json!(version_str));
                }
            }
        }


        // --- Temperatura por slot (hwEntityTemperature) ---
        let (temp_entries, _) = client
            .bulk("1.3.6.1.4.1.2011.5.25.31.1.1.1.1.11", 32)
            .await;

        if !temp_entries.is_empty() {
            let mut temps: Vec<serde_json::Value> = Vec::new();
            let mut max_temp = i64::MIN;

            for (oid, val) in &temp_entries {
                if let Some(celsius) = val.as_i64() {
                    // Filtrar valores inválidos (0xFFFFFFFF = sensor ausente)
                    if celsius > -1000 && celsius < 200 {
                        let idx = oid.rsplit('.').next().unwrap_or("?").to_string();
                        if celsius > max_temp { max_temp = celsius; }
                        temps.push(json!({ "slot": idx, "temperature_c": celsius }));
                    }
                }
            }

            data.insert("temperatures".into(), json!(temps));
            if max_temp != i64::MIN {
                data.insert("max_temperature_c".into(), json!(max_temp));

                // Alerta de temperatura
                let temp_status = if max_temp >= 80 {
                    "critical"
                } else if max_temp >= 65 {
                    "warning"
                } else {
                    "ok"
                };
                data.insert("temperature_status".into(), json!(temp_status));
            }
        }
        
        // --- Almacenamiento Flash Real (hwFlashPartitionTable) ---
        let mut flash_total_bytes = 0.0;
        let mut flash_free_bytes = 0.0;
        
        // hwFlhParSize
        let mut flash_res = client.get("1.3.6.1.4.1.2011.6.9.1.1.4.1.1.4.1.1").await;
        if let Some(val) = flash_res.value {
            if let Some(total) = val.as_f64() {
                flash_total_bytes = total;
            } else if let Some(total) = val.as_i64() {
                flash_total_bytes = total as f64;
            }
        }
        
        // hwFlhParFreeSize
        let mut flash_free_res = client.get("1.3.6.1.4.1.2011.6.9.1.1.4.1.1.5.1.1").await;
        if let Some(val) = flash_free_res.value {
            if let Some(free) = val.as_f64() {
                flash_free_bytes = free;
            } else if let Some(free) = val.as_i64() {
                flash_free_bytes = free as f64;
            }
        }
        
        if flash_total_bytes > 0.0 {
            let used_bytes = (flash_total_bytes - flash_free_bytes).max(0.0);
            let pct = calculate_percentage(used_bytes, flash_total_bytes);
            
            data.insert("flash_total_gb".into(), json!(bytes_to_gb(flash_total_bytes)));
            data.insert("flash_used_gb".into(), json!(bytes_to_gb(used_bytes)));
            data.insert("flash_free_gb".into(), json!(bytes_to_gb(flash_free_bytes)));
            data.insert("flash_usage_percent".into(), json!(pct));
        }


        // --- CPU detallada por slot (hwEntityCpuUsage) ---
        let (cpu_entries, _) = client
            .bulk("1.3.6.1.4.1.2011.5.25.31.1.1.1.1.5", 32)
            .await;

        if !cpu_entries.is_empty() {
            let mut cpu_slots: Vec<serde_json::Value> = Vec::new();
            let mut cpu_sum = 0i64;
            let mut cpu_count = 0i64;

            for (oid, val) in &cpu_entries {
                if let Some(pct) = val.as_i64() {
                    let idx = oid.rsplit('.').next().unwrap_or("?").to_string();
                    cpu_sum += pct;
                    cpu_count += 1;
                    cpu_slots.push(json!({ "slot": idx, "cpu_usage_percent": pct }));
                }
            }

            let cpu_avg = if cpu_count > 0 { (cpu_sum as f64) / (cpu_count as f64) } else { 0.0 };
            data.insert("cpu_slots".into(), json!(cpu_slots));
            data.insert("cpu_average_percent".into(), json!(cpu_avg));
        }

        // --- Memoria detallada por slot (uso% + tamaño) ---
        let (mem_usage_entries, _) = client
            .bulk("1.3.6.1.4.1.2011.5.25.31.1.1.1.1.7", 32)
            .await;
        let (mem_size_entries, _) = client
            .bulk("1.3.6.1.4.1.2011.5.25.31.1.1.1.1.8", 32)
            .await;

        if !mem_usage_entries.is_empty() {
            // Construir mapa índice → tamaño en MB
            let size_map: HashMap<String, i64> = mem_size_entries
                .iter()
                .filter_map(|(oid, val)| {
                    val.as_i64().map(|mb| {
                        let idx = oid.rsplit('.').next().unwrap_or("?").to_string();
                        (idx, mb)
                    })
                })
                .collect();

            let mut mem_slots: Vec<serde_json::Value> = Vec::new();
            let mut total_mb_all = 0i64;
            let mut used_mb_all = 0i64;

            for (oid, val) in &mem_usage_entries {
                if let Some(pct) = val.as_i64() {
                    let idx = oid.rsplit('.').next().unwrap_or("?").to_string();
                    let total_mb = size_map.get(&idx).copied().unwrap_or(0);
                    let used_mb = (total_mb * pct) / 100;
                    let free_mb = total_mb - used_mb;

                    total_mb_all += total_mb;
                    used_mb_all += used_mb;

                    mem_slots.push(json!({
                        "slot": idx,
                        "mem_usage_percent": pct,
                        "mem_total_mb": total_mb,
                        "mem_used_mb": used_mb,
                        "mem_free_mb": free_mb,
                    }));
                }
            }

            let free_mb_all = total_mb_all - used_mb_all;
            let usage_pct_all = if total_mb_all > 0 {
                (used_mb_all * 100) / total_mb_all
            } else {
                0
            };

            data.insert("memory_slots".into(), json!(mem_slots));
            data.insert("mem_total_mb".into(), json!(total_mb_all));
            data.insert("mem_used_mb".into(), json!(used_mb_all));
            data.insert("mem_free_mb".into(), json!(free_mb_all));
            data.insert("mem_usage_percent".into(), json!(usage_pct_all));
        }

        // ----------------------------------------------------------------
        // 10GE SFP+ Uplink Ports (descubrimiento multi-MIB)
        // ----------------------------------------------------------------
        // Los switches Huawei S5731-S48P4X tienen 4 puertos 10GE SFP+ que
        // NO aparecen en IF-MIB estándar (están en MIBs propietarios Huawei).
        // Intentamos 3 estrategias en orden de preferencia:
        //   1. ENTITY-MIB::entPhysicalDescr (1.3.6.1.2.1.47.1.1.1.1.2)
        //      + entAliasMappingTable (1.3.6.1.2.1.47.1.3.2.1.2)
        //      → Mapeo entPhysicalIndex → ifIndex
        //   2. HUAWEI-IF-EXT-MIB::hwIfName (1.3.6.1.4.1.2011.5.25.41.1.1.1.1.2)
        //      → Tabla de extensiones Huawei con nombres reales
        //   3. HUAWEI-L2IF-MIB::hwL2IfPortTable (1.3.6.1.4.1.2011.5.25.42.1.1.1)
        //      → Tabla L2 de Huawei
        // Filtramos por nombre: "XGigabitEthernet", "10GE", "TE-Gigabit".
        let mut uplink_ports: Vec<serde_json::Value> = Vec::new();

        // ── Estrategia 1: ENTITY-MIB ──
        let (phys_entries, _) = client
            .bulk("1.3.6.1.2.1.47.1.1.1.1.2", 32)
            .await;
        let (alias_entries, _) = client
            .bulk("1.3.6.1.2.1.47.1.3.2.1.2", 32)
            .await;

        debug!(
            "Huawei 10GE discovery (ENTITY-MIB): phys_entries={}, alias_entries={}",
            phys_entries.len(),
            alias_entries.len()
        );

        if !phys_entries.is_empty() {
            // Construir mapa: entPhysicalIndex → ifIndex (último componente OID)
            let mut alias_map: HashMap<String, String> = HashMap::new();
            for (oid, _) in &alias_entries {
                // OID format: 1.3.6.1.2.1.47.1.3.2.1.2.<physIndex>.<ifIndex>
                let parts: Vec<&str> = oid.split('.').collect();
                if parts.len() >= 2 {
                    let phys_idx = parts[parts.len() - 2].to_string();
                    let if_idx = parts[parts.len() - 1].to_string();
                    alias_map.insert(phys_idx, if_idx);
                }
            }

            let mut uplink_ports: Vec<serde_json::Value> = Vec::new();
            for (oid, val) in &phys_entries {
                let name = val.as_string();
                // Detectar puertos 10GE por nombre
                let is_10ge = name.starts_with("XGigabitEthernet")
                    || name.starts_with("10GE")
                    || name.starts_with("TE-Gigabit")
                    || name.starts_with("TwentyFiveGigE")
                    || name.starts_with("25GE");
                if !is_10ge {
                    continue;
                }
                let phys_idx = oid.rsplit('.').next().unwrap_or("?").to_string();
                let if_idx = alias_map.get(&phys_idx).cloned().unwrap_or_default();
                uplink_ports.push(json!({
                    "phys_index": phys_idx,
                    "if_index": if_idx,
                    "name": name,
                    "media": "fiber",
                    "speed_mbps_target": 10000,
                }));
            }

            if !uplink_ports.is_empty() {
                data.insert("uplink_10ge_ports".into(), json!(uplink_ports));
                data.insert("uplink_10ge_count".into(), json!(uplink_ports.len()));

                // Intentar enriquecer con métricas de IF-MIB extendido para
                // cada puerto 10GE (velocidad, estado, contadores)
                let mut uplink_with_stats: Vec<serde_json::Value> = Vec::new();
                for mut port in uplink_ports {
                    let if_idx = port.get("if_index").and_then(|v| v.as_str()).unwrap_or("");
                    if if_idx.is_empty() {
                        uplink_with_stats.push(port);
                        continue;
                    }
                    // ifHighSpeed (1.3.6.1.2.1.31.1.1.1.15.<ifIndex>)
                    let speed_oid = format!("1.3.6.1.2.1.31.1.1.1.15.{}", if_idx);
                    let oper_oid  = format!("1.3.6.1.2.1.2.2.1.8.{}", if_idx);
                    let admin_oid = format!("1.3.6.1.2.1.2.2.1.7.{}", if_idx);

                    let speed_res = client.get(&speed_oid).await;
                    let oper_res  = client.get(&oper_oid).await;
                    let admin_res = client.get(&admin_oid).await;

                    let speed_mbps = speed_res.value
                        .as_ref()
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10000);
                    let oper_status = oper_res.value
                        .as_ref()
                        .map(|v| v.as_i64().unwrap_or(2))
                        .unwrap_or(2);  // 1=up, 2=down, 3=testing, 4=unknown
                    let admin_status = admin_res.value
                        .as_ref()
                        .map(|v| v.as_i64().unwrap_or(2))
                        .unwrap_or(2); // 1=up, 2=down, 3=testing, 4=unknown

                    if let Some(obj) = port.as_object_mut() {
                        obj.insert("speed_mbps".into(), json!(speed_mbps));
                        obj.insert("operational_status".into(), json!(oper_status));
                        obj.insert("admin_status".into(), json!(admin_status));
                        // Estado textual para el dashboard
                        let status_text = match oper_status {
                            1 => "up",
                            2 => "down",
                            3 => "testing",
                            _ => "unknown",
                        };
                        obj.insert("operational_status_text".into(), json!(status_text));
                    }
                    uplink_with_stats.push(port);
                }
                data.insert("uplink_10ge_ports".into(), json!(uplink_with_stats));
            }
        }

        // ── Estrategia 2: HUAWEI-IF-EXT-MIB (hwIfName) ──
        // Si la ENTITY-MIB no devolvió 10GE, intentar la MIB propietaria
        // de Huawei para extensiones de interfaces.
        if uplink_ports.is_empty() {
            let (hw_if_entries, _) = client
                .bulk("1.3.6.1.4.1.2011.5.25.41.1.1.1.1.2", 32)
                .await;

            debug!(
                "Huawei 10GE discovery (HUAWEI-IF-EXT-MIB hwIfName): entries={}",
                hw_if_entries.len()
            );

            for (oid, val) in &hw_if_entries {
                let name = val.as_string();
                let is_10ge = name.starts_with("XGigabitEthernet")
                    || name.starts_with("10GE")
                    || name.starts_with("TE-Gigabit")
                    || name.starts_with("TwentyFiveGigE")
                    || name.starts_with("25GE");
                if !is_10ge {
                    continue;
                }
                let if_idx = oid.rsplit('.').next().unwrap_or("?").to_string();
                uplink_ports.push(json!({
                    "if_index": if_idx,
                    "name": name,
                    "media": "fiber",
                    "speed_mbps_target": 10000,
                    "source": "HUAWEI-IF-EXT-MIB",
                }));
            }

            if !uplink_ports.is_empty() {
                data.insert("uplink_10ge_ports".into(), json!(uplink_ports.clone()));
                data.insert("uplink_10ge_count".into(), json!(uplink_ports.len()));
                data.insert("uplink_10ge_source".into(), json!("HUAWEI-IF-EXT-MIB"));
            }
        }

        // ── Estrategia 3: HUAWEI-L2IF-MIB (hwL2IfPortTable) ──
        if uplink_ports.is_empty() {
            // hwL2IfPortName: 1.3.6.1.4.1.2011.5.25.42.1.1.1.1.5
            let (hw_l2_entries, _) = client
                .bulk("1.3.6.1.4.1.2011.5.25.42.1.1.1.1.5", 32)
                .await;

            debug!(
                "Huawei 10GE discovery (HUAWEI-L2IF-MIB hwL2IfPortName): entries={}",
                hw_l2_entries.len()
            );

            for (oid, val) in &hw_l2_entries {
                let name = val.as_string();
                let is_10ge = name.starts_with("XGigabitEthernet")
                    || name.starts_with("10GE")
                    || name.starts_with("TE-Gigabit")
                    || name.starts_with("TwentyFiveGigE")
                    || name.starts_with("25GE");
                if !is_10ge {
                    continue;
                }
                let phys_idx = oid.rsplit('.').next().unwrap_or("?").to_string();
                uplink_ports.push(json!({
                    "phys_index": phys_idx,
                    "name": name,
                    "media": "fiber",
                    "speed_mbps_target": 10000,
                    "source": "HUAWEI-L2IF-MIB",
                }));
            }

            if !uplink_ports.is_empty() {
                data.insert("uplink_10ge_ports".into(), json!(uplink_ports.clone()));
                data.insert("uplink_10ge_count".into(), json!(uplink_ports.len()));
                data.insert("uplink_10ge_source".into(), json!("HUAWEI-L2IF-MIB"));
            }
        }

        // Enriquecer los puertos encontrados con estadísticas (si hay)
        if !uplink_ports.is_empty() && !data.get("uplink_10ge_ports_with_stats").is_some() {
            let mut uplink_with_stats: Vec<serde_json::Value> = Vec::new();
            for mut port in uplink_ports {
                let if_idx = port.get("if_index").and_then(|v| v.as_str()).unwrap_or("");
                if if_idx.is_empty() {
                    uplink_with_stats.push(port);
                    continue;
                }
                let speed_oid = format!("1.3.6.1.2.1.31.1.1.1.15.{}", if_idx);
                let oper_oid  = format!("1.3.6.1.2.1.2.2.1.8.{}", if_idx);
                let admin_oid = format!("1.3.6.1.2.1.2.2.1.7.{}", if_idx);

                let speed_res = client.get(&speed_oid).await;
                let oper_res  = client.get(&oper_oid).await;
                let admin_res = client.get(&admin_oid).await;

                let speed_mbps = speed_res.value.as_ref().and_then(|v| v.as_u64()).unwrap_or(10000);
                let oper_status = oper_res.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(2);
                let admin_status = admin_res.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(2);

                if let Some(obj) = port.as_object_mut() {
                    obj.insert("speed_mbps".into(), json!(speed_mbps));
                    obj.insert("operational_status".into(), json!(oper_status));
                    obj.insert("admin_status".into(), json!(admin_status));
                    let status_text = match oper_status {
                        1 => "up",
                        2 => "down",
                        3 => "testing",
                        _ => "unknown",
                    };
                    obj.insert("operational_status_text".into(), json!(status_text));
                }
                uplink_with_stats.push(port);
            }
            // Sobrescribir con la versión enriquecida (si ya existía en Estrategia 1)
            data.insert("uplink_10ge_ports".into(), json!(uplink_with_stats));
        }

        debug!(
            "Huawei vendor data: temps={}, cpu_slots={}, mem_slots={}, 10ge_uplinks={}",
            data.get("temperatures").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            data.get("cpu_slots").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            data.get("memory_slots").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            data.get("uplink_10ge_count").and_then(|v| v.as_f64()).unwrap_or(0.0) as usize,
        );

        json!(data)
    }

    // -----------------------------------------------------------------------
    // Post-proceso: unificar datos detallados de vendor con los de rendimiento
    // -----------------------------------------------------------------------
    fn finalize_collected_data(&self, mut data: serde_json::Value) -> serde_json::Value {
        // Si vendor_specific tiene datos de CPU/memoria más detallados,
        // los promovemos a la sección de performance para el dashboard.
        if let Some(vendor) = data.get("huawei_specific").cloned() {
            let cpu_avg = vendor.get("cpu_average_percent").cloned();
            let mem_pct  = vendor.get("mem_usage_percent").cloned();
            let mem_total = vendor.get("mem_total_mb").cloned();
            let mem_used  = vendor.get("mem_used_mb").cloned();
            let mem_free  = vendor.get("mem_free_mb").cloned();

            if let Some(perf) = data.get_mut("performance").and_then(|v| v.as_object_mut()) {
                if let Some(cpu) = perf.get_mut("cpu").and_then(|v| v.as_object_mut()) {
                    if let Some(avg) = cpu_avg {
                        cpu.insert("cpu_usage_percent".into(), avg);
                    }
                }
                if let Some(mem) = perf.get_mut("memory").and_then(|v| v.as_object_mut()) {
                    if let Some(p) = mem_pct  { mem.insert("mem_usage_percent".into(), p); }
                    if let Some(t) = mem_total { mem.insert("mem_total_mb".into(), t); }
                    if let Some(u) = mem_used  { mem.insert("mem_used_mb".into(), u); }
                    if let Some(f) = mem_free  { mem.insert("mem_free_mb".into(), f); }
                }
                
                // Promover disco Flash si no hay discos estándar descubiertos
                if let Some(total_gb) = vendor.get("flash_total_gb").and_then(|v| v.as_f64()) {
                    let used_gb = vendor.get("flash_used_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let free_gb = vendor.get("flash_free_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let pct = vendor.get("flash_usage_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    
                    let mut is_empty = false;
                    if let Some(disk) = perf.get("disk").and_then(|v| v.as_object()) {
                        is_empty = disk.is_empty();
                    } else if let Some(disks) = perf.get("disks").and_then(|v| v.as_array()) {
                        is_empty = disks.is_empty();
                    } else {
                        is_empty = true; // Si no hay ni `disk` ni `disks`
                    }
                    
                    if is_empty {
                        // Lo insertamos como array, payload_compat.rs lo convertirá a diccionario
                        perf.insert("disks".into(), json!([
                            {
                                "mount": "System Flash",
                                "total_gb": total_gb,
                                "used_gb": used_gb,
                                "free_gb": free_gb,
                                "usage_percent": pct
                            }
                        ]));
                    }
                }
            }
        }
        data
    }

    // -----------------------------------------------------------------------
    // Detección por sysObjectID
    // -----------------------------------------------------------------------
    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        // Prefijo empresarial de Huawei: 1.3.6.1.4.1.2011
        sys_oid.starts_with("1.3.6.1.4.1.2011")
    }
}