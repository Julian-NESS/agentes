# ==============================================================================
# NESS Relay v2.0.0 - Switch Device Type
# ==============================================================================
# Stub para Phase 2+ : Tipo de dispositivo base para switches.
# ==============================================================================

from typing import Dict


class SwitchMixin:
    """
    Mixin para switches. Define características comunes a todos los switches.
    Implementación completa en Phase 2.
    """
    
    device_type: str = "switch"
    
    SWITCH_METRIC_CATEGORIES = [
        'vlan_table',
        'mac_address_table',
        'spanning_tree',
        'port_security',
    ]
    
    @staticmethod
    def get_switch_health_indicators() -> Dict[str, str]:
        return {
            'mac_table_size': 'Número de MACs en la tabla',
            'stp_changes': 'Cambios de topología STP',
            'port_errors': 'Errores en puertos del switch',
        }
