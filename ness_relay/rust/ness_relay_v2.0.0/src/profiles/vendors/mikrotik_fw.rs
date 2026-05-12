// ==============================================================================
// NESS Relay v2.0.0 — Perfil MikroTik Firewall (RouterOS — modo firewall/gateway)
// Equivalente Python: profiles/vendors/mikrotik_fw.py
// ==============================================================================
//
// Diferencias con MikroTik router:
//   - device_type = "firewall"
//   - Memoria se extrae de hrStorageTable buscando "main memory"
//   - Agrega: Netwatch (monitoring de hosts), Queue Simple table
//   - Agrega: detección de interfaces WAN
// ==============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::{json, Value, Map};

use crate::profiles::base::DeviceProfile;
use crate::snmp::{SnmpClient, types::SnmpValue};
use crate::utils::conversions::{bytes_to_gb, calculate_percentage};

// ==============================================================================
// WAN INTERFACE PATTERNS — EXPANDIDO A 44+ PATRONES
// ==============================================================================

const WAN_INTERFACE_PATTERNS: &[&str] = &[
    // Patrones básicos Ethernet
    "ether1", "ether-wan", "wan",
    // SFP/Fiber
    "sfp1", "sfp-sfpplus1", "sfpplus1", "sfp-", "sfpplus",
    // Internet/ISP
    "internet", "isp", "uplink", "externa", "external", "primary", "secondary", "backup",
    "principal", "respaldo",
    // PPP/L2TP/PPTP
    "pppoe", "pppoe-out", "pppoe-wan", "pptp", "l2tp", "dhcp-wan",
    // VLAN WAN
    "vlan-wan", "vlan_wan", "vlan999",
    // ISP Providers
    "etb", "tigo", "claro", "movistar", "azteca", "att", "une", "millicom",
    "telefonica", "supercanal", "edatel", "coltel", "directv", "starlink", "internexa",
    // Tecnologías de conexión
    "fibra", "fiber", "dsl", "adsl", "cable", "mpls", "lte", "4g", "5g", "celular",
    // Legacy patterns
    "bridge-wan",
];

// ==============================================================================
// OID CONSTANTES — 44+ CONSTANTES PARA COLECCIÓN ESTRUCTURADA
// ==============================================================================

const OID_HR_PROCESSOR_LOAD: &str = "1.3.6.1.2.1.25.3.3.1.2";

const OID_HR_STORAGE_TABLE: &str = "1.3.6.1.2.1.25.2.3";
const OID_HR_STORAGE_DESCR: &str = "1.3.6.1.2.1.25.2.3.1.3";
const OID_HR_STORAGE_ALLOC_UNITS: &str = "1.3.6.1.2.1.25.2.3.1.4";
const OID_HR_STORAGE_SIZE: &str = "1.3.6.1.2.1.25.2.3.1.5";
const OID_HR_STORAGE_USED: &str = "1.3.6.1.2.1.25.2.3.1.6";

const OID_IF_NUMBER: &str = "1.3.6.1.2.1.2.1.0";
const OID_IF_DESCR: &str = "1.3.6.1.2.1.2.2.1.2";
const OID_IF_SPEED: &str = "1.3.6.1.2.1.2.2.1.5";
const OID_IF_ADMIN_STATUS: &str = "1.3.6.1.2.1.2.2.1.7";
const OID_IF_OPER_STATUS: &str = "1.3.6.1.2.1.2.2.1.8";
const OID_IF_IN_ERRORS: &str = "1.3.6.1.2.1.2.2.1.14";
const OID_IF_OUT_ERRORS: &str = "1.3.6.1.2.1.2.2.1.20";
const OID_IF_IN_DISCARDS: &str = "1.3.6.1.2.1.2.2.1.13";
const OID_IF_OUT_DISCARDS: &str = "1.3.6.1.2.1.2.2.1.19";
const OID_IF_NAME: &str = "1.3.6.1.2.1.31.1.1.1.1";
const OID_IF_ALIAS: &str = "1.3.6.1.2.1.31.1.1.1.18";
const OID_IF_HC_IN_OCTETS: &str = "1.3.6.1.2.1.31.1.1.1.6";
const OID_IF_HC_OUT_OCTETS: &str = "1.3.6.1.2.1.31.1.1.1.10";
const OID_IF_HC_IN_UCAST: &str = "1.3.6.1.2.1.31.1.1.1.7";
const OID_IF_HC_OUT_UCAST: &str = "1.3.6.1.2.1.31.1.1.1.11";
const OID_IF_HIGH_SPEED: &str = "1.3.6.1.2.1.31.1.1.1.15";

const OID_NETWATCH_NAME: &str = "1.3.6.1.4.1.14988.1.1.8.1.1.2";
const OID_NETWATCH_IP: &str = "1.3.6.1.4.1.14988.1.1.8.1.1.3";
const OID_NETWATCH_INTERVAL: &str = "1.3.6.1.4.1.14988.1.1.8.1.1.4";
const OID_NETWATCH_TIMEOUT: &str = "1.3.6.1.4.1.14988.1.1.8.1.1.5";
const OID_NETWATCH_STATUS: &str = "1.3.6.1.4.1.14988.1.1.8.1.1.6";

const OID_QUEUE_NAME: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.2";
const OID_QUEUE_SRC_ADDR: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.3";
const OID_QUEUE_DST_ADDR: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.4";
const OID_QUEUE_INTERFACE: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.5";
const OID_QUEUE_TX_BYTES: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.7";
const OID_QUEUE_TX_PACKETS: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.8";
const OID_QUEUE_RX_BYTES: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.9";
const OID_QUEUE_RX_PACKETS: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.10";
const OID_QUEUE_TX_DROP: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.11";
const OID_QUEUE_RX_DROP: &str = "1.3.6.1.4.1.14988.1.1.2.1.1.12";

