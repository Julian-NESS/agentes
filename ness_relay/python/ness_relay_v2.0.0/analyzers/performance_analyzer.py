# ==============================================================================
# NESS Relay v2.0.0 - Performance Analyzer
# ==============================================================================
# Analiza métricas de rendimiento para generar alertas de CPU, memoria
# e interfaces con errores.
# ==============================================================================

import logging
from typing import Any, Dict, List

from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


def analyze_performance_metrics(data: Dict[str, Any]) -> Dict[str, Any]:
    """
    Analiza métricas de rendimiento para detectar problemas de performance.
    
    Genera alertas (critical) y advertencias (warning) basadas en umbrales
    para:
    - Uso de CPU (>90% critical, >80% warning)
    - Uso de memoria (>90% critical, >85% warning)
    - Errores en interfaces (>1000 critical, >100 warning)
    
    Args:
        data: Diccionario completo con todos los datos recolectados.
              Espera claves: 'performance' con sub-claves cpu, memory;
              'network' con sub-clave interfaces.
        
    Returns:
        Diccionario con el resultado del análisis de performance.
    """
    logger.info("Iniciando análisis de métricas de performance...")
    
    performance_alerts: List[Dict[str, Any]] = []
    performance_warnings: List[Dict[str, Any]] = []
    
    # === CPU Usage ===
    cpu_data = data.get("performance", {}).get("cpu", {})
    cpu_usage = _safe_numeric(cpu_data.get("cpu_usage_percent"))
    
    if cpu_usage > 90:
        performance_alerts.append({
            "level": "critical",
            "type": "cpu_usage",
            "message": f"Uso de CPU crítico: {cpu_usage:.1f}% (>90%)",
            "value": cpu_usage,
            "threshold": 90
        })
    elif cpu_usage > 80:
        performance_warnings.append({
            "level": "warning",
            "type": "cpu_usage",
            "message": f"Uso de CPU elevado: {cpu_usage:.1f}% (>80%)",
            "value": cpu_usage,
            "threshold": 80
        })
    
    # === Memory Usage ===
    mem_data = data.get("performance", {}).get("memory", {})
    mem_usage = _safe_numeric(mem_data.get("mem_usage_percent"))
    
    if mem_usage > 90:
        performance_alerts.append({
            "level": "critical",
            "type": "memory_usage",
            "message": f"Uso de memoria crítico: {mem_usage:.1f}% (>90%)",
            "value": mem_usage,
            "threshold": 90
        })
    elif mem_usage > 85:
        performance_warnings.append({
            "level": "warning",
            "type": "memory_usage",
            "message": f"Uso de memoria elevado: {mem_usage:.1f}% (>85%)",
            "value": mem_usage,
            "threshold": 85
        })
    
    # === Interface Errors ===
    network_data = data.get("network", {}).get("interfaces", {})
    for interface_id, interface_data in network_data.items():
        total_errors = interface_data.get("total_errors", 0)
        if not isinstance(total_errors, int):
            total_errors = 0
        
        interface_name = interface_data.get("name", f"Interface {interface_id}")
        
        if total_errors > 1000:
            performance_alerts.append({
                "level": "critical",
                "type": "interface_errors",
                "message": f"Errores críticos en {interface_name}: {total_errors:,} errores",
                "value": total_errors,
                "threshold": 1000,
                "interface": interface_name
            })
        elif total_errors > 100:
            performance_warnings.append({
                "level": "warning",
                "type": "interface_errors",
                "message": f"Errores elevados en {interface_name}: {total_errors:,} errores",
                "value": total_errors,
                "threshold": 100,
                "interface": interface_name
            })
    
    # === Resultado ===
    performance_analysis = {
        "timestamp": now_iso(),
        "total_alerts": len(performance_alerts),
        "total_warnings": len(performance_warnings),
        "alerts": performance_alerts,
        "warnings": performance_warnings,
        "performance_status": (
            "critical" if performance_alerts
            else ("warning" if performance_warnings else "ok")
        )
    }
    
    logger.info(
        f"Análisis de performance completado: "
        f"{len(performance_alerts)} alertas, {len(performance_warnings)} advertencias"
    )
    return performance_analysis


def _safe_numeric(value: Any) -> float:
    """Extrae valor numérico seguro."""
    if isinstance(value, (int, float)):
        return float(value)
    return 0.0
