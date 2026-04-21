# ==============================================================================
# NESS Relay v2.0.0 - Security Collector
# ==============================================================================
# Recolecta métricas de seguridad de red: TCP, UDP, IP, ICMP, SNMP stats.
# Todos estos OIDs son estándar RFC y aplican a cualquier vendor.
# Incluye normalización de métricas derivadas (tasas de error, retransmisión).
# ==============================================================================

import logging
from typing import Any, Dict

from core.snmp_client import SnmpClient
from profiles.standard_oids import (
    ICMP_OIDS,
    IP_OIDS,
    SNMP_STATS_OIDS,
    TCP_OIDS,
    UDP_OIDS,
)
from utils.conversions import calculate_percentage, safe_int
from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


async def collect_security_data(client: SnmpClient) -> Dict[str, Any]:
    """
    Recolecta datos de seguridad de red (TCP, UDP, IP, ICMP, SNMP stats).
    
    Todos los OIDs son estándar RFC y aplican a cualquier dispositivo SNMP.
    Adicionalmente normaliza los datos crudos para calcular métricas
    derivadas (tasas de retransmisión, error, fragmentación, etc.).
    
    Args:
        client: Instancia de SnmpClient conectada al dispositivo.
        
    Returns:
        Diccionario con datos de seguridad crudos y normalizados.
    """
    logger.info("Iniciando recolección de datos de seguridad...")
    
    security_data: Dict[str, Any] = {
        "tcp_security": {},
        "udp_security": {},
        "ip_security": {},
        "icmp_security": {},
        "snmp_security": {},
        "collection_timestamp": now_iso()
    }
    
    # ===== TCP =====
    tcp_oid_names = [
        'tcp_active_opens', 'tcp_passive_opens', 'tcp_attempt_fails',
        'tcp_estab_resets', 'tcp_curr_estab', 'tcp_in_segs',
        'tcp_out_segs', 'tcp_retrans_segs', 'tcp_out_rsts'
    ]
    
    for oid_name in tcp_oid_names:
        oid = TCP_OIDS.get(oid_name)
        if oid:
            res = await client.get(oid)
            if not res.error and res.value is not None:
                security_data["tcp_security"][oid_name] = safe_int(res.value)
            else:
                security_data["tcp_security"][oid_name] = {"error": res.error}
    
    # Tasa de retransmisión TCP
    tcp_out = security_data["tcp_security"].get("tcp_out_segs", 0)
    tcp_retrans = security_data["tcp_security"].get("tcp_retrans_segs", 0)
    if isinstance(tcp_out, int) and tcp_out > 0 and isinstance(tcp_retrans, int):
        security_data["tcp_security"]["retransmission_rate_percent"] = calculate_percentage(tcp_retrans, tcp_out)
    
    # ===== UDP =====
    udp_oid_names = ['udp_in_datagrams', 'udp_out_datagrams', 'udp_no_ports', 'udp_in_errors']
    
    for oid_name in udp_oid_names:
        oid = UDP_OIDS.get(oid_name)
        if oid:
            res = await client.get(oid)
            if not res.error and res.value is not None:
                security_data["udp_security"][oid_name] = safe_int(res.value)
            else:
                security_data["udp_security"][oid_name] = {"error": res.error}
    
    # ===== IP =====
    ip_oid_names = [
        'ip_in_receives', 'ip_in_hdr_errors', 'ip_in_addr_errors',
        'ip_in_unknown_protos', 'ip_in_discards', 'ip_frag_oks', 'ip_frag_fails'
    ]
    
    for oid_name in ip_oid_names:
        oid = IP_OIDS.get(oid_name)
        if oid:
            res = await client.get(oid)
            if not res.error and res.value is not None:
                security_data["ip_security"][oid_name] = safe_int(res.value)
            else:
                security_data["ip_security"][oid_name] = {"error": res.error}
    
    # ===== ICMP =====
    icmp_oid_names = [
        'icmp_in_msgs', 'icmp_in_errors', 'icmp_in_dest_unreachs',
        'icmp_in_time_excds', 'icmp_in_redirects', 'icmp_in_echos', 'icmp_in_echo_reps'
    ]
    
    for oid_name in icmp_oid_names:
        oid = ICMP_OIDS.get(oid_name)
        if oid:
            res = await client.get(oid)
            if not res.error and res.value is not None:
                security_data["icmp_security"][oid_name] = safe_int(res.value)
            else:
                security_data["icmp_security"][oid_name] = {"error": res.error}
    
    # ===== SNMP Stats =====
    snmp_oid_names = [
        'snmp_in_pkts', 'snmp_in_bad_community_names', 'snmp_in_bad_community_uses',
        'snmp_in_bad_versions', 'snmp_in_asn_parse_errs', 'snmp_in_gen_errs'
    ]
    
    for oid_name in snmp_oid_names:
        oid = SNMP_STATS_OIDS.get(oid_name)
        if oid:
            res = await client.get(oid)
            if not res.error and res.value is not None:
                security_data["snmp_security"][oid_name] = safe_int(res.value)
            else:
                security_data["snmp_security"][oid_name] = {"error": res.error}
    
    # ===== NORMALIZACIÓN / MÉTRICAS DERIVADAS =====
    security_data["normalized"] = _normalize_security_data(security_data)
    
    logger.info("Datos de seguridad recolectados y normalizados exitosamente")
    return security_data


