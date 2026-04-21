# ==============================================================================
# NESS Relay v2.0.0 - Security Analyzer
# ==============================================================================
# Analiza datos de seguridad recolectados para generar alertas y warnings.
# Mismas reglas lógicas que v1.0.4 con defensas contra tipos inválidos.
# ==============================================================================

import logging
from typing import Any, Dict, List

from utils.conversions import calculate_percentage
from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


def analyze_security_threats(data: Dict[str, Any]) -> Dict[str, Any]:
    """
    Analiza los datos recolectados para detectar amenazas de seguridad.
    
    Genera alertas (critical) y advertencias (warning) basadas en umbrales
    predefinidos para:
    - Conexiones TCP excesivas / fallos / retransmisiones
    - Accesos SNMP no autorizados
    - Fragmentación IP anormal
    - Actividad ICMP sospechosa
    
    Args:
        data: Diccionario completo con todos los datos recolectados.
              Espera claves: 'security' con sub-claves tcp_security, 
              snmp_security, ip_security, icmp_security.
        
    Returns:
        Diccionario con el resultado del análisis de seguridad.
    """
    logger.info("Iniciando análisis de amenazas de seguridad...")
    
    alerts: List[Dict[str, Any]] = []
    warnings: List[Dict[str, Any]] = []
    
    # === TCP Connections ===
    tcp_data = data.get("security", {}).get("tcp_security", {})
    
    tcp_connections = _safe_int_val(tcp_data.get("tcp_curr_estab"))
    if tcp_connections > 10000:
        alerts.append({
            "level": "critical",
            "type": "tcp_connections",
            "message": f"Conexiones TCP críticas: {tcp_connections:,} (>10,000)",
            "value": tcp_connections,
            "threshold": 10000
        })
    elif tcp_connections > 5000:
        warnings.append({
            "level": "warning",
            "type": "tcp_connections",
            "message": f"Conexiones TCP elevadas: {tcp_connections:,} (>5,000)",
            "value": tcp_connections,
            "threshold": 5000
        })
    
    # === TCP Failures ===
    tcp_fails = _safe_int_val(tcp_data.get("tcp_attempt_fails"))
    if tcp_fails > 1000:
        alerts.append({
            "level": "critical",
            "type": "tcp_failures",
            "message": f"Fallos de conexión TCP críticos: {tcp_fails:,} (>1,000)",
            "value": tcp_fails,
            "threshold": 1000
        })
    
    # === TCP Retransmission ===
    retrans_rate = _safe_float_val(tcp_data.get("retransmission_rate_percent"))
    if retrans_rate > 10:
        alerts.append({
            "level": "critical",
            "type": "tcp_retransmission",
            "message": f"Tasa de retransmisión TCP crítica: {retrans_rate:.2f}% (>10%)",
            "value": retrans_rate,
            "threshold": 10
        })
    elif retrans_rate > 5:
        warnings.append({
            "level": "warning",
            "type": "tcp_retransmission",
            "message": f"Tasa de retransmisión TCP elevada: {retrans_rate:.2f}% (>5%)",
            "value": retrans_rate,
            "threshold": 5
        })
    
    # === SNMP Unauthorized Access ===
    snmp_data = data.get("security", {}).get("snmp_security", {})
    snmp_bad = _safe_int_val(snmp_data.get("snmp_in_bad_community_names"))
    if snmp_bad > 10:
        alerts.append({
            "level": "critical",
            "type": "snmp_security",
            "message": f"Intentos de acceso SNMP no autorizado: {snmp_bad} (>10)",
            "value": snmp_bad,
            "threshold": 10
        })
    elif snmp_bad > 0:
        warnings.append({
            "level": "warning",
            "type": "snmp_security",
            "message": f"Intentos de acceso SNMP sospechosos: {snmp_bad}",
            "value": snmp_bad,
            "threshold": 0
        })
    
    # === IP Fragmentation ===
    ip_data = data.get("security", {}).get("ip_security", {})
    frag_rate = _safe_float_val(ip_data.get("fragmentation_rate_percent"))
    if frag_rate > 10:
        alerts.append({
            "level": "critical",
            "type": "ip_fragmentation",
            "message": f"Fragmentación IP crítica: {frag_rate:.2f}% (>10%)",
            "value": frag_rate,
            "threshold": 10
        })
    elif frag_rate > 5:
        warnings.append({
            "level": "warning",
            "type": "ip_fragmentation",
            "message": f"Fragmentación IP elevada: {frag_rate:.2f}% (>5%)",
            "value": frag_rate,
            "threshold": 5
        })
    
    # === ICMP Reconnaissance ===
    icmp_data = data.get("security", {}).get("icmp_security", {})
    icmp_echos = _safe_int_val(icmp_data.get("icmp_in_echos"))
    if icmp_echos > 1000:
        alerts.append({
            "level": "critical",
            "type": "icmp_reconnaissance",
            "message": f"ICMP Echo Requests excesivos: {icmp_echos:,} (posible reconocimiento)",
            "value": icmp_echos,
            "threshold": 1000
        })
    
    # === Resultado ===
    analysis_result = {
        "timestamp": now_iso(),
        "total_alerts": len(alerts),
        "total_warnings": len(warnings),
        "alerts": alerts,
        "warnings": warnings,
        "security_status": "critical" if alerts else ("warning" if warnings else "ok")
    }
    
    logger.info(
        f"Análisis de seguridad completado: "
        f"{len(alerts)} alertas, {len(warnings)} advertencias"
    )
    return analysis_result


def _safe_int_val(value: Any) -> int:
    """Extrae entero seguro de un valor que puede ser dict/error."""
    if isinstance(value, int):
        return value
    return 0


def _safe_float_val(value: Any) -> float:
    """Extrae float seguro de un valor que puede ser dict/error."""
    if isinstance(value, (int, float)):
        return float(value)
    return 0.0
