// ==============================================================================
// NESS Relay v2.0.0 — Transformación de payload para compatibilidad con servidor
// ==============================================================================
//
// El servidor NESS (Django) fue diseñado para recibir datos del agente Python.
// Este módulo transforma el payload del formato interno Rust al formato que
// espera el serializer del servidor, asegurando compatibilidad total.
//
// Se invoca DESPUÉS del análisis de alertas y ANTES de exportar/enviar,
// para que los analizadores internos sigan trabajando con el formato Rust.
// ==============================================================================

use serde_json::{json, Map, Value};
use crate::utils::{geoip, helpers};

/// Transforma el payload del formato interno Rust al formato compatible
/// con el serializer del servidor NESS (formato Python).
pub fn transform_for_server(mut payload: Value) -> Value {
    transform_system(&mut payload);
    transform_network(&mut payload);
    transform_performance(&mut payload);
    transform_security(&mut payload);
    transform_vendor_specific(&mut payload);
    transform_geolocation(&mut payload);
    payload
}

// ---------------------------------------------------------------------------
// system: envolver en basic_info, renombrar uptime → sys_uptime
// ---------------------------------------------------------------------------
fn transform_system(payload: &mut Value) {
    let system = match payload.get("system").cloned() {
        Some(s) => s,
        None => return,
    };

    let mut basic_info = Map::new();

    for key in &["sys_name", "sys_descr", "sys_location", "sys_contact"] {
        if let Some(v) = system.get(*key) {
            basic_info.insert((*key).to_string(), v.clone());
        }
    }

    // Renombrar "uptime" → "sys_uptime" y añadir campo "formatted"
    if let Some(mut uptime) = system.get("uptime").cloned() {
        if let Some(human) = uptime.get("human").cloned() {
            if let Some(obj) = uptime.as_object_mut() {
                obj.insert("formatted".into(), human);
            }
        }
        basic_info.insert("sys_uptime".into(), uptime);
    }

    payload["system"] = json!({ "basic_info": basic_info });
}

