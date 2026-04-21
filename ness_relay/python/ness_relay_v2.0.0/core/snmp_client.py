# ==============================================================================
# NESS Relay v2.0.0 - SNMP Client
# ==============================================================================
# Cliente SNMP encapsulado por dispositivo. Soporta SNMPv1, SNMPv2c y SNMPv3.
# Cada instancia representa una conexión a un dispositivo específico.
# Usa pysnmp v7.x (hlapi.v3arch.asyncio) — requiere factory async .create().
# ==============================================================================

import logging
from typing import Any, Dict, List, Optional, Tuple

from pysnmp.hlapi.v3arch.asyncio import (
    CommunityData,
    ContextData,
    ObjectIdentity,
    ObjectType,
    SnmpEngine,
    UdpTransportTarget,
    UsmUserData,
    bulk_cmd,
    get_cmd,
    next_cmd,
)

from utils.helpers import SnmpResult

logger = logging.getLogger("ness_relay")

# ==============================================================================
# MAPEO DE PROTOCOLOS SNMPv3
# ==============================================================================

try:
    from pysnmp.hlapi.v3arch.asyncio import (
        usmHMACSHAAuthProtocol,
        usmHMACMD5AuthProtocol,
        usmAesCfb128Protocol,
        usmDESPrivProtocol,
        usmNoAuthProtocol,
        usmNoPrivProtocol,
    )
    
    AUTH_PROTOCOLS = {
        'SHA': usmHMACSHAAuthProtocol,
        'MD5': usmHMACMD5AuthProtocol,
        'NONE': usmNoAuthProtocol,
    }
    
    PRIV_PROTOCOLS = {
        'AES': usmAesCfb128Protocol,
        'AES128': usmAesCfb128Protocol,
        'DES': usmDESPrivProtocol,
        'NONE': usmNoPrivProtocol,
    }
except ImportError:
    AUTH_PROTOCOLS = {}
    PRIV_PROTOCOLS = {}
    logger.warning("Protocolos SNMPv3 no disponibles (Cryptodome no encontrado)")


# ==============================================================================
# SNMP CLIENT
# ==============================================================================

