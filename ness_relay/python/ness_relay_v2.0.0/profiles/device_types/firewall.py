# ==============================================================================
# NESS Relay v2.0.0 - Firewall Device Type
# ==============================================================================
# Tipo de dispositivo base para firewalls. Define características comunes
# a todos los firewalls (pfSense, Fortinet FortiGate, Cisco ASA, etc.)
# ==============================================================================

from typing import Dict


class FirewallMixin:
    """
    Mixin que define características comunes de firewalls.
    
    Proporciona métodos auxiliares y constantes compartidas entre
    todos los perfiles de firewall, independientemente del vendor.
    """
    
    device_type: str = "firewall"
    
    # Categorías estándar de métricas de firewall
    FIREWALL_METRIC_CATEGORIES = [
        'connection_states',    # Tabla de estados (conexiones activas)
        'packet_filtering',     # Reglas de filtrado
        'nat_translations',     # Traducciones NAT
        'vpn_tunnels',          # Túneles VPN
        'threat_detection',     # Detección de amenazas
    ]
    
    @staticmethod
    def classify_connection_count(count: int) -> str:
        """
        Clasifica la cantidad de conexiones activas del firewall.
        
        Args:
            count: Número de conexiones activas.
            
        Returns:
            Nivel: 'normal', 'elevated', 'high', 'critical'.
        """
        if count > 100000:
            return 'critical'
        elif count > 50000:
            return 'high'
        elif count > 10000:
            return 'elevated'
        return 'normal'
    
    @staticmethod
    def get_firewall_health_indicators() -> Dict[str, str]:
        """Retorna los indicadores estándar de salud de un firewall."""
        return {
            'connection_states': 'Conexiones activas en tabla de estados',
            'packet_drops': 'Paquetes descartados por reglas',
            'throughput': 'Rendimiento de tráfico a través del firewall',
            'cpu_firewall': 'Uso de CPU por procesos de firewall',
            'memory_states': 'Memoria usada por tabla de estados',
        }
