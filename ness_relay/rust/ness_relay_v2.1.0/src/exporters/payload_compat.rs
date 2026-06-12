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

/// Transforma el payload del formato interno Rust al formato compatible
/// con el serializer del servidor NESS (formato Python).
pub fn transform_for_server(mut payload: Value) -> Value {
    transform_system(&mut payload);
    transform_network(&mut payload);
    transform_performance(&mut payload);
    transform_security(&mut payload);
    transform_vendor_specific(&mut payload);
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

    // Extraer versión de firmware desde sys_descr si es posible de forma robusta
    if let Some(sys_descr) = system.get("sys_descr").and_then(|v| v.as_str()) {
        let descr_lower = sys_descr.to_lowercase();
        let mut version_str = "";
        
        if let Some(idx) = descr_lower.find("software version ") {
            let start = idx + 17;
            version_str = sys_descr[start..].trim_start().split(|c: char| c.is_whitespace() || c == ',').next().unwrap_or("");
        } else if let Some(idx) = descr_lower.find("version ") {
            let start = idx + 8;
            version_str = sys_descr[start..].trim_start().split(|c: char| c.is_whitespace() || c == ',').next().unwrap_or("");
        }
        
        if !version_str.is_empty() {
            basic_info.insert("firmware_version".into(), json!(version_str));
            basic_info.insert("os_version".into(), json!(version_str));
        }
    }

    if !basic_info.contains_key("firmware_version") {
        if let Some(tp_link) = payload.get("tp_link_specific") {
            if let Some(fw) = tp_link.get("firmware_version") {
                basic_info.insert("firmware_version".into(), fw.clone());
                basic_info.insert("os_version".into(), fw.clone());
            }
        }
        if let Some(huawei) = payload.get("huawei_specific") {
            if let Some(fw) = huawei.get("firmware_version") {
                basic_info.insert("firmware_version".into(), fw.clone());
                basic_info.insert("os_version".into(), fw.clone());
            }
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

    // Construir map de preservación: iniciar con basic_info, luego iterar system keys
    let mut transformed = Map::new();
    transformed.insert("basic_info".into(), Value::Object(basic_info));

    // Preservar campos adicionales (timestamps, etc) que no sean los core fields
    if let Some(sys_obj) = system.as_object() {
        for (k, v) in sys_obj.iter() {
            // Skip los campos que ya fueron procesados
            if !["sys_name", "sys_descr", "sys_location", "sys_contact", "uptime"].contains(&k.as_str()) {
                transformed.entry(k.clone()).or_insert(v.clone());
            }
        }
    }

    payload["system"] = Value::Object(transformed);
}

// ---------------------------------------------------------------------------
// network.interfaces: array → dict, renombrar campos, convertir unidades
// ---------------------------------------------------------------------------
fn transform_network(payload: &mut Value) {
    let network = match payload.get_mut("network") {
        Some(n) => n,
        None => return,
    };

    let interfaces = match network.get("interfaces").and_then(|v| v.as_array()) {
        Some(arr) => arr.clone(),
        None => return,
    };

    let mut interfaces_dict = Map::new();

    for iface in &interfaces {
        let index = iface
            .get("index")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string();

        let speed_bps = iface
            .get("speed_bps")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let speed_mbps = speed_bps / 1_000_000;

        let in_octets = iface
            .get("in_octets")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let out_octets = iface
            .get("out_octets")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let traffic_in_mb = round2(in_octets as f64 / 1_048_576.0);
        let traffic_out_mb = round2(out_octets as f64 / 1_048_576.0);

        let admin_status = iface
            .get("admin_status")
            .and_then(|v| v.as_str())
            .unwrap_or("down")
            .to_uppercase();
        let oper_status = iface
            .get("oper_status")
            .and_then(|v| v.as_str())
            .unwrap_or("down")
            .to_uppercase();

        let errors_in = iface.get("in_errors").and_then(|v| v.as_u64()).unwrap_or(0);
        let errors_out = iface.get("out_errors").and_then(|v| v.as_u64()).unwrap_or(0);
        let discards_in = iface.get("in_discards").and_then(|v| v.as_u64()).unwrap_or(0);
        let discards_out = iface.get("out_discards").and_then(|v| v.as_u64()).unwrap_or(0);

        let name = iface
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

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
            }),
        );
    }

    network["interfaces"] = Value::Object(interfaces_dict.clone());
    
    // Remover metadatos internos
    if let Some(network_obj) = network.as_object_mut() {
        network_obj.remove("interface_count");
    }
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
        let total_gb = memory.get("total_gb").and_then(|v| str_or_f64(&v)).unwrap_or(0.0);
        let used_gb = memory.get("used_gb").and_then(|v| str_or_f64(&v)).unwrap_or(0.0);
        let free_gb = memory.get("free_gb").and_then(|v| str_or_f64(&v)).unwrap_or(0.0);
        
        let usage_pct = memory.get("mem_usage_percent").and_then(|v| str_or_f64(&v))
            .unwrap_or_else(|| memory.get("usage_percent").and_then(|v| str_or_f64(&v)).unwrap_or(0.0));
            
        let swap_total_gb = memory.get("swap_total_gb").and_then(|v| str_or_f64(&v)).unwrap_or(0.0);
        let swap_used_gb = memory.get("swap_used_gb").and_then(|v| str_or_f64(&v)).unwrap_or(0.0);

        let total_mb = memory.get("mem_total_mb").and_then(|v| str_or_f64(&v)).unwrap_or_else(|| round2(total_gb * 1024.0));
        let used_mb = memory.get("mem_used_mb").and_then(|v| str_or_f64(&v)).unwrap_or_else(|| round2(used_gb * 1024.0));
        let free_mb = memory.get("mem_free_mb").and_then(|v| str_or_f64(&v)).unwrap_or_else(|| round2(free_gb * 1024.0));
        
        let swap_total_mb = memory.get("swap_total_mb").and_then(|v| str_or_f64(&v)).unwrap_or_else(|| round2(swap_total_gb * 1024.0));
        let swap_free_mb = memory.get("swap_free_mb").and_then(|v| str_or_f64(&v)).unwrap_or_else(|| round2((swap_total_gb - swap_used_gb).max(0.0) * 1024.0));

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

    // Preservar campos adicionales (como collection_timestamp) que no sean los core security fields
    if let Some(sec_obj) = security.as_object() {
        if let Some(transformed) = payload.get_mut("security").and_then(|v| v.as_object_mut()) {
            for (k, v) in sec_obj.iter() {
                // Skip los campos que ya fueron procesados
                if !["tcp", "udp", "ip", "icmp", "snmp_stats"].contains(&k.as_str()) {
                    transformed.entry(k.clone()).or_insert(v.clone());
                }
            }
        }
    }
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
        // Alinear estructura de queues al formato Python:
        // { available: bool, entries: [...], summary: {...} }
        // NO inyectar datos sintéticos - preservar exactamente como vienen del collector
        let queues_value = obj.get("queues").cloned();
        match queues_value {
            Some(Value::Array(items)) => {
                // Preservar items exactamente como vienen, solo calcular totals
                let mut entries = items.clone();
                let mut total_rx_gb = 0.0;
                let mut total_tx_gb = 0.0;
                let mut total_rx_drops: u64 = 0;
                let mut total_tx_drops: u64 = 0;

                for item in &entries {
                    if let Some(rx) = item.get("rx_gb").and_then(|v| v.as_f64()) {
                        total_rx_gb += rx;
                    }
                    if let Some(tx) = item.get("tx_gb").and_then(|v| v.as_f64()) {
                        total_tx_gb += tx;
                    }
                    if let Some(rx_drop) = item.get("rx_drop").and_then(|v| v.as_u64()) {
                        total_rx_drops += rx_drop;
                    }
                    if let Some(tx_drop) = item.get("tx_drop").and_then(|v| v.as_u64()) {
                        total_tx_drops += tx_drop;
                    }
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
                // Si ya viene en formato objeto, preservar sin sobrescribir
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

        // Estructura MINIMALISTA solo si internet_channels no existe
        if !obj.contains_key("internet_channels") {
            let netwatch_available = obj
                .get("netwatch")
                .and_then(|v| v.get("available"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let queues_available = obj
                .get("queues")
                .and_then(|v| v.get("available"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            obj.insert(
                "internet_channels".into(),
                json!({
                    "channels": [],
                    "summary": {
                        "total_channels": 0,
                        "channels_up": 0,
                        "channels_down": 0,
                        "total_traffic_in_mb": 0.0,
                        "total_traffic_out_mb": 0.0,
                        "netwatch_available": netwatch_available,
                        "queues_available": queues_available,
                    }
                }),
            );
        }
    }
    payload[vendor_key] = transformed;
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