class SnmpClient:
    """
    Cliente SNMP encapsulado para un dispositivo específico.
    
    Soporta SNMPv1, SNMPv2c y SNMPv3 con autenticación y cifrado.
    
    IMPORTANTE: En pysnmp v7.x, UdpTransportTarget requiere construcción
    async via .create(). Por eso este cliente usa un factory classmethod:
    
        client = await SnmpClient.create(device_config)
    
    NO usar el constructor directamente.
    """
    
    def __init__(self, device_config: Dict[str, Any]):
        """
        Inicializa los atributos del cliente (sin crear el transport).
        
        Para crear un cliente completo, usar:
            client = await SnmpClient.create(device_config)
        """
        self.host: str = device_config['ip']
        self.port: int = device_config.get('port', 161)
        self.snmp_version: str = device_config.get('snmp_version', '2c')
        self.vendor: str = device_config.get('vendor', 'generic')
        self.description: str = device_config.get('description', self.host)
        
        # SNMPv2c
        self.community: str = device_config.get('community', 'public')
        
        # SNMPv3
        self.v3_user: str = device_config.get('v3_user', '')
        self.v3_auth_protocol: str = device_config.get('v3_auth_protocol', 'SHA')
        self.v3_auth_password: str = device_config.get('v3_auth_password', '')
        self.v3_priv_protocol: str = device_config.get('v3_priv_protocol', 'AES128')
        self.v3_priv_password: str = device_config.get('v3_priv_password', '')
        
        # Motor SNMP
        self._engine = SnmpEngine()
        
        # Se construyen en create()
        self._auth_data = None
        self._transport = None
    
    @classmethod
    async def create(cls, device_config: Dict[str, Any]) -> 'SnmpClient':
        """
        Factory async para crear un SnmpClient con transport inicializado.
        
        En pysnmp v7.x, UdpTransportTarget.create() es async, por lo que
        no se puede construir en __init__.
        
        Args:
            device_config: Configuración del dispositivo.
            
        Returns:
            Instancia de SnmpClient lista para usar.
        """
        instance = cls(device_config)
        instance._auth_data = instance._build_auth_data()
        instance._transport = await UdpTransportTarget.create(
            (instance.host, instance.port)
        )
        return instance
    
    def _build_auth_data(self):
        """Construye el objeto de autenticación según la versión SNMP."""
        if self.snmp_version == '3':
            auth_proto = AUTH_PROTOCOLS.get(self.v3_auth_protocol.upper())
            priv_proto = PRIV_PROTOCOLS.get(self.v3_priv_protocol.upper())
            
            if not auth_proto:
                logger.warning(
                    f"Protocolo de autenticación '{self.v3_auth_protocol}' no reconocido. "
                    f"Disponibles: {list(AUTH_PROTOCOLS.keys())}"
                )
            if not priv_proto:
                logger.warning(
                    f"Protocolo de privacidad '{self.v3_priv_protocol}' no reconocido. "
                    f"Disponibles: {list(PRIV_PROTOCOLS.keys())}"
                )
            
            kwargs = {'userName': self.v3_user}
            if self.v3_auth_password:
                kwargs['authKey'] = self.v3_auth_password
            if auth_proto:
                kwargs['authProtocol'] = auth_proto
            if self.v3_priv_password:
                kwargs['privKey'] = self.v3_priv_password
            if priv_proto:
                kwargs['privProtocol'] = priv_proto
            
            return UsmUserData(**kwargs)
        
        elif self.snmp_version == '1':
            return CommunityData(self.community, mpModel=0)
        
        else:
            # SNMPv2c (default)
            return CommunityData(self.community, mpModel=1)
    
    async def get(self, oid: str) -> SnmpResult:
        """
        Realiza una operación SNMP GET para un OID específico.
        
        Args:
            oid: OID numérico (ej: '1.3.6.1.2.1.1.5.0')
            
        Returns:
            SnmpResult con el valor obtenido o el error.
        """
        try:
            error_indication, error_status, error_index, var_binds = await get_cmd(
                self._engine,
                self._auth_data,
                self._transport,
                ContextData(),
                ObjectType(ObjectIdentity(oid)),
            )
            
            if error_indication:
                return SnmpResult(error=str(error_indication), oid=oid)
            if error_status:
                error_msg = (
                    f"{error_status.prettyPrint()} at "
                    f"{error_index and var_binds[int(error_index) - 1][0] or '?'}"
                )
                return SnmpResult(error=error_msg, oid=oid)
            
            if var_binds:
                name, value = var_binds[0]
                value_str = str(value) if value is not None else None
                
                # Verificar si es un valor SNMP "no such instance" o similar
                if value_str and ('noSuch' in value_str or 'No Such' in value_str):
                    return SnmpResult(error=f"OID no soportado: {value_str}", oid=oid)
                
                return SnmpResult(value=value, oid=oid)
            
            return SnmpResult(error="Sin datos en respuesta", oid=oid)
            
        except Exception as e:
            logger.error(f"Excepción en SNMP GET ({self.host}) OID {oid}: {e}")
            return SnmpResult(error=str(e), oid=oid)
    
    async def bulk(
        self,
        oid: str,
        max_repetitions: int = 25,
        non_repeaters: int = 0,
    ) -> Tuple[List[Tuple[str, Any]], Optional[str]]:
        """
        Realiza una operación SNMP BULK GET para obtener tablas.
        
        Para SNMPv1 (que no soporta GETBULK), se realiza automáticamente
        un walk via GETNEXT repetido para obtener el mismo resultado.
        
        Args:
            oid: OID base de la tabla (ej: '1.3.6.1.2.1.2.2.1.2')
            max_repetitions: Máximo de filas a obtener por solicitud.
            non_repeaters: Número de OIDs que no se repiten.
            
        Returns:
            Tupla (lista_de_resultados, error_string_o_None).
            Cada resultado es (oid_string, valor).
        """
        # SNMPv1 no soporta GETBULK — usar GETNEXT walk como fallback
        if self.snmp_version == '1':
            return await self._walk_via_getnext(oid, max_results=max_repetitions)
        
        results: List[Tuple[str, Any]] = []
        
        try:
            error_indication, error_status, error_index, var_binds = await bulk_cmd(
                self._engine,
                self._auth_data,
                self._transport,
                ContextData(),
                non_repeaters,
                max_repetitions,
                ObjectType(ObjectIdentity(oid)),
            )
            
            if error_indication:
                return [], str(error_indication)
            if error_status:
                error_msg = (
                    f"{error_status.prettyPrint()} at "
                    f"{error_index and var_binds[int(error_index) - 1][0] or '?'}"
                )
                return [], error_msg
            
            # Filtrar resultados que pertenecen al OID base solicitado
            base_oid = oid.rstrip('.')
            for var_bind in var_binds:
                name, value = var_bind
                oid_str = str(name)
                
                # Solo incluir OIDs que son hijos del OID base
                if oid_str.startswith(base_oid):
                    value_str = str(value) if value is not None else None
                    if value_str and ('noSuch' in value_str or 'endOfMib' in value_str.lower()):
                        continue
                    results.append((oid_str, value))
            
            return results, None
            
        except Exception as e:
            logger.error(f"Excepción en SNMP BULK ({self.host}) OID {oid}: {e}")
            return [], str(e)
    
    async def _walk_via_getnext(
        self,
        oid: str,
        max_results: int = 50,
    ) -> Tuple[List[Tuple[str, Any]], Optional[str]]:
        """
        Implementa un SNMP walk usando GETNEXT repetido.
        Usado como fallback para SNMPv1 (que no soporta GETBULK).
        
        Args:
            oid: OID base de la tabla.
            max_results: Máximo de entradas a recolectar (protección contra loops infinitos).
            
        Returns:
            Tupla (lista_de_resultados, error_string_o_None).
        """
        results: List[Tuple[str, Any]] = []
        base_oid = oid.rstrip('.')
        current_oid = oid
        
        try:
            for _ in range(max_results):
                error_indication, error_status, error_index, var_binds = await next_cmd(
                    self._engine,
                    self._auth_data,
                    self._transport,
                    ContextData(),
                    ObjectType(ObjectIdentity(current_oid)),
                )
                
                if error_indication:
                    if results:
                        # Ya tenemos datos, devolver lo que hay
                        return results, None
                    return [], str(error_indication)
                
                if error_status:
                    if results:
                        return results, None
                    try:
                        error_at = (
                            var_binds[int(error_index) - 1][0]
                            if var_binds and error_index
                            else '?'
                        )
                    except (IndexError, TypeError):
                        error_at = '?'
                    error_msg = f"{error_status.prettyPrint()} at {error_at}"
                    return [], error_msg
                
                if not var_binds:
                    break
                
                # Extraer nombre y valor del varbind de forma segura
                try:
                    var_bind = var_binds[0]
                    # Manejar formato tabla [[ObjectType]] vs plano [ObjectType]
                    if isinstance(var_bind, (list, tuple)) and len(var_bind) > 0:
                        if hasattr(var_bind[0], '__iter__') and not isinstance(var_bind[0], (str, bytes)):
                            var_bind = var_bind[0]
                    name = var_bind[0]
                    value = var_bind[1]
                except (IndexError, TypeError):
                    break
                oid_str = str(name)
                
                # Detener si salimos del subárbol OID base
                if not oid_str.startswith(base_oid):
                    break
                
                value_str = str(value) if value is not None else None
                if value_str and ('noSuch' in value_str or 'endOfMib' in value_str.lower()):
                    break
                
                results.append((oid_str, value))
                current_oid = oid_str  # Siguiente GETNEXT desde este OID
            
            return results, None
            
        except Exception as e:
            logger.error(f"Excepción en SNMP GETNEXT walk ({self.host}) OID {oid}: {e}")
            if results:
                return results, None
            return [], str(e)
    
    async def test_connectivity(self, test_oid: str = '1.3.6.1.2.1.1.5.0') -> SnmpResult:
        """
        Prueba de conectividad SNMP usando sysName como OID de prueba.
        
        Args:
            test_oid: OID a usar para la prueba (default: sysName).
            
        Returns:
            SnmpResult con el resultado de la prueba.
        """
        return await self.get(test_oid)
    
    def get_connection_info(self) -> Dict[str, Any]:
        """Retorna información de conexión para logging/display."""
        info = {
            'host': self.host,
            'port': self.port,
            'snmp_version': f'v{self.snmp_version}',
            'vendor': self.vendor,
            'description': self.description,
        }
        if self.snmp_version == '3':
            info['v3_user'] = self.v3_user
            info['v3_auth'] = self.v3_auth_protocol
            info['v3_priv'] = self.v3_priv_protocol
        else:
            info['community'] = self.community
        return info
    
    def __repr__(self) -> str:
        return (
            f"SnmpClient(host={self.host!r}, port={self.port}, "
            f"version={self.snmp_version!r}, vendor={self.vendor!r})"
        )