const OID_MTXR_FIRMWARE_VERSION: &str = "1.3.6.1.4.1.14988.1.1.4.4.0";
const OID_MTXR_LICENSE_ID: &str = "1.3.6.1.4.1.14988.1.1.4.3.0";
const OID_MTXR_SERIAL_NUMBER: &str = "1.3.6.1.4.1.14988.1.1.7.3.0";
const OID_MTXR_FIRMWARE_UPGRADE: &str = "1.3.6.1.4.1.14988.1.1.4.7.0";
const OID_MTXR_BOARD_NAME: &str = "1.3.6.1.4.1.14988.1.1.7.8.0";
const OID_MTXR_HL_TEMPERATURE: &str = "1.3.6.1.4.1.14988.1.1.3.10.0";
const OID_MTXR_HL_PROCESSOR_TEMP: &str = "1.3.6.1.4.1.14988.1.1.3.11.0";
const OID_MTXR_HL_VOLTAGE: &str = "1.3.6.1.4.1.14988.1.1.3.8.0";
const OID_MTXR_HL_CURRENT: &str = "1.3.6.1.4.1.14988.1.1.3.9.0";
const OID_MTXR_HL_POWER: &str = "1.3.6.1.4.1.14988.1.1.3.12.0";
const OID_MTXR_HL_FAN1: &str = "1.3.6.1.4.1.14988.1.1.3.17.0";
const OID_MTXR_HL_FAN2: &str = "1.3.6.1.4.1.14988.1.1.3.18.0";
const OID_MTXR_HL_DISK_TOTAL: &str = "1.3.6.1.4.1.14988.1.1.3.1.0";
const OID_MTXR_HL_DISK_USED: &str = "1.3.6.1.4.1.14988.1.1.3.2.0";

// ==============================================================================
// STRUCTS PARA DATOS ESTRUCTURADOS
// ==============================================================================

#[derive(Debug, Clone)]
struct WanInterface {
    index: String,
    name: String,
    if_name: String,
    alias: Option<String>,
    is_wan: bool,
    admin_status: u8,
    oper_status: u8,
    speed_mbps: u64,
    traffic_in_bytes: u64,
    traffic_out_bytes: u64,
    errors_in: u64,
    errors_out: u64,
    discards_in: u64,
    discards_out: u64,
    packets_in: u64,
    packets_out: u64,
    isp_detected: Option<String>,
}

impl WanInterface {
    fn to_json(&self) -> Value {
        let in_mb = bytes_to_gb(self.traffic_in_bytes as f64) * 1024.0;
        let out_mb = bytes_to_gb(self.traffic_out_bytes as f64) * 1024.0;
        let admin_status = status_to_text(self.admin_status);
        let oper_status = status_to_text(self.oper_status);
        json!({
            "index": self.index,
            "name": self.name,
            "if_name": self.if_name,
            "alias": self.alias,
            "is_wan": self.is_wan,
            "admin_status": admin_status,
            "oper_status": oper_status,
            "speed_mbps": self.speed_mbps,
            "traffic_in_mb": round2(in_mb),
            "traffic_out_mb": round2(out_mb),
            "traffic_in_bytes": self.traffic_in_bytes,
            "traffic_out_bytes": self.traffic_out_bytes,
            "errors_in": self.errors_in,
            "errors_out": self.errors_out,
            "discards_in": self.discards_in,
            "discards_out": self.discards_out,
            "packets_in": self.packets_in,
            "packets_out": self.packets_out,
            "isp_detected": self.isp_detected,
        })
    }
}

#[derive(Debug, Clone)]
struct NetwatchProbe {
    index: String,
    name: String,
    target_ip: String,
    interval_ms: u64,
    timeout_ms: u64,
    status: u8,
    status_text: String,
    isp_detected: Option<String>,
}

impl NetwatchProbe {
    fn to_json(&self) -> Value {
        json!({
            "index": self.index,
            "name": self.name,
            "target_ip": self.target_ip,
            "interval_ms": self.interval_ms,
            "timeout_ms": self.timeout_ms,
            "status": self.status,
            "status_text": self.status_text,
            "isp_detected": self.isp_detected,
        })
    }
}

#[derive(Debug, Clone)]
struct QueueEntry {
    index: String,
    name: String,
    src_addr: String,
    dst_addr: String,
    interface: String,
    tx_bytes: u64,
    rx_bytes: u64,
    tx_packets: u64,
    rx_packets: u64,
    tx_drop: u64,
    rx_drop: u64,
    isp_detected: Option<String>,
}

impl QueueEntry {
    fn tx_gb(&self) -> f64 {
        bytes_to_gb(self.tx_bytes as f64)
    }

    fn rx_gb(&self) -> f64 {
        bytes_to_gb(self.rx_bytes as f64)
    }

    fn to_json(&self) -> Value {
        json!({
            "index": self.index,
            "name": self.name,
            "src_addr": self.src_addr,
            "dst_addr": self.dst_addr,
            "interface": self.interface,
            "tx_bytes": self.tx_bytes,
            "rx_bytes": self.rx_bytes,
            "tx_gb": round3(self.tx_gb()),
            "rx_gb": round3(self.rx_gb()),
            "tx_packets": self.tx_packets,
            "rx_packets": self.rx_packets,
            "tx_drop": self.tx_drop,
            "rx_drop": self.rx_drop,
            "isp_detected": self.isp_detected,
        })
    }
}

pub struct MikroTikFwProfile;

impl MikroTikFwProfile {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DeviceProfile for MikroTikFwProfile {
    fn vendor(&self) -> &str { "mikrotik_fw" }
    fn vendor_display_name(&self) -> &str { "MikroTik Firewall (RouterOS)" }
    fn device_type(&self) -> &str { "firewall" }

    fn get_cpu_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("cpu_load".into(), OID_HR_PROCESSOR_LOAD.into());
        m
    }

