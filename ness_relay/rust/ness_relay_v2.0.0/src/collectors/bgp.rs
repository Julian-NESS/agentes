// ============================================================================
// NESS Relay v2.0.0 — Colector BGP (BGP4-MIB)
// Recolecta `bgpLocalAs` y la `bgpPeerTable` (peers: remoteAddr, remoteAs, state)
// ============================================================================

use std::collections::HashMap;
use serde_json::json;
use tracing::debug;

use crate::snmp::SnmpClient;
use crate::profiles::standard_oids::bgp_oids;
use crate::utils::helpers::now_iso;

fn bgp_state_name(code: i64) -> &'static str {
    match code {
        1 => "idle",
        2 => "connect",
        3 => "active",
        4 => "opensent",
        5 => "openconfirm",
        6 => "established",
        _ => "unknown",
    }
}

pub async fn collect(client: &SnmpClient) -> serde_json::Value {
    let oids = bgp_oids();

    // Intentar obtener bgpLocalAs (scalar)
    let local_as = match client.get(oids["bgpLocalAs"]).await {
        res if res.is_ok() => res.value.unwrap().as_string(),
        _ => "unknown".to_string(),
    };

    // Walk bgpPeerTable
    let (varbinds, err) = client.bulk(oids["bgpPeerTable"], 50).await;
    if err.is_some() {
        debug!("BGP collector: no disponible o error: {:?}", err);
        return json!({
            "available": false,
            "error": err,
            "collected_at": now_iso(),
        });
    }

    // Agrupar por índice de fila (sub-oid después de la columna base)
    let mut peers_map: HashMap<String, serde_json::Map<String, serde_json::Value>> = HashMap::new();

    for (resp_oid, snmp_val) in varbinds.into_iter() {
        let oid = resp_oid.as_str();

        if oid.starts_with(oids["bgpPeerRemoteAddr"]) {
            let idx = oid.trim_start_matches(oids["bgpPeerRemoteAddr"]).trim_start_matches('.').to_string();
            let entry = peers_map.entry(idx).or_insert_with(|| serde_json::Map::new());
            entry.insert("remote_addr".to_string(), serde_json::Value::String(snmp_val.as_string()));
            continue;
        }
        if oid.starts_with(oids["bgpPeerRemoteAs"]) {
            let idx = oid.trim_start_matches(oids["bgpPeerRemoteAs"]).trim_start_matches('.').to_string();
            let entry = peers_map.entry(idx).or_insert_with(|| serde_json::Map::new());
            if let Some(n) = snmp_val.as_u64() {
                entry.insert("remote_as".to_string(), serde_json::Value::Number((n as i64).into()));
            } else {
                entry.insert("remote_as".to_string(), serde_json::Value::String(snmp_val.as_string()));
            }
            continue;
        }
        if oid.starts_with(oids["bgpPeerState"]) {
            let idx = oid.trim_start_matches(oids["bgpPeerState"]).trim_start_matches('.').to_string();
            let entry = peers_map.entry(idx).or_insert_with(|| serde_json::Map::new());
            let state_name = snmp_val.as_i64().map(|c| bgp_state_name(c).to_string()).unwrap_or_else(|| snmp_val.as_string());
            entry.insert("state".to_string(), serde_json::Value::String(state_name));
            continue;
        }
        if oid.starts_with(oids["bgpPeerLocalAddr"]) {
            let idx = oid.trim_start_matches(oids["bgpPeerLocalAddr"]).trim_start_matches('.').to_string();
            let entry = peers_map.entry(idx).or_insert_with(|| serde_json::Map::new());
            entry.insert("local_addr".to_string(), serde_json::Value::String(snmp_val.as_string()));
            continue;
        }
        if oid.starts_with(oids["bgpPeerLastError"]) {
            let idx = oid.trim_start_matches(oids["bgpPeerLastError"]).trim_start_matches('.').to_string();
            let entry = peers_map.entry(idx).or_insert_with(|| serde_json::Map::new());
            entry.insert("last_error".to_string(), serde_json::Value::String(snmp_val.as_string()));
            continue;
        }
    }

    // Construir vector de peers
    let mut peers = Vec::new();
    for (idx, mut map) in peers_map.into_iter() {
        map.insert("index".to_string(), serde_json::Value::String(idx.clone()));
        peers.push(serde_json::Value::Object(map));
    }

    debug!("BGP peers recolectados: {}", peers.len());

    json!({
        "available": true,
        "local_as": local_as,
        "peers": peers,
        "collected_at": now_iso(),
    })
}
