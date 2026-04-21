# ==============================================================================
# NESS Relay v2.0.0 - Performance Collector
# ==============================================================================
# Recolecta métricas de rendimiento: CPU, Memoria y Disco.
# Los OIDs y la normalización son delegados al perfil del vendor,
# ya que cada fabricante usa MIBs diferentes para estas métricas.
# ==============================================================================

import logging
from typing import Any, Dict

from core.snmp_client import SnmpClient
from profiles.base_profile import BaseDeviceProfile
from utils.conversions import safe_float, safe_int
from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


async def collect_performance_data(
    client: SnmpClient,
    profile: BaseDeviceProfile,
) -> Dict[str, Any]:
    """
    Recolecta datos de rendimiento (CPU, Memoria, Disco) usando el perfil del vendor.
    
    El perfil proporciona los OIDs correctos y la lógica de normalización
    específica del vendor. El collector es genérico.
    
    Args:
        client: Instancia de SnmpClient conectada al dispositivo.
        profile: Perfil del vendor para OIDs y normalización.
        
    Returns:
        Diccionario con datos de rendimiento normalizados.
    """
    logger.info("Iniciando recolección de datos de performance...")
    
    performance_data: Dict[str, Any] = {
        "cpu": {},
        "memory": {},
        "disk": {},
        "collection_timestamp": now_iso()
    }
    
    # ===== CPU =====
    cpu_oids = profile.get_cpu_oids()
    cpu_raw: Dict[str, Any] = {}
    
    for oid_name, oid in cpu_oids.items():
        res = await client.get(oid)
        if not res.error and res.value is not None:
            cpu_raw[oid_name] = res.value
        else:
            cpu_raw[oid_name] = None
            if res.error:
                logger.debug(f"CPU OID {oid_name}: {res.error}")
    
    # Delegar normalización al perfil del vendor
    performance_data["cpu"] = profile.normalize_cpu_data(cpu_raw)
    
    # ===== MEMORY =====
    mem_oids = profile.get_memory_oids()
    mem_raw: Dict[str, Any] = {}
    
    for oid_name, oid in mem_oids.items():
        res = await client.get(oid)
        if not res.error and res.value is not None:
            mem_raw[oid_name] = res.value
        else:
            mem_raw[oid_name] = None
            if res.error:
                logger.debug(f"Memory OID {oid_name}: {res.error}")
    
    # Delegar normalización al perfil del vendor
    performance_data["memory"] = profile.normalize_memory_data(mem_raw)
    
    # ===== DISK =====
    disk_oids = profile.get_disk_oids()
    disk_raw: Dict[str, Dict[str, Any]] = {}
    
    # Los OIDs de disco son tablas SNMP -> usar bulk
    for oid_name, oid in disk_oids.items():
        results, error = await client.bulk(oid)
        if not error and results:
            for oid_result, value in results:
                idx = oid_result.split('.')[-1]
                disk_raw.setdefault(idx, {})[oid_name] = value
        elif error:
            logger.debug(f"Disk OID {oid_name}: {error}")
    
    # Delegar normalización al perfil del vendor
    performance_data["disk"] = profile.normalize_disk_data(disk_raw)
    
    # Post-procesamiento: permite al perfil corregir/enriquecer datos
    # (ej: MikroTik extrae memoria desde hrStorageTable "main memory")
    performance_data = profile.post_process_performance(performance_data)
    
    logger.info("Datos de performance recolectados exitosamente")
    return performance_data