    fn get_memory_oids(&self) -> HashMap<String, String> {
        // Retorna vacío — memoria se extrae en post_process_performance de hrStorageTable
        HashMap::new()
    }

    fn get_disk_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("hrStorageTable".into(), OID_HR_STORAGE_TABLE.into());
        m.insert("hrStorageDescr".into(), OID_HR_STORAGE_DESCR.into());
        m.insert("hrStorageAllocationUnits".into(), OID_HR_STORAGE_ALLOC_UNITS.into());
        m.insert("hrStorageSize".into(), OID_HR_STORAGE_SIZE.into());
        m.insert("hrStorageUsed".into(), OID_HR_STORAGE_USED.into());
        m
    }

    fn get_vendor_oids(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        // Interfaces
        m.insert("if_number".into(), OID_IF_NUMBER.into());
        m.insert("if_descr".into(), OID_IF_DESCR.into());
        m.insert("if_speed".into(), OID_IF_SPEED.into());
        m.insert("if_admin_status".into(), OID_IF_ADMIN_STATUS.into());
        m.insert("if_oper_status".into(), OID_IF_OPER_STATUS.into());
        m.insert("if_in_errors".into(), OID_IF_IN_ERRORS.into());
        m.insert("if_out_errors".into(), OID_IF_OUT_ERRORS.into());
        m.insert("if_in_discards".into(), OID_IF_IN_DISCARDS.into());
        m.insert("if_out_discards".into(), OID_IF_OUT_DISCARDS.into());
        m.insert("if_name".into(), OID_IF_NAME.into());
        m.insert("if_alias".into(), OID_IF_ALIAS.into());
        m.insert("if_hc_in_octets".into(), OID_IF_HC_IN_OCTETS.into());
        m.insert("if_hc_out_octets".into(), OID_IF_HC_OUT_OCTETS.into());
        m.insert("if_hc_in_ucast".into(), OID_IF_HC_IN_UCAST.into());
        m.insert("if_hc_out_ucast".into(), OID_IF_HC_OUT_UCAST.into());
        m.insert("if_high_speed".into(), OID_IF_HIGH_SPEED.into());
        // Netwatch
        m.insert("netwatch_name".into(), OID_NETWATCH_NAME.into());
        m.insert("netwatch_ip".into(), OID_NETWATCH_IP.into());
        m.insert("netwatch_interval".into(), OID_NETWATCH_INTERVAL.into());
        m.insert("netwatch_timeout".into(), OID_NETWATCH_TIMEOUT.into());
        m.insert("netwatch_status".into(), OID_NETWATCH_STATUS.into());
        // Queue
        m.insert("queue_name".into(), OID_QUEUE_NAME.into());
        m.insert("queue_src_addr".into(), OID_QUEUE_SRC_ADDR.into());
        m.insert("queue_dst_addr".into(), OID_QUEUE_DST_ADDR.into());
        m.insert("queue_interface".into(), OID_QUEUE_INTERFACE.into());
        m.insert("queue_tx_bytes".into(), OID_QUEUE_TX_BYTES.into());
        m.insert("queue_tx_packets".into(), OID_QUEUE_TX_PACKETS.into());
        m.insert("queue_rx_bytes".into(), OID_QUEUE_RX_BYTES.into());
        m.insert("queue_rx_packets".into(), OID_QUEUE_RX_PACKETS.into());
        m.insert("queue_tx_drop".into(), OID_QUEUE_TX_DROP.into());
        m.insert("queue_rx_drop".into(), OID_QUEUE_RX_DROP.into());
        // MikroTik Health
        m.insert("mtxr_firmware_version".into(), OID_MTXR_FIRMWARE_VERSION.into());
        m.insert("mtxr_license_id".into(), OID_MTXR_LICENSE_ID.into());
        m.insert("mtxr_serial_number".into(), OID_MTXR_SERIAL_NUMBER.into());
        m.insert("mtxr_firmware_upgrade".into(), OID_MTXR_FIRMWARE_UPGRADE.into());
        m.insert("mtxr_board_name".into(), OID_MTXR_BOARD_NAME.into());
        m.insert("mtxr_hl_temperature".into(), OID_MTXR_HL_TEMPERATURE.into());
        m.insert("mtxr_hl_processor_temp".into(), OID_MTXR_HL_PROCESSOR_TEMP.into());
        m.insert("mtxr_hl_voltage".into(), OID_MTXR_HL_VOLTAGE.into());
        m.insert("mtxr_hl_current".into(), OID_MTXR_HL_CURRENT.into());
        m.insert("mtxr_hl_power".into(), OID_MTXR_HL_POWER.into());
        m.insert("mtxr_hl_fan1".into(), OID_MTXR_HL_FAN1.into());
        m.insert("mtxr_hl_fan2".into(), OID_MTXR_HL_FAN2.into());
        m.insert("mtxr_hl_disk_total".into(), OID_MTXR_HL_DISK_TOTAL.into());
        m.insert("mtxr_hl_disk_used".into(), OID_MTXR_HL_DISK_USED.into());
        m
    }

    fn normalize_cpu_data(&self, raw: &HashMap<String, SnmpValue>) -> Value {
        let mut cores = Vec::new();
        let mut total = 0.0;
        let mut count = 0u64;
        for (key, val) in raw {
            if let Some(usage) = val.as_f64() {
                cores.push(json!({ "core": key, "usage_percent": round2(usage) }));
                total += usage;
                count += 1;
            }
        }
        let avg = if count > 0 { total / count as f64 } else { 0.0 };
        json!({
            "cpu_usage_percent": round2(avg),
            "cpu_cores": cores,
            "cpu_core_count": count,
        })
    }

    fn normalize_memory_data(&self, _raw: &HashMap<String, SnmpValue>) -> Value {
        // Retorna mínimo — se extrae en post_process_performance desde hrStorageTable "main memory"
        json!({
            "total_gb": 0.0,
            "used_gb": 0.0,
            "free_gb": 0.0,
            "usage_percent": 0.0,
        })
    }

    fn normalize_disk_data(
        &self,
        raw: &HashMap<String, HashMap<String, SnmpValue>>,
    ) -> Value {
        let mut disks = Vec::new();
        for (idx, entry) in raw {
            let descr = entry.get("hrStorageDescr")
                .map(|v| v.as_string())
                .unwrap_or_else(|| format!("storage-{}", idx));
            
            let descr_lower = descr.to_lowercase();
            let skip_keywords = ["real memory", "virtual memory", "swap", "memory buffers", "ram"];
            if skip_keywords.iter().any(|kw| descr_lower.contains(kw)) {
                continue;
            }

            let units = entry.get("hrStorageAllocationUnits")
                .and_then(|v| v.as_i64()).unwrap_or(512) as f64;
            let size = entry.get("hrStorageSize")
                .and_then(|v| v.as_i64()).unwrap_or(0) as f64;
            let used = entry.get("hrStorageUsed")
                .and_then(|v| v.as_i64()).unwrap_or(0) as f64;

            let total_bytes = size * units;
            let used_bytes = used * units;
            let free_bytes = (total_bytes - used_bytes).max(0.0);
            
            if total_bytes > 0.0 {
                disks.push(json!({
                    "mount": clean_text(&descr),
                    "total_gb": round3(bytes_to_gb(total_bytes)),
                    "used_gb": round3(bytes_to_gb(used_bytes)),
                    "free_gb": round3(bytes_to_gb(free_bytes)),
                    "usage_percent": round2(calculate_percentage(used_bytes, total_bytes)),
                }));
            }
        }
        json!(disks)
    }

    fn post_process_performance(&self, mut data: Value) -> Value {
        // Si memory.total_gb == 0.0, buscar en disks[] una entrada con "main memory"
        let mem_total = data.get("memory")
            .and_then(|m| m.get("total_gb"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if mem_total == 0.0 {
            let disk_key = if data.get("disks").is_some() { "disks" } else { "disk" };
            let mut found_idx = None;
            if let Some(disks) = data.get(disk_key).and_then(|d| d.as_array()) {
                for (idx, disk) in disks.iter().enumerate() {
                    let mount = disk.get("mount").and_then(|m| m.as_str()).unwrap_or("");
                    if mount.to_lowercase().contains("main memory") {
                        found_idx = Some(idx);
                        break;
                    }
                }
            }

            if let Some(idx) = found_idx {
                if let Some(disk) = data.get(disk_key).and_then(|d| d.as_array()).and_then(|a| a.get(idx)) {
                    let total = disk.get("total_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let used = disk.get("used_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let usage = disk.get("usage_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if let Some(mem) = data.get_mut("memory") {
                        *mem = json!({
                            "total_gb": total,
                            "used_gb": used,
                            "free_gb": (total - used).max(0.0),
                            "usage_percent": usage,
                        });
                    }
                }
                // Remover del array de disks
                if let Some(disk_array) = data.get_mut(disk_key).and_then(|d| d.as_array_mut()) {
                    disk_array.remove(idx);
                }
            }
        }
        data
    }

    fn finalize_collected_data(&self, mut data: Value) -> Value {
        let vendor_key = format!("{}_specific", self.vendor());
        let avg_cpu = data
            .get(&vendor_key)
            .and_then(|v| v.get("cpu_detailed"))
            .and_then(|v| v.get("average_percent"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if avg_cpu > 0.0 {
            if let Some(cpu_obj) = data.get_mut("performance")
                .and_then(|p| p.get_mut("cpu"))
                .and_then(|c| c.as_object_mut()) {
                let current = cpu_obj
                    .get("cpu_usage_percent")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                if current == 0.0 {
                    cpu_obj.insert("cpu_usage_percent".into(), json!(round2(avg_cpu)));
                }
            }
        }

        data
    }

    async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> Value {
        let mut data = Map::new();
        let now = chrono::Utc::now().to_rfc3339();

        // Collect system info
        let system_info = collect_system_info(client).await;
        data.insert("system_info".into(), system_info);

        // Collect health data
        let health = collect_health(client).await;
        data.insert("health".into(), health);

        // Collect CPU detailed
        let cpu_detailed = collect_cpu_detailed(client).await;
        data.insert("cpu_detailed".into(), cpu_detailed);

        // Disk fallback (from MTXR)
        let disk_fallback = collect_disk_fallback(client).await;
        data.insert("disk_fallback".into(), disk_fallback);

        // Get interface count
        let if_count = snmp_get_i64(client, OID_IF_NUMBER).await.unwrap_or(0);
        data.insert("interfaces_total".into(), json!(if_count));

        // Collect WAN interfaces
        let wan_interfaces = collect_wan_interfaces(client).await;
        data.insert("wan_interfaces".into(), json!(wan_interfaces.iter().map(|w| w.to_json()).collect::<Vec<_>>()));

        // Collect Netwatch probes
        let (netwatch_probes, netwatch_available) = collect_netwatch(client).await;
        let netwatch_payload = build_netwatch_payload(&netwatch_probes, netwatch_available);
        data.insert("netwatch".into(), netwatch_payload);

        // Collect Queues
        let (queue_entries, queues_available) = collect_queues(client).await;
        let queues_payload = build_queues_payload(&queue_entries, queues_available);
        data.insert("queues".into(), queues_payload);

        // Build Internet Channels
        let internet_channels = build_internet_channels_summary(
            &wan_interfaces,
            &netwatch_probes,
            netwatch_available,
            &queue_entries,
            queues_available,
        );
        data.insert("internet_channels".into(), internet_channels);

        data.insert("collection_timestamp".into(), json!(now));

        json!(data)
    }

    fn matches_sys_object_id(&self, sys_oid: &str) -> bool {
        sys_oid.starts_with("1.3.6.1.4.1.14988")
    }
}

// ==============================================================================
// ASYNC HELPER FUNCTIONS
// ==============================================================================

async fn collect_system_info(client: &SnmpClient) -> Value {
    let mut info = Map::new();
    
    if let Some(fw_ver) = snmp_get_clean_string(client, OID_MTXR_FIRMWARE_VERSION).await {
        info.insert("mtxr_firmware_version".into(), json!(fw_ver));
    }
    if let Some(license) = snmp_get_clean_string(client, OID_MTXR_LICENSE_ID).await {
        info.insert("mtxr_license_id".into(), json!(license));
    }
    if let Some(serial) = snmp_get_clean_string(client, OID_MTXR_SERIAL_NUMBER).await {
        info.insert("mtxr_serial_number".into(), json!(serial));
    }
    if let Some(upgrade) = snmp_get_clean_string(client, OID_MTXR_FIRMWARE_UPGRADE).await {
        info.insert("mtxr_firmware_upgrade_ver".into(), json!(upgrade));
    }
    if let Some(board) = snmp_get_clean_string(client, OID_MTXR_BOARD_NAME).await {
        info.insert("mtxr_board_name".into(), json!(board));
    }

    json!(info)
}

async fn collect_health(client: &SnmpClient) -> Value {
    let mut health = Map::new();

    if let Some(temp) = snmp_get_i64(client, OID_MTXR_HL_TEMPERATURE).await {
        health.insert("temperature_celsius".into(), json!(round1(temp as f64 / 10.0)));
    }
    if let Some(proc_temp) = snmp_get_i64(client, OID_MTXR_HL_PROCESSOR_TEMP).await {
        health.insert("processor_temp_celsius".into(), json!(round1(proc_temp as f64 / 10.0)));
    }
    if let Some(voltage) = snmp_get_i64(client, OID_MTXR_HL_VOLTAGE).await {
        health.insert("voltage_volts".into(), json!(round1(voltage as f64 / 10.0)));
    }
    if let Some(current) = snmp_get_i64(client, OID_MTXR_HL_CURRENT).await {
        health.insert("current_ma".into(), json!(current));
    }
    if let Some(power) = snmp_get_i64(client, OID_MTXR_HL_POWER).await {
        health.insert("power_watts".into(), json!(round1(power as f64 / 10.0)));
    }
    if let Some(fan1) = snmp_get_i64(client, OID_MTXR_HL_FAN1).await {
        health.insert("fan1_rpm".into(), json!(fan1));
    }
    if let Some(fan2) = snmp_get_i64(client, OID_MTXR_HL_FAN2).await {
        health.insert("fan2_rpm".into(), json!(fan2));
    }

    json!(health)
}

async fn collect_cpu_detailed(client: &SnmpClient) -> Value {
    let (cpu_results, _) = client.bulk(OID_HR_PROCESSOR_LOAD, 32).await;
    
    let mut cores = Vec::new();
    let mut total = 0.0;
    
    for (oid, val) in cpu_results {
        if let Some(usage) = val.as_f64() {
            let idx = oid_index(&oid);
            cores.push(json!({
                "index": idx,
                "load_percent": round2(usage),
            }));
            total += usage;
        }
    }

    let avg = if !cores.is_empty() {
        total / cores.len() as f64
    } else {
        0.0
    };

    json!({
        "cores": cores,
        "core_count": cores.len(),
        "average_percent": round2(avg),
    })
}

async fn collect_disk_fallback(client: &SnmpClient) -> Value {
    let mut fallback = Map::new();

    if let Some(total) = snmp_get_u64(client, OID_MTXR_HL_DISK_TOTAL).await {
        fallback.insert("mtxr_hl_disk_total".into(), json!(total));
        fallback.insert("total_gb".into(), json!(round2(bytes_to_gb(total as f64))));
    }
    if let Some(used) = snmp_get_u64(client, OID_MTXR_HL_DISK_USED).await {
        fallback.insert("mtxr_hl_disk_used".into(), json!(used));
        fallback.insert("used_gb".into(), json!(round2(bytes_to_gb(used as f64))));
        if let Some(total) = snmp_get_u64(client, OID_MTXR_HL_DISK_TOTAL).await {
            let free = (total as i128 - used as i128).max(0) as f64;
            fallback.insert("free_gb".into(), json!(round2(bytes_to_gb(free))));
            fallback.insert("percent_used".into(), json!(round2(calculate_percentage(used as f64, total as f64))));
        }
    }

    json!(fallback)
}

async fn collect_wan_interfaces(client: &SnmpClient) -> Vec<WanInterface> {
    let (if_descrs, _) = client.bulk(OID_IF_DESCR, 50).await;
    let (if_names, _) = client.bulk(OID_IF_NAME, 50).await;
    let (if_aliases, _) = client.bulk(OID_IF_ALIAS, 50).await;
    let (if_admin_status, _) = client.bulk(OID_IF_ADMIN_STATUS, 50).await;
    let (if_oper_status, _) = client.bulk(OID_IF_OPER_STATUS, 50).await;
    let (if_speeds, _) = client.bulk(OID_IF_HIGH_SPEED, 50).await;
    let (if_in_octets, _) = client.bulk(OID_IF_HC_IN_OCTETS, 50).await;
    let (if_out_octets, _) = client.bulk(OID_IF_HC_OUT_OCTETS, 50).await;
    let (if_in_errors, _) = client.bulk(OID_IF_IN_ERRORS, 50).await;
    let (if_out_errors, _) = client.bulk(OID_IF_OUT_ERRORS, 50).await;
    let (if_in_discards, _) = client.bulk(OID_IF_IN_DISCARDS, 50).await;
    let (if_out_discards, _) = client.bulk(OID_IF_OUT_DISCARDS, 50).await;
    let (if_in_ucast, _) = client.bulk(OID_IF_HC_IN_UCAST, 50).await;
    let (if_out_ucast, _) = client.bulk(OID_IF_HC_OUT_UCAST, 50).await;

    // Build maps
    let mut results = Vec::new();
    let base_list = if !if_descrs.is_empty() { &if_descrs } else { &if_names };
    for (oid, name_val) in base_list {
        let idx = oid_index(&oid);
        let name = clean_text(&name_val.as_string());

        let if_name = if_names.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .map(|(_, v)| clean_text(&v.as_string()))
            .unwrap_or_else(|| name.clone());

        let alias = if_aliases.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .map(|(_, v)| clean_text(&v.as_string()))
            .filter(|s| !s.is_empty());

        let mut is_wan = is_wan_candidate(&name) || is_wan_candidate(&if_name);
        if let Some(ref a) = alias {
            if is_wan_candidate(a) {
                is_wan = true;
            }
        }

        if !is_wan {
            continue;
        }

        let admin_status = if_admin_status.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_i64())
            .unwrap_or(0) as u8;

        let oper_status = if_oper_status.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_i64())
            .unwrap_or(0) as u8;

        let speed_mbps = if_speeds.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let traffic_in = if_in_octets.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let traffic_out = if_out_octets.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let errors_in = if_in_errors.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let errors_out = if_out_errors.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let discards_in = if_in_discards.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let discards_out = if_out_discards.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let packets_in = if_in_ucast.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let packets_out = if_out_ucast.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let mut isp = detect_isp_from_name(&name);
        if isp.is_none() {
            if let Some(ref a) = alias {
                isp = detect_isp_from_name(a);
            }
        }
        if isp.is_none() {
            isp = detect_isp_from_name(&if_name);
        }

        results.push(WanInterface {
            index: idx,
            name: name.clone(),
            if_name,
            alias,
            is_wan: true,
            admin_status,
            oper_status,
            speed_mbps,
            traffic_in_bytes: traffic_in,
            traffic_out_bytes: traffic_out,
            errors_in,
            errors_out,
            discards_in,
            discards_out,
            packets_in,
            packets_out,
            isp_detected: isp,
        });
    }

    results
}

async fn collect_netwatch(client: &SnmpClient) -> (Vec<NetwatchProbe>, bool) {
    let (nw_names, _) = client.bulk(OID_NETWATCH_NAME, 30).await;
    
    if nw_names.is_empty() {
        return (Vec::new(), false);
    }

    let (nw_ips, _) = client.bulk(OID_NETWATCH_IP, 30).await;
    let (nw_intervals, _) = client.bulk(OID_NETWATCH_INTERVAL, 30).await;
    let (nw_timeouts, _) = client.bulk(OID_NETWATCH_TIMEOUT, 30).await;
    let (nw_status, _) = client.bulk(OID_NETWATCH_STATUS, 30).await;

    let mut results = Vec::new();

    for (oid, name_val) in nw_names {
        let idx = oid_index(&oid);
        let name = name_val.as_string();

        let target_ip = nw_ips.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .map(|(_, v)| snmp_addr_to_string(v))
            .unwrap_or_default();

        let interval = nw_intervals.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let timeout = nw_timeouts.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let status = nw_status.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_i64())
            .unwrap_or(0) as u8;

        let status_text = match status {
            1 => "up".to_string(),
            2 => "down".to_string(),
            _ => "unknown".to_string(),
        };
        let isp = detect_isp_from_name(&name);

        results.push(NetwatchProbe {
            index: idx,
            name,
            target_ip,
            interval_ms: interval,
            timeout_ms: timeout,
            status,
            status_text,
            isp_detected: isp,
        });
    }

    (results, true)
}

fn build_netwatch_payload(probes: &[NetwatchProbe], available: bool) -> Value {
    let total = probes.len();
    let up = probes.iter().filter(|p| p.status == 1).count();
    let down = total - up;
    let availability = if total > 0 {
        Some(round2((up as f64 / total as f64) * 100.0))
    } else {
        None
    };

    json!({
        "probes": probes.iter().map(|p| p.to_json()).collect::<Vec<_>>(),
        "summary": {
            "total": total,
            "up": up,
            "down": down,
            "availability_percent": availability,
        },
        "available": available,
    })
}

async fn collect_queues(client: &SnmpClient) -> (Vec<QueueEntry>, bool) {
    let (q_names, _) = client.bulk(OID_QUEUE_NAME, 50).await;
    
    if q_names.is_empty() {
        return (Vec::new(), false);
    }

    let (q_src_addrs, _) = client.bulk(OID_QUEUE_SRC_ADDR, 50).await;
    let (q_dst_addrs, _) = client.bulk(OID_QUEUE_DST_ADDR, 50).await;
    let (q_interfaces, _) = client.bulk(OID_QUEUE_INTERFACE, 50).await;
    let (q_tx_bytes, _) = client.bulk(OID_QUEUE_TX_BYTES, 50).await;
    let (q_tx_packets, _) = client.bulk(OID_QUEUE_TX_PACKETS, 50).await;
    let (q_rx_bytes, _) = client.bulk(OID_QUEUE_RX_BYTES, 50).await;
    let (q_rx_packets, _) = client.bulk(OID_QUEUE_RX_PACKETS, 50).await;
    let (q_tx_drops, _) = client.bulk(OID_QUEUE_TX_DROP, 50).await;
    let (q_rx_drops, _) = client.bulk(OID_QUEUE_RX_DROP, 50).await;

    let mut results = Vec::new();

    for (oid, name_val) in q_names {
        let idx = oid_index(&oid);
        let name = name_val.as_string();

        let src_addr = q_src_addrs.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .map(|(_, v)| snmp_addr_to_string(v))
            .unwrap_or_default();

        let dst_addr = q_dst_addrs.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .map(|(_, v)| snmp_addr_to_string(v))
            .unwrap_or_default();

        let interface = q_interfaces.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .map(|(_, v)| clean_text(&v.as_string()))
            .unwrap_or_default();

        let tx_bytes = q_tx_bytes.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let tx_packets = q_tx_packets.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let rx_bytes = q_rx_bytes.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let rx_packets = q_rx_packets.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let tx_drop = q_tx_drops.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let rx_drop = q_rx_drops.iter()
            .find(|(o, _)| oid_index(o) == idx)
            .and_then(|(_, v)| v.as_u64())
            .unwrap_or(0);

        let isp = detect_isp_from_name(&name);

        results.push(QueueEntry {
            index: idx,
            name,
            src_addr,
            dst_addr,
            interface,
            tx_bytes,
            rx_bytes,
            tx_packets,
            rx_packets,
            tx_drop,
            rx_drop,
            isp_detected: isp,
        });
    }

    (results, true)
}

fn build_queues_payload(entries: &[QueueEntry], available: bool) -> Value {
    let total = entries.len();
    let total_tx_gb: f64 = entries.iter().map(|e| e.tx_gb()).sum();
    let total_rx_gb: f64 = entries.iter().map(|e| e.rx_gb()).sum();
    let total_tx_drops: u64 = entries.iter().map(|e| e.tx_drop).sum();
    let total_rx_drops: u64 = entries.iter().map(|e| e.rx_drop).sum();

    json!({
        "entries": entries.iter().map(|e| e.to_json()).collect::<Vec<_>>(),
        "summary": {
            "total_queues": total,
            "total_tx_gb": round3(total_tx_gb),
            "total_rx_gb": round3(total_rx_gb),
            "total_tx_drops": total_tx_drops,
            "total_rx_drops": total_rx_drops,
        },
        "available": available,
    })
}

fn build_internet_channels_summary(
    wan_interfaces: &[WanInterface],
    netwatch_probes: &[NetwatchProbe],
    netwatch_available: bool,
    queue_entries: &[QueueEntry],
    queues_available: bool,
) -> Value {
    let mut channels: Vec<Value> = Vec::new();
    let mut seen_isps: HashMap<String, usize> = HashMap::new();

    // 1) Canales basados en interfaces WAN
    for iface in wan_interfaces {
        let channel_name = iface.alias.clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| if !iface.if_name.is_empty() { iface.if_name.clone() } else { iface.name.clone() });
        let isp = iface.isp_detected.clone().unwrap_or_else(|| "Desconocido".to_string());
        let oper_status = status_to_text(iface.oper_status).to_string();
        let is_up = oper_status.eq_ignore_ascii_case("UP");

        let traffic_in_mb = round2(bytes_to_gb(iface.traffic_in_bytes as f64) * 1024.0);
        let traffic_out_mb = round2(bytes_to_gb(iface.traffic_out_bytes as f64) * 1024.0);

        let mut channel = json!({
            "channel_name": channel_name,
            "isp": isp,
            "source": "wan_interface",
            "oper_status": oper_status,
            "is_up": is_up,
            "speed_mbps": iface.speed_mbps,
            "traffic_in_mb": traffic_in_mb,
            "traffic_out_mb": traffic_out_mb,
            "errors_in": iface.errors_in,
            "errors_out": iface.errors_out,
            "discards_in": iface.discards_in,
            "discards_out": iface.discards_out,
            "netwatch_status": Value::Null,
            "alerts": [],
        });

        if let Some(obj) = channel.as_object_mut() {
            let alerts = check_channel_alerts(obj);
            obj.insert("alerts".into(), json!(alerts));
        }

        if let Some(ref isp_name) = iface.isp_detected {
            if !seen_isps.contains_key(isp_name) {
                seen_isps.insert(isp_name.clone(), channels.len());
            }
        }
        channels.push(channel);
    }

    // 2) Enriquecer con Netwatch (por ISP detectado en el nombre del probe)
    if netwatch_available {
        for probe in netwatch_probes {
            let probe_isp = probe.isp_detected.clone();
            let probe_name = probe.name.clone();
            let probe_status = probe.status_text.clone();

            if let Some(isp_name) = probe_isp.clone() {
                if let Some(idx) = seen_isps.get(&isp_name).copied() {
                    if let Some(obj) = channels.get_mut(idx).and_then(|v| v.as_object_mut()) {
                        obj.insert("netwatch_status".into(), json!(probe_status.clone()));
                        obj.insert("netwatch_probe".into(), json!(probe.target_ip.clone()));
                        obj.insert("netwatch_probe_name".into(), json!(probe_name.clone()));
                    }
                    continue;
                }
            }

            let channel = json!({
                "channel_name": probe_name,
                "isp": probe_isp.clone().unwrap_or_else(|| "Desconocido".to_string()),
                "source": "netwatch",
                "oper_status": "unknown",
                "is_up": probe_status == "up",
                "speed_mbps": 0,
                "traffic_in_mb": 0.0,
                "traffic_out_mb": 0.0,
                "errors_in": 0,
                "errors_out": 0,
                "discards_in": 0,
                "discards_out": 0,
                "netwatch_status": probe_status,
                "netwatch_probe": probe.target_ip.clone(),
                "netwatch_probe_name": probe.name.clone(),
                "alerts": [],
            });

            if let Some(ref isp_name) = probe_isp {
                if !seen_isps.contains_key(isp_name) {
                    seen_isps.insert(isp_name.clone(), channels.len());
                }
            }
            channels.push(channel);
        }
    }

    // 3) Enriquecer con Queue (tráfico por ISP)
    if queues_available {
        for entry in queue_entries {
            if let Some(ref isp_name) = entry.isp_detected {
                if let Some(idx) = seen_isps.get(isp_name).copied() {
                    if let Some(obj) = channels.get_mut(idx).and_then(|v| v.as_object_mut()) {
                        obj.insert("queue_tx_gb".into(), json!(round4(entry.tx_gb())));
                        obj.insert("queue_rx_gb".into(), json!(round4(entry.rx_gb())));
                        obj.insert("queue_tx_drops".into(), json!(entry.tx_drop));
                        obj.insert("queue_rx_drops".into(), json!(entry.rx_drop));

                        if entry.tx_drop > 0 || entry.rx_drop > 0 {
                            let total_drops = entry.tx_drop + entry.rx_drop;
                            let mut alerts = obj.get("alerts")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            alerts.push(Value::String(format!(
                                "Queue drops detectados en canal {}: {} drops",
                                isp_name, total_drops
                            )));
                            obj.insert("alerts".into(), Value::Array(alerts));
                        }
                    }
                }
            }
        }
    }

    // 4) Summary global
    let up_count = channels.iter()
        .filter(|c| c.get("is_up").and_then(|v| v.as_bool()).unwrap_or(false))
        .count();
    let down_count = channels.iter()
        .filter(|c| {
            !c.get("is_up").and_then(|v| v.as_bool()).unwrap_or(false)
                && c.get("oper_status")
                    .and_then(|v| v.as_str())
                    .map(|s| s != "unknown")
                    .unwrap_or(false)
        })
        .count();
    let total_in_mb: f64 = channels.iter()
        .map(|c| c.get("traffic_in_mb").and_then(|v| v.as_f64()).unwrap_or(0.0))
        .sum();
    let total_out_mb: f64 = channels.iter()
        .map(|c| c.get("traffic_out_mb").and_then(|v| v.as_f64()).unwrap_or(0.0))
        .sum();

    json!({
        "channels": channels,
        "summary": {
            "total_channels": channels.len(),
            "channels_up": up_count,
            "channels_down": down_count,
            "total_traffic_in_mb": round2(total_in_mb),
            "total_traffic_out_mb": round2(total_out_mb),
            "netwatch_available": netwatch_available,
            "queues_available": queues_available,
        },
    })
}

