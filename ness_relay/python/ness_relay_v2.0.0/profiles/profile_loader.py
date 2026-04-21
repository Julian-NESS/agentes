# ==============================================================================
# NESS Relay v2.0.0 - Profile Loader (Registry Pattern)
# ==============================================================================
# Registro centralizado de perfiles de vendor. Carga y gestiona los perfiles
# disponibles usando un patrón Registry.
#
# Uso:
#   profile = ProfileLoader.get_profile('pfsense')
#   profile = ProfileLoader.auto_detect(sys_object_id)
# ==============================================================================

import logging
from typing import Dict, Optional, Type

from profiles.base_profile import BaseDeviceProfile

logger = logging.getLogger("ness_relay")


class ProfileLoader:
    """
    Registry de perfiles de dispositivos.
    
    Cada perfil de vendor se registra aquí y puede ser obtenido
    por nombre de vendor o por auto-detección via sysObjectID.
    """
    
    # Registro de perfiles: vendor_name -> ProfileClass
    _registry: Dict[str, Type[BaseDeviceProfile]] = {}
    
    # Cache de instancias (singleton por vendor)
    _instances: Dict[str, BaseDeviceProfile] = {}
    
    @classmethod
    def register(cls, vendor_name: str, profile_class: Type[BaseDeviceProfile]) -> None:
        """
        Registra un perfil de vendor en el registry.
        
        Args:
            vendor_name: Nombre del vendor (ej: 'pfsense', 'cisco').
            profile_class: Clase que implementa BaseDeviceProfile.
        """
        vendor_key = vendor_name.lower().strip()
        
        if not issubclass(profile_class, BaseDeviceProfile):
            raise TypeError(
                f"'{profile_class.__name__}' debe ser subclase de BaseDeviceProfile"
            )
        
        cls._registry[vendor_key] = profile_class
        logger.debug(f"Perfil registrado: {vendor_key} -> {profile_class.__name__}")
    
    @classmethod
    def get_profile(cls, vendor_name: str) -> BaseDeviceProfile:
        """
        Obtiene una instancia del perfil para un vendor específico.
        
        Args:
            vendor_name: Nombre del vendor (ej: 'pfsense', 'cisco').
            
        Returns:
            Instancia del perfil del vendor.
            
        Raises:
            ValueError: Si el vendor no tiene perfil registrado.
        """
        vendor_key = vendor_name.lower().strip()
        
        # Retornar instancia cacheada si existe
        if vendor_key in cls._instances:
            return cls._instances[vendor_key]
        
        # Buscar en el registry
        profile_class = cls._registry.get(vendor_key)
        
        if profile_class is None:
            available = list(cls._registry.keys())
            raise ValueError(
                f"No hay perfil registrado para vendor '{vendor_name}'. "
                f"Vendors disponibles: {available}"
            )
        
        # Crear y cachear instancia
        instance = profile_class()
        cls._instances[vendor_key] = instance
        logger.info(f"Perfil cargado: {instance.vendor_display_name} ({vendor_key})")
        return instance
    
    @classmethod
    def auto_detect(cls, sys_object_id: str) -> Optional[BaseDeviceProfile]:
        """
        Intenta detectar automáticamente el vendor basándose en sysObjectID.
        
        Args:
            sys_object_id: OID del sistema obtenido via SNMP.
            
        Returns:
            Instancia del perfil detectado, o None si no se reconoce.
        """
        for vendor_key, profile_class in cls._registry.items():
            if profile_class.matches_sys_object_id(sys_object_id):
                logger.info(f"Auto-detectado vendor '{vendor_key}' via sysObjectID")
                return cls.get_profile(vendor_key)
        
        logger.warning(f"sysObjectID '{sys_object_id}' no corresponde a ningún vendor conocido")
        return None
    
    @classmethod
    def list_vendors(cls) -> list:
        """Retorna lista de vendors registrados."""
        return list(cls._registry.keys())
    
    @classmethod
    def is_registered(cls, vendor_name: str) -> bool:
        """Verifica si un vendor tiene perfil registrado."""
        return vendor_name.lower().strip() in cls._registry
    
    @classmethod
    def clear(cls) -> None:
        """Limpia el registry (útil para testing)."""
        cls._registry.clear()
        cls._instances.clear()


# ==============================================================================
# CARGA AUTOMÁTICA DE PERFILES
# ==============================================================================

def load_all_profiles() -> None:
    """
    Importa y registra todos los perfiles de vendor disponibles.
    
    Esta función se llama al inicio del relay para que todos los
    perfiles estén disponibles en el registry.
    """
    # Importar aquí para evitar imports circulares
    try:
        from profiles.vendors.pfsense import PfSenseProfile
        ProfileLoader.register('pfsense', PfSenseProfile)
    except ImportError as e:
        logger.warning(f"No se pudo cargar perfil pfSense: {e}")
    
    try:
        from profiles.vendors.fortinet import FortinetProfile
        ProfileLoader.register('fortinet', FortinetProfile)
    except ImportError as e:
        logger.warning(f"No se pudo cargar perfil Fortinet: {e}")
    
    try:
        from profiles.vendors.mikrotik import MikroTikProfile
        ProfileLoader.register('mikrotik', MikroTikProfile)
    except ImportError as e:
        logger.warning(f"No se pudo cargar perfil MikroTik RouterOS: {e}")
    
    # MikroTik Firewall (CHR/CCR/RB como gateway/perimetral)
    try:
        from profiles.vendors.mikrotik_fw import MikroTikFirewallProfile
        ProfileLoader.register('mikrotik_fw', MikroTikFirewallProfile)
    except ImportError as e:
        logger.warning(f"No se pudo cargar perfil MikroTik Firewall: {e}")
    
    # UBNT (Ubiquiti) - Switches
    try:
        from profiles.vendors.ubnt import UbiquitiProfile
        ProfileLoader.register('ubnt', UbiquitiProfile)
    except ImportError as e:
        logger.warning(f"No se pudo cargar perfil UBNT: {e}")
    
    # Cambium Networks - Access Points
    try:
        from profiles.vendors.c_n import CambiumProfile
        ProfileLoader.register('c_n', CambiumProfile)
    except ImportError as e:
        logger.warning(f"No se pudo cargar perfil Cambium: {e}")
    
    # Stubs para futuros vendors (Phase 3+)
    try:
        from profiles.vendors.cisco import CiscoProfile
        ProfileLoader.register('cisco', CiscoProfile)
    except ImportError:
        pass  # No implementado aún
    
    registered = ProfileLoader.list_vendors()
    logger.info(f"Perfiles cargados: {registered}")
