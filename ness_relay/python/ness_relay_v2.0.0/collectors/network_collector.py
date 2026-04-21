# ==============================================================================
# NESS Relay v2.0.0 - Network Collector
# ==============================================================================
# Recolecta datos de interfaces de red: tráfico, errores, estado.
# Usa OIDs estándar IF-MIB (RFC 2863) que son universales.
# ==============================================================================

import logging
from typing import Any, Dict

from core.snmp_client import SnmpClient
from profiles.standard_oids import HC_INTERFACE_OIDS, INTERFACE_OIDS
from utils.conversions import safe_int
from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


async def collect_network_data(client: SnmpClient) -> Dict[str, Any]:
    """
    Recolecta datos de interfaces de red via IF-MIB (RFC 2863).
    
    Para cada interfaz obtiene: nombre, estado admin/operativo, velocidad,
    tráfico (in/out), errores y descartados. Usa contadores HC (64-bit)
    cuando están disponibles.
    
    Args:
        client: Instancia de SnmpClient conectada al dispositivo.
        
    Returns:
        Diccionario con datos de todas las interfaces.
    """
    logger.info("Iniciando recolección de datos de red...")
    
    network_data: Dict[str, Any] = {
        "interfaces": {},
        "collection_timestamp": now_iso()
    }
    
    # Combinar OIDs de interfaz estándar + high-capacity
    all_if_oids = {**INTERFACE_OIDS, **HC_INTERFACE_OIDS}
    
    interface_oid_names = [
        'if_descr', 'if_admin_status', 'if_oper_status', 'if_speed',
        'if_high_speed', 'if_in_octets', 'if_out_octets',
        'if_hc_in_octets', 'if_hc_out_octets', 'if_in_errors',
        'if_out_errors', 'if_in_discards', 'if_out_discards'
    ]
    
    interface_data: Dict[str, Dict[str, Any]] = {}
    
    for oid_name in interface_oid_names:
        oid = all_if_oids.get(oid_name)
        if not oid:
            continue
        
        results, error = await client.bulk(oid)
        if not error and results:
            for oid_result, value in results:
                idx = oid_result.split('.')[-1]
                interface_data.setdefault(idx, {})[oid_name] = value
        elif error:
            logger.debug(f"Network OID {oid_name}: {error}")
    
    # Procesar cada interfaz y normalizar datos
    for idx, data in interface_data.items():
        # Tráfico: preferir contadores HC (64-bit) sobre estándar (32-bit)
        in_octets = safe_int(data.get('if_hc_in_octets')) or safe_int(data.get('if_in_octets'))
        out_octets = safe_int(data.get('if_hc_out_octets')) or safe_int(data.get('if_out_octets'))
        
        # Velocidad: preferir ifHighSpeed (Mbps) sobre ifSpeed (bps)
        if safe_int(data.get('if_high_speed')):
            speed_mbps = safe_int(data.get('if_high_speed'))
        else:
            speed_bps = safe_int(data.get('if_speed'))
            speed_mbps = round(speed_bps / 1_000_000, 2) if speed_bps else 0
        
        errors_in = safe_int(data.get('if_in_errors'))
        errors_out = safe_int(data.get('if_out_errors'))
        total_errors = errors_in + errors_out
        
        processed_data = {
            "index": idx,
            "name": str(data.get('if_descr', 'Unknown')),
            "admin_status": "UP" if str(data.get('if_admin_status')) == '1' else "DOWN",
            "operational_status": "UP" if str(data.get('if_oper_status')) == '1' else "DOWN",
            "speed_mbps": speed_mbps,
            "traffic_in_mb": round(in_octets / (1024.0 * 1024.0), 2),
            "traffic_out_mb": round(out_octets / (1024.0 * 1024.0), 2),
            "errors_in": errors_in,
            "errors_out": errors_out,
            "total_errors": total_errors,
            "discards_in": safe_int(data.get('if_in_discards')),
            "discards_out": safe_int(data.get('if_out_discards')),
        }
        
        network_data["interfaces"][idx] = processed_data
    
    logger.info(
        f"Datos de red recolectados exitosamente - "
        f"{len(network_data['interfaces'])} interfaces"
    )
    return network_data
