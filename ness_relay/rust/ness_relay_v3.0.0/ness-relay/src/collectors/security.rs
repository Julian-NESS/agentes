// ==============================================================================
// NESS Relay v2.0.0 — Colector de seguridad (TCP/UDP/IP/ICMP/SNMP stats)
// Equivalente Python: collectors/security_collector.py
// ==============================================================================
//
// Recolecta estadísticas de protocolos de capa 3/4 para análisis de seguridad:
//   - TCP: conexiones, retransmisiones, resets
//   - UDP: datagramas con errores
//   - IP: fragmentación
//   - ICMP: mensajes (detección de reconocimiento)
//   - SNMP: intentos de autenticación fallidos
// ==============================================================================

use serde_json::json;
use tracing::debug;

use crate::snmp::SnmpClient;
use crate::profiles::standard_oids::{tcp_oids, udp_oids, ip_oids, icmp_oids, snmp_stats_oids};
use crate::utils::helpers::now_iso;

/// Recolecta estadísticas de seguridad del dispositivo.
pub async fn collect(client: &SnmpClient) -> serde_json::Value {
    let tcp_o  = tcp_oids();
    let udp_o  = udp_oids();
    let ip_o   = ip_oids();
    let icmp_o = icmp_oids();
    let snmp_o = snmp_stats_oids();

    // -----------------------------------------------------------------------
    // TCP
    // -----------------------------------------------------------------------
    macro_rules! get_i64 {
        ($oid_map:expr, $key:expr) => {
            client.get($oid_map[$key]).await
                .value.as_ref().and_then(|v| v.as_i64()).unwrap_or(0)
        };
    }

    let tcp_active  = get_i64!(tcp_o, "tcpActiveOpens");
    let tcp_passive = get_i64!(tcp_o, "tcpPassiveOpens");
    let tcp_fail    = get_i64!(tcp_o, "tcpAttemptFails");
    let tcp_reset   = get_i64!(tcp_o, "tcpEstabResets");
    let tcp_curr    = get_i64!(tcp_o, "tcpCurrEstab");
    let tcp_in      = get_i64!(tcp_o, "tcpInSegs");
    let tcp_out     = get_i64!(tcp_o, "tcpOutSegs");
    let tcp_retrans = get_i64!(tcp_o, "tcpRetransSegs");
    let tcp_in_err  = get_i64!(tcp_o, "tcpInErrs");
    let tcp_out_rst = get_i64!(tcp_o, "tcpOutRsts");

    let retrans_rate = if tcp_out > 0 {
        (tcp_retrans as f64 / tcp_out as f64) * 100.0
    } else {
        0.0
    };

    // -----------------------------------------------------------------------
    // UDP
    // -----------------------------------------------------------------------
    let udp_in  = get_i64!(udp_o, "udpInDatagrams");
    let udp_err = get_i64!(udp_o, "udpInErrors");
    let udp_out = get_i64!(udp_o, "udpOutDatagrams");

    // -----------------------------------------------------------------------
    // IP
    // -----------------------------------------------------------------------
    let ip_in        = get_i64!(ip_o, "ipInReceives");
    let ip_fwd       = get_i64!(ip_o, "ipForwDatagrams");
    let ip_discard   = get_i64!(ip_o, "ipInDiscards");
    let ip_reasm     = get_i64!(ip_o, "ipReasmReqds");
    let ip_reasm_fail= get_i64!(ip_o, "ipReasmFails");
    let ip_frag_ok   = get_i64!(ip_o, "ipFragOKs");
    let ip_frag_fail = get_i64!(ip_o, "ipFragFails");
    let ip_frag_creates = get_i64!(ip_o, "ipFragCreates");

    let ip_frag_rate = if ip_in > 0 {
        (ip_frag_creates as f64 / ip_in as f64) * 100.0
    } else {
        0.0
    };

    // -----------------------------------------------------------------------
    // ICMP
    // -----------------------------------------------------------------------
    let icmp_in      = get_i64!(icmp_o, "icmpInMsgs");
    let icmp_in_echo = get_i64!(icmp_o, "icmpInEchos");
    let icmp_out     = get_i64!(icmp_o, "icmpOutMsgs");
    let icmp_errors  = get_i64!(icmp_o, "icmpInErrors");

    // -----------------------------------------------------------------------
    // SNMP Stats (intentos de autenticación fallidos)
    // -----------------------------------------------------------------------
    let snmp_bad_community = get_i64!(snmp_o, "snmpInBadCommunityNames");
    let snmp_bad_version   = get_i64!(snmp_o, "snmpInBadVersions");
    let snmp_in_pkts       = get_i64!(snmp_o, "snmpInPkts");

    debug!("Seguridad: TCP curr={}, retrans_rate={:.2}%, ICMP in={}, SNMP bad_comm={}",
        tcp_curr, retrans_rate, icmp_in, snmp_bad_community);

    json!({
        "tcp": {
            "active_opens": tcp_active,
            "passive_opens": tcp_passive,
            "attempt_fails": tcp_fail,
            "estab_resets": tcp_reset,
            "curr_estab": tcp_curr,
            "in_segs": tcp_in,
            "out_segs": tcp_out,
            "retrans_segs": tcp_retrans,
            "in_errors": tcp_in_err,
            "out_resets": tcp_out_rst,
            "retransmission_rate_pct": (retrans_rate * 100.0).round() / 100.0,
        },
        "udp": {
            "in_datagrams": udp_in,
            "in_errors": udp_err,
            "out_datagrams": udp_out,
        },
        "ip": {
            "in_receives": ip_in,
            "forward_datagrams": ip_fwd,
            "in_discards": ip_discard,
            "reasm_required": ip_reasm,
            "reasm_fails": ip_reasm_fail,
            "frag_creates": ip_frag_creates,
            "frag_fails": ip_frag_fail,
            "fragmentation_rate_pct": (ip_frag_rate * 100.0).round() / 100.0,
        },
        "icmp": {
            "in_msgs": icmp_in,
            "in_errors": icmp_errors,
            "in_echos": icmp_in_echo,
            "out_msgs": icmp_out,
        },
        "snmp_stats": {
            "in_pkts": snmp_in_pkts,
            "bad_community_names": snmp_bad_community,
            "bad_versions": snmp_bad_version,
        },
        "collection_timestamp": now_iso(),
    })
}