def _normalize_security_data(raw: Dict[str, Any]) -> Dict[str, Any]:
    """
    Genera métricas normalizadas/derivadas a partir de los datos crudos.
    
    Calcula tasas de error, retransmisión, fragmentación, etc.
    Estas métricas son usadas por el security_analyzer.
    """
    normalized: Dict[str, Any] = {}
    
    # --- TCP normalizado ---
    tcp = raw.get("tcp_security", {})
    tcp_n = {
        "active_opens": _safe_val(tcp.get("tcp_active_opens")),
        "passive_opens": _safe_val(tcp.get("tcp_passive_opens")),
        "current_estab": _safe_val(tcp.get("tcp_curr_estab")),
        "attempt_fails": _safe_val(tcp.get("tcp_attempt_fails")),
        "estab_resets": _safe_val(tcp.get("tcp_estab_resets")),
        "in_segs": _safe_val(tcp.get("tcp_in_segs")),
        "out_segs": _safe_val(tcp.get("tcp_out_segs")),
        "retrans_segs": _safe_val(tcp.get("tcp_retrans_segs")),
        "out_rsts": _safe_val(tcp.get("tcp_out_rsts")),
    }
    tcp_n["retransmission_rate_percent"] = (
        calculate_percentage(tcp_n["retrans_segs"], tcp_n["out_segs"])
        if tcp_n["out_segs"] else 0.0
    )
    normalized["tcp"] = tcp_n
    
    # --- UDP normalizado ---
    udp = raw.get("udp_security", {})
    udp_n = {
        "in_datagrams": _safe_val(udp.get("udp_in_datagrams")),
        "out_datagrams": _safe_val(udp.get("udp_out_datagrams")),
        "no_ports": _safe_val(udp.get("udp_no_ports")),
        "in_errors": _safe_val(udp.get("udp_in_errors")),
    }
    udp_n["error_rate_percent"] = (
        calculate_percentage(udp_n["in_errors"], udp_n["in_datagrams"])
        if udp_n["in_datagrams"] else 0.0
    )
    normalized["udp"] = udp_n
    
    # --- IP normalizado ---
    ip = raw.get("ip_security", {})
    ip_n = {
        "in_receives": _safe_val(ip.get("ip_in_receives")),
        "in_hdr_errors": _safe_val(ip.get("ip_in_hdr_errors")),
        "in_addr_errors": _safe_val(ip.get("ip_in_addr_errors")),
        "in_unknown_protos": _safe_val(ip.get("ip_in_unknown_protos")),
        "in_discards": _safe_val(ip.get("ip_in_discards")),
        "frag_oks": _safe_val(ip.get("ip_frag_oks")),
        "frag_fails": _safe_val(ip.get("ip_frag_fails")),
    }
    ip_errors_sum = (
        ip_n["in_hdr_errors"] + ip_n["in_addr_errors"] +
        ip_n["in_unknown_protos"] + ip_n["in_discards"]
    )
    ip_n["error_rate_percent"] = (
        calculate_percentage(ip_errors_sum, ip_n["in_receives"])
        if ip_n["in_receives"] else 0.0
    )
    ip_n["fragmentation_rate_percent"] = (
        calculate_percentage(ip_n["frag_oks"], ip_n["in_receives"])
        if ip_n["in_receives"] else 0.0
    )
    normalized["ip"] = ip_n
    
    # --- ICMP normalizado ---
    icmp = raw.get("icmp_security", {})
    icmp_n = {
        "in_msgs": _safe_val(icmp.get("icmp_in_msgs")),
        "in_errors": _safe_val(icmp.get("icmp_in_errors")),
        "in_echos": _safe_val(icmp.get("icmp_in_echos")),
        "in_echo_reps": _safe_val(icmp.get("icmp_in_echo_reps")),
    }
    icmp_n["echo_reply_rate_percent"] = (
        calculate_percentage(icmp_n["in_echo_reps"], icmp_n["in_echos"])
        if icmp_n["in_echos"] else 0.0
    )
    normalized["icmp"] = icmp_n
    
    # --- SNMP normalizado ---
    snmp = raw.get("snmp_security", {})
    snmp_n = {
        "in_pkts": _safe_val(snmp.get("snmp_in_pkts")),
        "bad_community_names": _safe_val(snmp.get("snmp_in_bad_community_names")),
        "bad_versions": _safe_val(snmp.get("snmp_in_bad_versions")),
        "asn_parse_errs": _safe_val(snmp.get("snmp_in_asn_parse_errs")),
        "gen_errs": _safe_val(snmp.get("snmp_in_gen_errs")),
    }
    snmp_n["bad_community_rate_percent"] = (
        calculate_percentage(snmp_n["bad_community_names"], snmp_n["in_pkts"])
        if snmp_n["in_pkts"] else 0.0
    )
    normalized["snmp"] = snmp_n
    
    return normalized


def _safe_val(value: Any) -> int:
    """Extrae valor entero seguro, retorna 0 si es dict/error."""
    if isinstance(value, int):
        return value
    return 0
