# ==============================================================================
# NESS Relay v2.0.0 - Base Device Profile (ABC)
# ==============================================================================
# Clase abstracta que define la interfaz obligatoria para todos los perfiles
# de dispositivos. Cada vendor (pfSense, Cisco, Fortinet, MikroTik) debe
# implementar esta interfaz.
#
# El Engine es vendor-agnostic: le pregunta al Profile qué OIDs consultar,
# usa el SnmpClient para obtener datos crudos, y le pide al Profile que
# normalice los resultados.

# ==============================================================================

from abc import ABC, abstractmethod
from typing import Any, Dict, List, Optional

from profiles.standard_oids import STANDARD_OIDS


class BaseDeviceProfile(ABC):
    """
    Interfaz base para todos los perfiles de dispositivos.
    
    Cada vendor debe implementar esta clase para definir:
    - Qué OIDs específicos del vendor se consultan
    - Cómo se normalizan los datos de CPU, memoria, disco
    - Qué datos vendor-specific se recolectan
    
    Los OIDs estándar RFC (interfaces, TCP, UDP, IP, ICMP, SNMP stats)
    son comunes a todos los vendors y se heredan automáticamente.
    """
    
    # Metadatos del perfil (deben ser sobreescritos por cada vendor)
    vendor: str = "generic"
    vendor_display_name: str = "Generic SNMP Device"
    device_type: str = "unknown"  # firewall, router, switch, access_point
    
    def __init__(self):
        """Inicializa el perfil base con OIDs estándar."""
        self._standard_oids = STANDARD_OIDS.copy()
    
    # ==========================================================================
    # OIDs
    # ==========================================================================
    
    def get_all_oids(self) -> Dict[str, str]:
        """
        Retorna TODOS los OIDs que este perfil puede consultar
        (estándar RFC + vendor-specific).
        
        Returns:
            Diccionario completo de OIDs {nombre: oid_string}.
        """
        all_oids = self._standard_oids.copy()
        all_oids.update(self.get_vendor_oids())
        all_oids.update(self.get_cpu_oids())
        all_oids.update(self.get_memory_oids())
        all_oids.update(self.get_disk_oids())
        return all_oids
    
    def get_standard_oids(self) -> Dict[str, str]:
        """Retorna solo los OIDs estándar RFC (comunes a todos)."""
        return self._standard_oids.copy()
    
    @abstractmethod
    def get_vendor_oids(self) -> Dict[str, str]:
        """
        Retorna OIDs específicos del vendor que no son estándar RFC.
        
        Ejemplo para pfSense: OIDs de PF-MIB (estados de firewall, logs).
        Ejemplo para Cisco: OIDs de CISCO-PROCESS-MIB.
        
        Returns:
            Diccionario con OIDs vendor-specific {nombre: oid_string}.
        """
        ...
    
    @abstractmethod
    def get_cpu_oids(self) -> Dict[str, str]:
        """
        Retorna OIDs para monitoreo de CPU.
        
        Varían por vendor:
        - pfSense/Linux: UCD-SNMP-MIB (laLoad, ssCpu*)
        - Cisco: CISCO-PROCESS-MIB (cpmCPUTotal*)
        - Fortinet: FORTINET-FORTIGATE-MIB (fgSysCpuUsage)
        - MikroTik: MIKROTIK-MIB (mtxrHlProcessorTemperature)
        
        Returns:
            Diccionario con OIDs de CPU {nombre: oid_string}.
        """
        ...
    
    @abstractmethod
    def get_memory_oids(self) -> Dict[str, str]:
        """
        Retorna OIDs para monitoreo de memoria.
        
        Varían por vendor según la MIB utilizada.
        
        Returns:
            Diccionario con OIDs de memoria {nombre: oid_string}.
        """
        ...
    
    @abstractmethod
    def get_disk_oids(self) -> Dict[str, str]:
        """
        Retorna OIDs para monitoreo de disco/almacenamiento.
        
        Varían por vendor. Muchos usan HOST-RESOURCES-MIB o UCD-SNMP-MIB.
        
        Returns:
            Diccionario con OIDs de disco {nombre: oid_string}.
        """
        ...
    
    # ==========================================================================
    # NORMALIZACIÓN DE DATOS
    # ==========================================================================
    
    @abstractmethod
    def normalize_cpu_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos crudos de CPU al formato estándar NESS.
        
        El formato de salida debe ser consistente independiente del vendor:
        {
            "cpu_usage_percent": float,
            "load_1min": float,
            "load_5min": float,
            "load_15min": float,
        }
        
        Args:
            raw_data: Datos crudos obtenidos por SNMP.
            
        Returns:
            Datos normalizados de CPU.
        """
        ...
    
    @abstractmethod
    def normalize_memory_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos crudos de memoria al formato estándar NESS.
        
        El formato de salida debe ser:
        {
            "mem_usage_percent": float,
            "mem_total_mb": float,
            "mem_used_mb": float,
            "mem_free_mb": float,
            ...
        }
        
        Args:
            raw_data: Datos crudos obtenidos por SNMP.
            
        Returns:
            Datos normalizados de memoria.
        """
        ...
    
    @abstractmethod
    def normalize_disk_data(self, raw_disk_entries: Dict[str, Dict[str, Any]]) -> Dict[str, Dict[str, Any]]:
        """
        Normaliza datos crudos de disco al formato estándar NESS.
        
        Args:
            raw_disk_entries: Diccionario indexado por idx con datos crudos de disco.
            
        Returns:
            Diccionario normalizado de discos.
        """
        ...
    
    # ==========================================================================
    # DATOS VENDOR-SPECIFIC
    # ==========================================================================
    
    @abstractmethod
    async def collect_vendor_specific_data(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta datos específicos del vendor que no se cubren
        por los collectors estándar.
        
        Ejemplo para pfSense: estados del firewall, logs de PF.
        Ejemplo para Cisco: sesiones VPN, estado del stack.
        
        Args:
            client: Instancia de SnmpClient conectada al dispositivo.
            
        Returns:
            Diccionario con datos vendor-specific.
        """
        ...
    
    # ==========================================================================
    # POST-PROCESAMIENTO DE PERFORMANCE
    # ==========================================================================
    
    def post_process_performance(self, performance_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Post-procesamiento opcional de datos de rendimiento.
        
        Permite a perfiles específicos corregir o enriquecer los datos
        después de la recolección estándar. Por ejemplo, MikroTik extrae
        memoria desde hrStorageTable (disk) porque no tiene MIB de memoria.
        
        Args:
            performance_data: Datos de rendimiento ya normalizados.
            
        Returns:
            Datos de rendimiento post-procesados.
        """
        return performance_data
    
    def finalize_collected_data(self, all_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Post-procesamiento final DESPUÉS de que todos los collectors terminen.
        
        Se ejecuta tras recolectar system, performance, network, security
        y vendor_specific. Permite al perfil usar datos vendor-specific
        para enriquecer/corregir datos de performance u otras secciones.
        
        Ejemplo: MikroTik usa cpu_detailed (vendor) para corregir CPU
        cuando hrProcessorLoad (table OID via GET) retorna 0.
        
        Args:
            all_data: Diccionario completo con todos los datos recolectados.
            
        Returns:
            all_data modificado.
        """
        return all_data
    
    # ==========================================================================
    # DETECCIÓN AUTOMÁTICA
    # ==========================================================================
    
    @classmethod
    def matches_sys_object_id(cls, sys_object_id: str) -> bool:
        """
        Verifica si un sysObjectID corresponde a este vendor.
        
        Usado para auto-detección cuando el campo 'vendor' no está
        configurado explícitamente en devices.conf.
        
        Args:
            sys_object_id: sysObjectID obtenido del dispositivo.
            
        Returns:
            True si el OID corresponde a este vendor.
        """
        return False
    
    # ==========================================================================
    # UTILIDADES
    # ==========================================================================
    
    def get_profile_info(self) -> Dict[str, str]:
        """Retorna información del perfil para logging/metadata."""
        return {
            'vendor': self.vendor,
            'vendor_display_name': self.vendor_display_name,
            'device_type': self.device_type,
            'total_oids': str(len(self.get_all_oids())),
            'vendor_oids': str(len(self.get_vendor_oids())),
        }
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(vendor={self.vendor!r}, type={self.device_type!r})"
