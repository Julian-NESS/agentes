# ==============================================================================
# NESS Relay v2.0.0 - Router Device Type
# ==============================================================================
# Stub para Phase 2+ : Tipo de dispositivo base para routers.
# ==============================================================================

from typing import Dict


class RouterMixin:
    """
    Mixin para routers. Define características comunes a todos los routers.
    Implementación completa en Phase 2.
    """
    
    device_type: str = "router"
    
    ROUTER_METRIC_CATEGORIES = [
        'routing_table',
        'bgp_neighbors',
        'ospf_neighbors',
        'interface_routing',
    ]
    
    @staticmethod
    def get_router_health_indicators() -> Dict[str, str]:
        return {
            'routing_table_size': 'Número de rutas en la tabla de routing',
            'bgp_peers': 'Estado de peers BGP',
            'ospf_adjacencies': 'Estado de adyacencias OSPF',
        }