// ==============================================================================
// HELPER FUNCTIONS
// ==============================================================================

fn status_to_text(status: u8) -> &'static str {
    match status {
        1 => "UP",
        2 => "DOWN",
        _ => "unknown",
    }
}

fn check_channel_alerts(channel: &Map<String, Value>) -> Vec<String> {
    let mut alerts = Vec::new();
    let channel_name = channel
        .get("channel_name")
        .and_then(|v| v.as_str())
        .unwrap_or("desconocido");
    let oper_status = channel
        .get("oper_status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    if oper_status.eq_ignore_ascii_case("DOWN") {
        alerts.push(format!("Canal WAN DOWN: {}", channel_name));
    }

    let errors_in = channel.get("errors_in").and_then(|v| v.as_u64()).unwrap_or(0);
    let errors_out = channel.get("errors_out").and_then(|v| v.as_u64()).unwrap_or(0);
    if errors_in + errors_out > 100 {
        alerts.push(format!(
            "Alto numero de errores en interfaz WAN: IN={} OUT={}",
            errors_in, errors_out
        ));
    }

    let discards_in = channel.get("discards_in").and_then(|v| v.as_u64()).unwrap_or(0);
    let discards_out = channel.get("discards_out").and_then(|v| v.as_u64()).unwrap_or(0);
    if discards_in + discards_out > 500 {
        alerts.push(format!(
            "Descartes elevados en interfaz WAN: {} paquetes",
            discards_in + discards_out
        ));
    }

    alerts
}