// ---------------------------------------------------------------------------
// network.interfaces: array → dict, renombrar campos, convertir unidades
// ---------------------------------------------------------------------------
fn transform_network(payload: &mut Value) {
    // 1. Capturar el proveedor desde metadata (lo usaremos para isp_detected)
    let provider_global = payload
        .get("metadata")
        .and_then(|m| m.get("provider"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // 2. Intentar obtener el acceso al objeto "network"
    let network = match payload.get_mut("network") {
        Some(n) => n,
        None => return,
    };

    // 3. Extraer el array de interfaces (lo clonamos para poder iterar tranquilos)
    let interfaces_array = match network.get("interfaces").and_then(|v| v.as_array()) {
        Some(arr) => arr.clone(),
        None => return,
    };

    let mut interfaces_dict = Map::new();

    // 4. Procesar cada interfaz del array
    for iface in &interfaces_array {
        // EXTRAEMOS LOS VALORES PRIMERO (Esto evita tus 18 errores)
        let index = iface.get("index").and_then(|v| v.as_str()).unwrap_or("0").to_string();
        let name = iface.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        
        let admin_status = iface.get("admin_status")
            .and_then(|v| v.as_str()).unwrap_or("down").to_uppercase();
        
        let oper_status = iface.get("operational_status")
            .and_then(|v| v.as_str()).unwrap_or("down").to_uppercase();

        let speed_mbps = iface.get("speed_mbps").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let traffic_in_mb = iface.get("traffic_in_mb").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let traffic_out_mb = iface.get("traffic_out_mb").and_then(|v| v.as_f64()).unwrap_or(0.0);
        
        let errors_in = iface.get("errors_in").and_then(|v| v.as_u64()).unwrap_or(0);
        let errors_out = iface.get("errors_out").and_then(|v| v.as_u64()).unwrap_or(0);
        let discards_in = iface.get("discards_in").and_then(|v| v.as_u64()).unwrap_or(0);
        let discards_out = iface.get("discards_out").and_then(|v| v.as_u64()).unwrap_or(0);

        // 5. INSERTAR en el nuevo diccionario
        interfaces_dict.insert(
            index.clone(),
            json!({
                "index": index,
                "name": name,
                "admin_status": admin_status,
                "operational_status": oper_status,
                "speed_mbps": speed_mbps,
                "traffic_in_mb": traffic_in_mb,
                "traffic_out_mb": traffic_out_mb,
                "errors_in": errors_in,
                "errors_out": errors_out,
                "total_errors": errors_in + errors_out,
                "discards_in": discards_in,
                "discards_out": discards_out,
                "isp_detected": provider_global, // Aquí usamos el dato de Bosa/Bogotá
            }),
        );
    }

    // 6. Reemplazar el array de interfaces original por nuestro nuevo diccionario (formato Python)
    network["interfaces"] = Value::Object(interfaces_dict);
}

// ---------------------------------------------------------------------------
// performance: CPU campo renaming, memoria GB→MB, disco array→dict
// ---------------------------------------------------------------------------
fn transform_performance(payload: &mut Value) {
    let perf = match payload.get_mut("performance") {
        Some(p) => p,
        None => return,
    };

    // --- CPU: renombrar load_avg_X → load_Xmin (string → float) ---
    if let Some(cpu) = perf.get_mut("cpu").and_then(|c| c.as_object_mut()) {
        let load_1 = cpu
            .remove("load_avg_1")
            .and_then(|v| str_or_f64(&v))
            .unwrap_or(0.0);
        let load_5 = cpu
            .remove("load_avg_5")
            .and_then(|v| str_or_f64(&v))
            .unwrap_or(0.0);
        let load_15 = cpu
            .remove("load_avg_15")
            .and_then(|v| str_or_f64(&v))
            .unwrap_or(0.0);

        cpu.insert("load_1min".into(), json!(load_1));
        cpu.insert("load_5min".into(), json!(load_5));
        cpu.insert("load_15min".into(), json!(load_15));
    }

    // --- Memoria: renombrar campos y convertir GB → MB ---
    if let Some(memory) = perf.get("memory").cloned() {
        let total_gb = memory.get("total_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let used_gb = memory.get("used_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let free_gb = memory.get("free_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let usage_pct = memory
            .get("usage_percent")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let swap_total_gb = memory
            .get("swap_total_gb")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let swap_used_gb = memory
            .get("swap_used_gb")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let total_mb = round2(total_gb * 1024.0);
        let used_mb = round2(used_gb * 1024.0);
        let free_mb = round2(free_gb * 1024.0);
        let swap_total_mb = round2(swap_total_gb * 1024.0);
        let swap_free_mb = round2((swap_total_gb - swap_used_gb).max(0.0) * 1024.0);

        perf["memory"] = json!({
            "mem_usage_percent": usage_pct,
            "mem_total_mb": total_mb,
            "mem_used_mb": used_mb,
            "mem_free_mb": free_mb,
            "mem_available_mb": free_mb,
            "swap_total_mb": swap_total_mb,
            "swap_free_mb": swap_free_mb,
        });
    }

    // --- Disco: clave "disks" → "disk", array → dict indexado ---
    let disks = perf
        .get("disks")
        .and_then(|v| v.as_array())
        .cloned();

    if let Some(disks) = disks {
        let mut disk_dict = Map::new();

        for (i, disk) in disks.iter().enumerate() {
            let idx = (i + 1).to_string();
            let path = disk
                .get("mount")
                .and_then(|v| v.as_str())
                .unwrap_or("/")
                .to_string();
            let total_gb = disk.get("total_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let used_gb = disk.get("used_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let free_gb = disk.get("free_gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let usage_pct = disk
                .get("usage_percent")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            disk_dict.insert(
                idx.clone(),
                json!({
                    "index": idx,
                    "path": path,
                    "total_gb": total_gb,
                    "used_gb": used_gb,
                    "free_gb": free_gb,
                    "percent_used": usage_pct,
                    "source_raw": {
                        "disk_total_raw_gb": total_gb,
                        "disk_used_raw_gb": used_gb,
                        "disk_percent_raw": usage_pct,
                    }
                }),
            );
        }

        if let Some(perf_obj) = perf.as_object_mut() {
            perf_obj.remove("disks");
            perf_obj.insert("disk".into(), Value::Object(disk_dict));
        }
    }
}

// ---------------------------------------------------------------------------
// security: reestructurar secciones con prefijos, añadir "normalized"
// ---------------------------------------------------------------------------
fn transform_security(payload: &mut Value) {
    let security = match payload.get("security").cloned() {
        Some(s) => s,
        None => return,
    };

    let tcp = security.get("tcp").cloned().unwrap_or_default();
    let udp = security.get("udp").cloned().unwrap_or_default();
    let ip = security.get("ip").cloned().unwrap_or_default();
    let icmp = security.get("icmp").cloned().unwrap_or_default();
    let snmp = security.get("snmp_stats").cloned().unwrap_or_default();

    // --- tcp_security (con prefijo "tcp_") ---
    let tcp_active = gi64(&tcp, "active_opens");
    let tcp_passive = gi64(&tcp, "passive_opens");
    let tcp_fail = gi64(&tcp, "attempt_fails");
    let tcp_reset = gi64(&tcp, "estab_resets");
    let tcp_curr = gi64(&tcp, "curr_estab");
    let tcp_in = gi64(&tcp, "in_segs");
    let tcp_out = gi64(&tcp, "out_segs");
    let tcp_retrans = gi64(&tcp, "retrans_segs");
    let tcp_out_rst = gi64(&tcp, "out_resets");
    let tcp_retrans_rate = gf64(&tcp, "retransmission_rate_pct");

    let tcp_security = json!({
        "tcp_active_opens": tcp_active,
        "tcp_passive_opens": tcp_passive,
        "tcp_attempt_fails": tcp_fail,
        "tcp_estab_resets": tcp_reset,
        "tcp_curr_estab": tcp_curr,
        "tcp_in_segs": tcp_in,
        "tcp_out_segs": tcp_out,
        "tcp_retrans_segs": tcp_retrans,
        "tcp_out_rsts": tcp_out_rst,
        "retransmission_rate_percent": tcp_retrans_rate,
    });

    // --- udp_security ---
    let udp_in = gi64(&udp, "in_datagrams");
    let udp_err = gi64(&udp, "in_errors");
    let udp_out = gi64(&udp, "out_datagrams");

    let udp_security = json!({
        "udp_in_datagrams": udp_in,
        "udp_out_datagrams": udp_out,
        "udp_no_ports": 0,
        "udp_in_errors": udp_err,
    });

    // --- ip_security ---
    let ip_in = gi64(&ip, "in_receives");
    let ip_discard = gi64(&ip, "in_discards");
    let ip_frag_ok = gi64(&ip, "frag_creates");
    let ip_frag_fail = gi64(&ip, "frag_fails");

    let ip_security = json!({
        "ip_in_receives": ip_in,
        "ip_in_hdr_errors": 0,
        "ip_in_addr_errors": 0,
        "ip_in_unknown_protos": 0,
        "ip_in_discards": ip_discard,
        "ip_frag_oks": ip_frag_ok,
        "ip_frag_fails": ip_frag_fail,
    });

    // --- icmp_security ---
    let icmp_in = gi64(&icmp, "in_msgs");
    let icmp_err = gi64(&icmp, "in_errors");
    let icmp_echos = gi64(&icmp, "in_echos");

    let icmp_security = json!({
        "icmp_in_msgs": icmp_in,
        "icmp_in_errors": icmp_err,
        "icmp_in_dest_unreachs": 0,
        "icmp_in_time_excds": 0,
        "icmp_in_redirects": 0,
        "icmp_in_echos": icmp_echos,
        "icmp_in_echo_reps": 0,
    });

    // --- snmp_security ---
    let snmp_pkts = gi64(&snmp, "in_pkts");
    let snmp_bad = gi64(&snmp, "bad_community_names");
    let snmp_bad_ver = gi64(&snmp, "bad_versions");

    let snmp_security = json!({
        "snmp_in_pkts": snmp_pkts,
        "snmp_in_bad_community_names": snmp_bad,
        "snmp_in_bad_community_uses": 0,
        "snmp_in_bad_versions": snmp_bad_ver,
        "snmp_in_asn_parse_errs": 0,
        "snmp_in_gen_errs": 0,
    });

    // --- normalized (sección usada por el serializer para tasas/rates) ---
    let udp_error_rate = if udp_in > 0 {
        (udp_err as f64 / udp_in as f64) * 100.0
    } else {
        0.0
    };
    let ip_frag_rate = if ip_in > 0 {
        (ip_frag_ok as f64 / ip_in as f64) * 100.0
    } else {
        0.0
    };
    let echo_rate = if icmp_in > 0 {
        (icmp_echos as f64 / icmp_in as f64) * 100.0
    } else {
        0.0
    };
    let snmp_bad_rate = if snmp_pkts > 0 {
        (snmp_bad as f64 / snmp_pkts as f64) * 100.0
    } else {
        0.0
    };

    let normalized = json!({
        "tcp": {
            "active_opens": tcp_active,
            "passive_opens": tcp_passive,
            "current_estab": tcp_curr,
            "attempt_fails": tcp_fail,
            "estab_resets": tcp_reset,
            "in_segs": tcp_in,
            "out_segs": tcp_out,
            "retrans_segs": tcp_retrans,
            "out_rsts": tcp_out_rst,
            "retransmission_rate_percent": tcp_retrans_rate,
        },
        "udp": {
            "in_datagrams": udp_in,
            "out_datagrams": udp_out,
            "no_ports": 0,
            "in_errors": udp_err,
            "error_rate_percent": round2(udp_error_rate),
        },
        "ip": {
            "in_receives": ip_in,
            "in_hdr_errors": 0,
            "in_addr_errors": 0,
            "in_unknown_protos": 0,
            "in_discards": ip_discard,
            "frag_oks": ip_frag_ok,
            "frag_fails": ip_frag_fail,
            "error_rate_percent": 0.0,
            "fragmentation_rate_percent": round2(ip_frag_rate),
        },
        "icmp": {
            "in_msgs": icmp_in,
            "in_errors": icmp_err,
            "in_echos": icmp_echos,
            "in_echo_reps": 0,
            "echo_reply_rate_percent": round2(echo_rate),
        },
        "snmp": {
            "in_pkts": snmp_pkts,
            "bad_community_names": snmp_bad,
            "bad_versions": snmp_bad_ver,
            "asn_parse_errs": 0,
            "gen_errs": 0,
            "bad_community_rate_percent": round2(snmp_bad_rate),
        },
    });

    payload["security"] = json!({
        "tcp_security": tcp_security,
        "udp_security": udp_security,
        "ip_security": ip_security,
        "icmp_security": icmp_security,
        "snmp_security": snmp_security,
        "normalized": normalized,
    });
}

// ---------------------------------------------------------------------------
// vendor_specific: reestructurar datos específicos del vendor
// ---------------------------------------------------------------------------
fn transform_vendor_specific(payload: &mut Value) {
    let vendor = payload
        .get("metadata")
        .and_then(|m| m.get("vendor"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let vendor_key = format!("{}_specific", vendor);

    match vendor.as_str() {
        "pfsense" => transform_pfsense_specific(payload, &vendor_key),
        "mikrotik_fw" => transform_mikrotik_fw_specific(payload, &vendor_key),
        _ => {} // Otros vendors pasan sin cambios
    }
}

fn transform_pfsense_specific(payload: &mut Value, vendor_key: &str) {
    let pf_data = match payload.get(vendor_key).cloned() {
        Some(d) => d,
        None => return,
    };

    let state_count = pf_data
        .get("pf_state_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let wan_interfaces = pf_data
        .get("wan_interfaces")
        .cloned()
        .unwrap_or(json!([]));

    payload[vendor_key] = json!({
        "firewall_states": {
            "pf_states_current": state_count,
            "pf_states_searches": 0,
            "pf_states_inserts": 0,
            "pf_states_removals": 0,
        },
        "firewall_logs": {
            "pf_log_entries": 0,
            "pf_log_bytes": 0,
            "pf_block_packets": 0,
            "pf_block_bytes": 0,
        },
        "wan_interfaces": wan_interfaces,
        "internet_channels": {
            "channels": [],
            "summary": {
                "total_channels": 0,
                "channels_up": 0,
                "channels_down": 0,
                "total_traffic_in_mb": 0,
                "total_traffic_out_mb": 0,
                "netwatch_available": false,
                "queues_available": false,
            }
        },
    });
}

fn transform_mikrotik_fw_specific(payload: &mut Value, vendor_key: &str) {
    let mk_data = match payload.get(vendor_key).cloned() {
        Some(d) => d,
        None => return,
    };

    let mut transformed = mk_data.clone();
    if let Some(obj) = transformed.as_object_mut() {
        // Provider global (opcional) tomado desde metadata
        let provider_global = payload
            .get("metadata")
            .and_then(|m| m.get("provider"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        // Alinear estructura de queues al formato Python:
        // { available: bool, entries: [...], summary: {...} }
        let queues_value = obj.get("queues").cloned();
        match queues_value {
            Some(Value::Array(items)) => {
                let mut entries = Vec::new();
                let mut total_rx_gb = 0.0;
                let mut total_tx_gb = 0.0;
                let mut total_rx_drops: i64 = 0;
                let mut total_tx_drops: i64 = 0;

                for (i, item) in items.iter().enumerate() {
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let bytes_in = item.get("bytes_in").and_then(|v| v.as_u64()).unwrap_or(0);
                    let bytes_out = item.get("bytes_out").and_then(|v| v.as_u64()).unwrap_or(0);
                    let dropped = item.get("packets_dropped").and_then(|v| v.as_i64()).unwrap_or(0);

                    // El collector Rust no trae todavía src/dst/interface para queue.
                    // Se preserva compatibilidad del backend usando valores por defecto.
                    let rx_gb = round4(bytes_in as f64 / (1024.0_f64.powi(3)));
                    let tx_gb = round4(bytes_out as f64 / (1024.0_f64.powi(3)));
                    total_rx_gb += rx_gb;
                    total_tx_gb += tx_gb;
                    total_tx_drops += dropped;

                    entries.push(json!({
                        "index": (i + 1).to_string(),
                        "name": name,
                        "src_addr": "",
                        "dst_addr": "",
                        "interface": "",
                        "tx_bytes": bytes_out,
                        "rx_bytes": bytes_in,
                        "tx_packets": 0,
                        "rx_packets": 0,
                        "tx_drop": dropped,
                        "rx_drop": 0,
                        "isp_detected": provider_global.clone().map(|s| json!(s)).unwrap_or(serde_json::Value::Null),
                        "tx_gb": tx_gb,
                        "rx_gb": rx_gb,
                    }));
                }

                obj.insert(
                    "queues".into(),
                    json!({
                        "entries": entries,
                        "summary": {
                            "total_queues": entries.len(),
                            "total_tx_gb": round3(total_tx_gb),
                            "total_rx_gb": round3(total_rx_gb),
                            "total_tx_drops": total_tx_drops,
                            "total_rx_drops": total_rx_drops,
                        },
                        "available": true,
                    }),
                );
            }
            Some(Value::Object(existing)) => {
                // Si ya viene en formato objeto, asegurar bandera available.
                if !existing.contains_key("available") {
                    let has_entries = existing
                        .get("entries")
                        .and_then(|v| v.as_array())
                        .map(|a| !a.is_empty())
                        .unwrap_or(false);
                    let mut patched = existing.clone();
                    patched.insert("available".into(), json!(has_entries));
                    obj.insert("queues".into(), Value::Object(patched));
                }
            }
            _ => {
                obj.insert(
                    "queues".into(),
                    json!({
                        "entries": [],
                        "summary": {
                            "total_queues": 0,
                            "total_tx_gb": 0.0,
                            "total_rx_gb": 0.0,
                            "total_tx_drops": 0,
                            "total_rx_drops": 0,
                        },
                        "available": false,
                    }),
                );
            }
        }

        // Asegurar internet_channels con estructura similar a Python.
        if !obj.contains_key("internet_channels") {
            obj.insert(
                "internet_channels".into(),
                json!({
                    "channels": [],
                    "summary": {
                        "total_channels": 0,
                        "channels_up": 0,
                        "channels_down": 0,
                        "total_traffic_in_mb": 0,
                        "total_traffic_out_mb": 0,
                        "netwatch_available": obj.contains_key("netwatch_probes"),
                        "queues_available": true,
                    },
                    "available": false,
                }),
            );
        }

        let wan_interfaces_snapshot = obj
            .get("wan_interfaces")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if let Some(ch_obj) = obj.get_mut("internet_channels").and_then(|v| v.as_object_mut()) {
            let existing_channels = ch_obj
                .get("channels")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            // Si no hay canales construidos, derivarlos de wan_interfaces (paridad base con Python).
            let channels = if existing_channels.is_empty() {
                let mut derived = Vec::new();
                for iface in &wan_interfaces_snapshot {
                    let oper = iface
                        .get("oper_status")
                        .and_then(|v| v.as_str())
                        .or_else(|| iface.get("operational_status").and_then(|v| v.as_str()))
                        .unwrap_or("unknown");
                    let in_mb = iface
                        .get("traffic_in_mb")
                        .and_then(|v| v.as_f64())
                        .unwrap_or_else(|| {
                            iface.get("in_bytes")
                                .and_then(|v| v.as_u64())
                                .map(|x| x as f64 / 1_048_576.0)
                                .unwrap_or(0.0)
                        });
                    let out_mb = iface
                        .get("traffic_out_mb")
                        .and_then(|v| v.as_f64())
                        .unwrap_or_else(|| {
                            iface.get("out_bytes")
                                .and_then(|v| v.as_u64())
                                .map(|x| x as f64 / 1_048_576.0)
                                .unwrap_or(0.0)
                        });

                    let name = iface.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                    derived.push(json!({
                        "channel_name": name,
                        "isp": provider_global.clone().unwrap_or_else(|| "Desconocido".to_string()),
                        "source": "wan_interface",
                        "oper_status": oper,
                        "is_up": oper.eq_ignore_ascii_case("UP"),
                        "speed_mbps": iface.get("speed_mbps").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        "traffic_in_mb": round2(in_mb),
                        "traffic_out_mb": round2(out_mb),
                        "errors_in": iface.get("errors_in").and_then(|v| v.as_i64()).unwrap_or(0),
                        "errors_out": iface.get("errors_out").and_then(|v| v.as_i64()).unwrap_or(0),
                        "discards_in": iface.get("discards_in").and_then(|v| v.as_i64()).unwrap_or(0),
                        "discards_out": iface.get("discards_out").and_then(|v| v.as_i64()).unwrap_or(0),
                        "netwatch_status": serde_json::Value::Null,
                        "alerts": if oper.eq_ignore_ascii_case("DOWN") {
                            vec![format!("Canal WAN DOWN: {}", name)]
                        } else {
                            Vec::<String>::new()
                        },
                    }));
                }
                derived
            } else {
                existing_channels
            };

            let channels_up = channels
                .iter()
                .filter(|c| c.get("is_up").and_then(|v| v.as_bool()).unwrap_or(false))
                .count();
            let channels_down = channels
                .iter()
                .filter(|c| {
                    c.get("oper_status")
                        .and_then(|v| v.as_str())
                        .map(|s| s.eq_ignore_ascii_case("DOWN"))
                        .unwrap_or(false)
                })
                .count();
            let total_in_mb: f64 = channels
                .iter()
                .map(|c| c.get("traffic_in_mb").and_then(|v| v.as_f64()).unwrap_or(0.0))
                .sum();
            let total_out_mb: f64 = channels
                .iter()
                .map(|c| c.get("traffic_out_mb").and_then(|v| v.as_f64()).unwrap_or(0.0))
                .sum();

            let netwatch_available = ch_obj
                .get("summary")
                .and_then(|s| s.get("netwatch_available"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let queues_available = ch_obj
                .get("summary")
                .and_then(|s| s.get("queues_available"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            ch_obj.insert("channels".into(), Value::Array(channels.clone()));
            ch_obj.insert(
                "summary".into(),
                json!({
                    "total_channels": channels.len(),
                    "channels_up": channels_up,
                    "channels_down": channels_down,
                    "total_traffic_in_mb": round2(total_in_mb),
                    "total_traffic_out_mb": round2(total_out_mb),
                    "netwatch_available": netwatch_available,
                    "queues_available": queues_available,
                }),
            );
            ch_obj.insert(
                "available".into(),
                json!(!channels.is_empty() || netwatch_available || queues_available),
            );
        }
    }
    payload[vendor_key] = transformed;
}

// ---------------------------------------------------------------------------
// Geolocation & Provider enrichment
// ---------------------------------------------------------------------------
fn transform_geolocation(payload: &mut Value) {
    // Try to find a public IP to lookup: prefer BGP peer remote_addr, else SNMP host
    let ip_opt = payload
        .get("network")
        .and_then(|n| n.get("bgp"))
        .and_then(|b| b.get("peers"))
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.get(0))
        .and_then(|first| first.get("remote_addr"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let ip = if let Some(ip) = ip_opt {
        ip
    } else {
        payload
            .get("metadata")
            .and_then(|m| m.get("snmp_host"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    if ip.is_empty() {
        return;
    }

    // City lookup
    if let Some(city_val) = geoip::lookup_city(&ip) {
        if let Some(meta) = payload.get_mut("metadata").and_then(|m| m.as_object_mut()) {
            meta.insert(
                "geolocation".to_string(),
                json!({
                    "enabled": true,
                    "timestamp": helpers::now_iso_utc(),
                    "lookup_method": "GeoLite2-City",
                    "result": city_val.clone()
                }),
            );
        }
    }

    // ASN lookup -> provider
    if let Some(asn_val) = geoip::lookup_asn(&ip) {
        if let Some(meta) = payload.get_mut("metadata").and_then(|m| m.as_object_mut()) {
            let asn_str = asn_val
                .get("asn")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let org = asn_val
                .get("organization")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            meta.insert("asn".to_string(), json!(asn_str));
            meta.insert("provider".to_string(), json!(format!("AS{} {}", asn_str, org)));
        }
    } else {
        // Fallback: use bgp.local_as if present (take owned String to avoid borrow issues)
        if let Some(local_as_owned) = payload
            .get("network")
            .and_then(|n| n.get("bgp"))
            .and_then(|b| b.get("local_as"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            if let Some(meta) = payload.get_mut("metadata").and_then(|m| m.as_object_mut()) {
                meta.insert("provider".to_string(), json!(format!("AS{}", local_as_owned)));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Redondea a 2 decimales.
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Redondea a 3 decimales.
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Redondea a 4 decimales.
fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

/// Extrae i64 de un JSON Value por clave.
fn gi64(obj: &Value, key: &str) -> i64 {
    obj.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

/// Extrae f64 de un JSON Value por clave.
fn gf64(obj: &Value, key: &str) -> f64 {
    obj.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

/// Convierte un Value que puede ser string o número a f64.
fn str_or_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
}
