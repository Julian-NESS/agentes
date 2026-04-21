# ==============================================================================
# NESS Relay v2.0.0 - Helper Utilities
# ==============================================================================
# Estructuras de datos comunes, funciones auxiliares de tiempo y logging seguro.
# ==============================================================================

import logging
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any, Optional

logger = logging.getLogger("ness_relay")


@dataclass
class SnmpResult:
    """Resultado de una operación SNMP individual."""
    value: Any = None
    error: Optional[str] = None
    oid: Optional[str] = None

    @property
    def success(self) -> bool:
        return self.error is None and self.value is not None


def now_iso(utc: bool = False) -> str:
    """
    Retorna timestamp ISO 8601 actual, siempre con zona horaria.
    
    Args:
        utc: Si True, retorna en UTC. Si False, retorna hora local con offset UTC.
    
    Returns:
        String ISO 8601 con timezone (ej: '2026-02-25T11:05:02.900269+00:00').
        Django requiere datetimes "aware" cuando USE_TZ=True.
    """
    if utc:
        return datetime.now(timezone.utc).isoformat()
    # Usar hora local PERO con zona horaria incluida (aware)
    # astimezone() sin argumentos convierte a la zona horaria local del sistema
    return datetime.now(timezone.utc).astimezone().isoformat()


def print_simple(msg: str) -> None:
    """Print seguro que no falla ante errores de encoding."""
    try:
        print(msg, flush=True)
    except Exception:
        try:
            print(msg.encode('ascii', errors='replace').decode('ascii'), flush=True)
        except Exception:
            pass


def safe_log(level: str, msg: str) -> None:
    """Logging seguro que no propaga excepciones."""
    try:
        log_func = getattr(logger, level.lower(), logger.info)
        log_func(msg)
    except Exception:
        pass
