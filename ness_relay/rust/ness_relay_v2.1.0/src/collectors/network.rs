// ==============================================================================
// NESS Relay v2.0.0 — Colector de interfaces de red
// Equivalente Python: collectors/network_collector.py
// ==============================================================================
//
// Recolecta la tabla de interfaces (IF-MIB) usando GETBULK.
// Prefiere contadores HC (64-bit) sobre contadores 32-bit.
// Indexa las interfaces por SNMP index para correlación posterior.
// ==============================================================================

use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::snmp::SnmpClient;
use crate::profiles::standard_oids::{interface_oids, hc_interface_oids};
use crate::utils::helpers::now_iso;

/// Recolecta la tabla completa de interfaces del dispositivo.
/// Retorna un JSON array con una entrada por interfaz.
pub async fn collect(client: &SnmpClient) -> serde_json::Value {
    let oids     = interface_oids();
    let hc_oids  = hc_interface_oids();

    // -----------------------------------------------------------------------
    // Recolectar tablas de interfaces (IF-MIB)
    // -----------------------------------------------------------------------

    // Tabla estándar (32-bit counters)
    let (if_descr, _)       = client.bulk(oids["ifDescr"], 50).await;
    let (if_type, _)        = client.bulk(oids["ifType"], 50).await;
    let (if_speed, _)       = client.bulk(oids["ifSpeed"], 50).await;
    let (if_admin, _)       = client.bulk(oids["ifAdminStatus"], 50).await;
    let (if_oper, _)        = client.bulk(oids["ifOperStatus"], 50).await;
    let (if_in_oct, _)      = client.bulk(oids["ifInOctets"], 50).await;
    let (if_out_oct, _)     = client.bulk(oids["ifOutOctets"], 50).await;
    let (if_in_err, _)      = client.bulk(oids["ifInErrors"], 50).await;
    let (if_out_err, _)     = client.bulk(oids["ifOutErrors"], 50).await;
    let (if_in_disc, _)     = client.bulk(oids["ifInDiscards"], 50).await;
    let (if_out_disc, _)    = client.bulk(oids["ifOutDiscards"], 50).await;
    let (if_in_pkts, _)     = client.bulk(oids["ifInUcastPkts"], 50).await;
    let (if_out_pkts, _)    = client.bulk(oids["ifOutUcastPkts"], 50).await;

    // Tabla HC (64-bit) — IF-MIB RFC 2863 extensión
    let (if_name, _)        = client.bulk(hc_oids["ifName"], 50).await;
    let (if_hc_in, _)       = client.bulk(hc_oids["ifHCInOctets"], 50).await;
    let (if_hc_out, _)      = client.bulk(hc_oids["ifHCOutOctets"], 50).await;
    let (if_high_speed, _)  = client.bulk(hc_oids["ifHighSpeed"], 50).await;
    let (if_alias, _)       = client.bulk(hc_oids["ifAlias"], 50).await;

    // -----------------------------------------------------------------------
    // Construir mapas por índice
    // -----------------------------------------------------------------------
    macro_rules! idx_map_str {
        ($vec:expr) => {
            $vec.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some((idx, v.as_string()))
                })
                .collect::<HashMap<String, String>>()
        };
    }
    macro_rules! idx_map_u64 {
        ($vec:expr) => {
            $vec.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some((idx, v.as_u64().unwrap_or(0)))
                })
                .collect::<HashMap<String, u64>>()
        };
    }
    macro_rules! idx_map_i64 {
        ($vec:expr) => {
            $vec.into_iter()
                .filter_map(|(oid, v)| {
                    let idx = oid.rsplit('.').next()?.to_string();
                    Some((idx, v.as_i64().unwrap_or(0)))
                })
                .collect::<HashMap<String, i64>>()
        };
    }

    let descr_map   = idx_map_str!(if_descr);
    let name_map    = idx_map_str!(if_name);
    let alias_map   = idx_map_str!(if_alias);
    let type_map    = idx_map_i64!(if_type);
    let speed_map   = idx_map_u64!(if_speed);
    let admin_map   = idx_map_i64!(if_admin);
    let oper_map    = idx_map_i64!(if_oper);

    // Contadores 32-bit
    let in_oct_map  = idx_map_u64!(if_in_oct);
    let out_oct_map = idx_map_u64!(if_out_oct);
    let in_err_map  = idx_map_u64!(if_in_err);
    let out_err_map = idx_map_u64!(if_out_err);
    let in_disc_map = idx_map_u64!(if_in_disc);
    let out_disc_map= idx_map_u64!(if_out_disc);
    let in_pkt_map  = idx_map_u64!(if_in_pkts);
    let out_pkt_map = idx_map_u64!(if_out_pkts);

    // Contadores 64-bit (HC)
    let hc_in_map   = idx_map_u64!(if_hc_in);
    let hc_out_map  = idx_map_u64!(if_hc_out);
    let hispeed_map = idx_map_u64!(if_high_speed);

    // -----------------------------------------------------------------------
    // Construir lista de interfaces
    // -----------------------------------------------------------------------
    let mut interfaces = Vec::new();

    // Usar los índices del mapa de descriptions (base de IF-MIB)
    let mut all_indices: std::collections::BTreeSet<String> = descr_map.keys().cloned().collect();
    // Añadir índices de ifName por si ifDescr no tiene todos
    for k in name_map.keys() { all_indices.insert(k.clone()); }

    for idx in all_indices {
        let descr = descr_map.get(&idx).cloned().unwrap_or_default();
        let name  = name_map.get(&idx).cloned().unwrap_or(descr.clone());
        let alias = alias_map.get(&idx).cloned().unwrap_or_default();

        let admin_status = admin_map.get(&idx).copied().unwrap_or(2);
        let oper_status  = oper_map.get(&idx).copied().unwrap_or(2);

        // Velocidad: preferir ifHighSpeed (Mbps → bps), luego ifSpeed (bps)
        let speed_bps = if let Some(&hs) = hispeed_map.get(&idx) {
            if hs > 0 { hs * 1_000_000 } else { speed_map.get(&idx).copied().unwrap_or(0) }
        } else {
            speed_map.get(&idx).copied().unwrap_or(0)
        };

        // Contadores: preferir HC (64-bit) sobre 32-bit
        let in_octets  = if let Some(&hc) = hc_in_map.get(&idx) {
            if hc > 0 { hc } else { in_oct_map.get(&idx).copied().unwrap_or(0) }
        } else {
            in_oct_map.get(&idx).copied().unwrap_or(0)
        };
        let out_octets = if let Some(&hc) = hc_out_map.get(&idx) {
            if hc > 0 { hc } else { out_oct_map.get(&idx).copied().unwrap_or(0) }
        } else {
            out_oct_map.get(&idx).copied().unwrap_or(0)
        };

        let in_errors   = in_err_map.get(&idx).copied().unwrap_or(0);
        let out_errors  = out_err_map.get(&idx).copied().unwrap_or(0);
        let in_discards = in_disc_map.get(&idx).copied().unwrap_or(0);
        let out_discards= out_disc_map.get(&idx).copied().unwrap_or(0);
        let in_pkts     = in_pkt_map.get(&idx).copied().unwrap_or(0);
        let out_pkts    = out_pkt_map.get(&idx).copied().unwrap_or(0);

        interfaces.push(json!({
            "index": idx,
            "name": name,
            "description": descr,
            "alias": alias,
            "type": type_map.get(&idx).copied().unwrap_or(0),
            "speed_bps": speed_bps,
            "admin_status": if admin_status == 1 { "up" } else { "down" },
            "oper_status": if oper_status == 1 { "up" } else { "down" },
            "in_octets": in_octets,
            "out_octets": out_octets,
            "in_errors": in_errors,
            "out_errors": out_errors,
            "in_discards": in_discards,
            "out_discards": out_discards,
            "in_unicast_pkts": in_pkts,
            "out_unicast_pkts": out_pkts,
        }));
    }

    debug!("Interfaces recolectadas: {}", interfaces.len());

    json!({
        "interfaces": interfaces,
        "interface_count": interfaces.len(),
        "collection_timestamp": now_iso(),
    })
}