fn is_wan_candidate(name: &str) -> bool {
    let lower = name.to_lowercase();
    WAN_INTERFACE_PATTERNS.iter().any(|p| lower.contains(p))
}

fn detect_isp_from_name(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    
    let isp_patterns = [
        ("etb", "ETB"),
        ("tigo", "Tigo"),
        ("claro", "Claro"),
        ("movistar", "Movistar"),
        ("azteca", "Azteca"),
        ("internexa", "InterNexa"),
        ("edatel", "Edatel"),
        ("coltel", "Coltel"),
        ("directv", "DirecTV"),
        ("starlink", "Starlink"),
        ("at&t", "AT&T"),
        ("att", "AT&T"),
        ("une", "UNE"),
        ("millicom", "Millicom"),
        ("telefonica", "Telefónica"),
        ("supercanal", "Supercanal"),
    ];

    for (pattern, name) in &isp_patterns {
        if lower.contains(pattern) {
            return Some(name.to_string());
        }
    }

    None
}

fn oid_index(oid: &str) -> String {
    oid.rsplit('.').next().unwrap_or("0").to_string()
}

fn clean_text(s: &str) -> String {
    s.replace('\0', "").trim().to_string()
}

fn snmp_addr_to_string(value: &SnmpValue) -> String {
    match value {
        SnmpValue::IpAddress(ip) => ip.clone(),
        SnmpValue::OctetString(s) => clean_text(s),
        SnmpValue::OctetStringRaw(_) => "unknown".to_string(),
        _ => clean_text(&value.as_string()),
    }
}

async fn snmp_get_clean_string(client: &SnmpClient, oid: &str) -> Option<String> {
    let result = client.get(oid).await;
    result.value.as_ref().map(|v| clean_text(&v.as_string()))
}

async fn snmp_get_i64(client: &SnmpClient, oid: &str) -> Option<i64> {
    let result = client.get(oid).await;
    result.value.as_ref().and_then(|v| v.as_i64())
}

async fn snmp_get_u64(client: &SnmpClient, oid: &str) -> Option<u64> {
    let result = client.get(oid).await;
    result.value.as_ref().and_then(|v| v.as_u64())
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}
