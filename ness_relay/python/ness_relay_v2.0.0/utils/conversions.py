# ==============================================================================
# NESS Relay v2.0.0 - Conversion Utilities
# ==============================================================================
# Funciones puras de conversión de datos: KB→GB, formateo de uptime,
# cálculo de porcentajes, conversiones seguras de tipos.
# ==============================================================================

from typing import Any, Dict, Optional, Union


def kb_to_gb(kb_value: Any) -> float:
    """Convierte kilobytes a gigabytes con 3 decimales."""
    try:
        return round(float(kb_value) / (1024.0 * 1024.0), 3)
    except (TypeError, ValueError):
        return 0.0


def safe_division(numerator: Any, denominator: Any) -> float:
    """División segura que retorna 0.0 en caso de error o denominador cero."""
    try:
        num = float(numerator)
        den = float(denominator)
        if den == 0:
            return 0.0
        return round((num / den) * 100.0, 2)
    except (TypeError, ValueError, ZeroDivisionError):
        return 0.0


def format_uptime(ticks: Any) -> Dict[str, Any]:
    """
    Convierte ticks SNMP (centésimas de segundo) a diccionario de uptime.
    
    Args:
        ticks: Valor de sysUpTime en centésimas de segundo (TimeTicks).
        
    Returns:
        Diccionario con total_seconds, days, hours, minutes, seconds y formatted.
    """
    try:
        total_seconds = int(ticks) / 100
        days = int(total_seconds // 86400)
        hours = int((total_seconds % 86400) // 3600)
        minutes = int((total_seconds % 3600) // 60)
        seconds = int(total_seconds % 60)
        return {
            "total_seconds": int(total_seconds),
            "days": days,
            "hours": hours,
            "minutes": minutes,
            "seconds": seconds,
            "formatted": f"{days}d {hours}h {minutes}m {seconds}s"
        }
    except (TypeError, ValueError):
        return {
            "total_seconds": None,
            "days": None,
            "hours": None,
            "minutes": None,
            "seconds": None,
            "formatted": str(ticks) if ticks is not None else "N/A"
        }


def calculate_percentage(part: Any, total: Any) -> float:
    """Calcula porcentaje de forma segura. Retorna 0.0 si no es posible."""
    try:
        p = float(part)
        t = float(total)
        if t == 0:
            return 0.0
        return round((p / t) * 100.0, 2)
    except (TypeError, ValueError, ZeroDivisionError):
        return 0.0


def safe_int(value: Any, default: int = 0) -> int:
    """Convierte un valor a entero de forma segura."""
    if value is None:
        return default
    try:
        return int(value)
    except (TypeError, ValueError):
        try:
            return int(float(value))
        except (TypeError, ValueError):
            return default


def safe_float(value: Any, default: float = 0.0) -> float:
    """Convierte un valor a float de forma segura."""
    if value is None:
        return default
    try:
        return float(value)
    except (TypeError, ValueError):
        return default
