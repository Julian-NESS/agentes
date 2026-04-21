# ==============================================================================
# NESS Relay v2.0.0 - Vendor Collector
# ==============================================================================
# Collector genérico que delega al perfil del vendor para recolectar
# datos específicos del fabricante (ej: PF-MIB para pfSense,
# CISCO-PROCESS-MIB para Cisco, etc.).
# ==============================================================================

import logging
from typing import Any, Dict

from core.snmp_client import SnmpClient
from profiles.base_profile import BaseDeviceProfile

logger = logging.getLogger("ness_relay")


async def collect_vendor_specific_data(
    client: SnmpClient,
    profile: BaseDeviceProfile,
) -> Dict[str, Any]:
    """
    Recolecta datos específicos del vendor delegando al perfil.
    
    Cada perfil de vendor implementa `collect_vendor_specific_data()`
    con la lógica particular de sus MIBs propietarias.
    
    Args:
        client: Instancia de SnmpClient conectada al dispositivo.
        profile: Perfil del vendor con la implementación específica.
        
    Returns:
        Diccionario con datos vendor-specific.
    """
    logger.info(f"Recolectando datos específicos de {profile.vendor_display_name}...")
    
    try:
        vendor_data = await profile.collect_vendor_specific_data(client)
        logger.info(
            f"Datos específicos de {profile.vendor_display_name} "
            f"recolectados exitosamente"
        )
        return vendor_data
    except Exception as e:
        logger.error(
            f"Error recolectando datos de {profile.vendor_display_name}: {e}"
        )
        return {
            "error": str(e),
            "vendor": profile.vendor,
        }
