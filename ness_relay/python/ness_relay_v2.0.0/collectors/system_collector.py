# ==============================================================================
# NESS Relay v2.0.0 - System Collector
# ==============================================================================
# Recolecta datos básicos del sistema: sysName, sysDescr, sysUpTime,
# sysLocation, sysContact. Estos OIDs son estándar RFC y aplican a
# todos los vendors por igual.
# ==============================================================================

import logging
from typing import Any, Dict

from core.snmp_client import SnmpClient
from profiles.standard_oids import SYSTEM_OIDS
from utils.conversions import format_uptime
from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


async def collect_system_data(client: SnmpClient) -> Dict[str, Any]:
    """
    Recolecta datos básicos del sistema via SNMP.
    
    Consulta los OIDs estándar de SNMPv2-MIB (RFC 1213):
    - sysName, sysDescr, sysLocation, sysContact, sysUpTime
    
    Args:
        client: Instancia de SnmpClient conectada al dispositivo.
        
    Returns:
        Diccionario con datos del sistema.
    """
    logger.info("Iniciando recolección de datos del sistema...")
    
    system_data: Dict[str, Any] = {
        "timestamp": now_iso(),
        "collection_time_utc": now_iso(utc=True),
    }
    
    basic_info: Dict[str, Any] = {}
    basic_oids = ['sys_name', 'sys_descr', 'sys_location', 'sys_contact', 'sys_uptime']
    
    for oid_name in basic_oids:
        oid = SYSTEM_OIDS.get(oid_name)
        if not oid:
            basic_info[oid_name] = {"error": f"OID '{oid_name}' no definido"}
            continue
        
        res = await client.get(oid)
        
        if res.error or res.value is None:
            basic_info[oid_name] = {"error": res.error}
            logger.warning(f"Error collecting {oid_name}: {res.error}")
        else:
            if oid_name == 'sys_uptime':
                basic_info[oid_name] = format_uptime(res.value)
            else:
                basic_info[oid_name] = str(res.value)
    
    system_data["basic_info"] = basic_info
    logger.info("Datos del sistema recolectados exitosamente")
    return system_data
