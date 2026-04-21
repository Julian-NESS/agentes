# ==============================================================================
# NESS Relay v2.0.0 - Access Point Device Type
# ==============================================================================
# Stub para Phase 2+ : Tipo de dispositivo base para access points.
# ==============================================================================

from typing import Dict


class AccessPointMixin:
    """
    Mixin para access points. Define características comunes a todos los APs.
    Implementación completa en Phase 2.
    """
    
    device_type: str = "access_point"
    
    AP_METRIC_CATEGORIES = [
        'wireless_clients',
        'channel_utilization',
        'signal_strength',
        'ssid_stats',
    ]
    
    @staticmethod
    def get_ap_health_indicators() -> Dict[str, str]:
        return {
            'connected_clients': 'Número de clientes wireless conectados',
            'channel_utilization': 'Porcentaje de utilización del canal',
            'noise_floor': 'Nivel de ruido del canal',
        }
