# ==============================================================================
# NESS Relay v2.0.0 - Ubiquiti (UBNT) Vendor Profile
# ==============================================================================
# Perfil completo para switches Ubiquiti (EdgeSwitch, UniFi Switch USW).
#
# CPU/Memory:    HOST-RESOURCES-MIB + UBNT-UniFi-MIB
# Puertos:       IF-MIB + BRIDGE-MIB
# PoE:           POWER-ETHERNET-MIB + UBNT-UniFi-MIB
# Temperatura:   UBNT-UniFi-MIB (si disponible)
# VLANs:         Q-BRIDGE-MIB
#
# Enterprise OID base: 1.3.6.1.4.1.41112 (Ubiquiti Networks)
# sysObjectID típico:  1.3.6.1.4.1.41112.*
# ==============================================================================

import logging
from typing import Any, Dict, List, Optional

from profiles.base_profile import BaseDeviceProfile
from utils.conversions import (
    calculate_percentage,
    safe_float,
    safe_int,
)
from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


# ==============================================================================
# OIDs ESPECÍFICOS DE UBIQUITI SWITCHES
# ==============================================================================

# CPU OIDs - HOST-RESOURCES-MIB (estándar, suelen implementarlo)
UBNT_CPU_OIDS: Dict[str, str] = {
    'hr_processor_load': '1.3.6.1.2.1.25.3.3.1.2',  # hrProcessorLoad (tabla, % por core)
}

# Memory OIDs - HOST-RESOURCES-MIB
UBNT_MEMORY_OIDS: Dict[str, str] = {
    'hr_storage_descr':      '1.3.6.1.2.1.25.2.3.1.3',   # hrStorageDescr
    'hr_storage_alloc':      '1.3.6.1.2.1.25.2.3.1.4',   # hrStorageAllocationUnits
    'hr_storage_size':       '1.3.6.1.2.1.25.2.3.1.5',   # hrStorageSize
    'hr_storage_used':       '1.3.6.1.2.1.25.2.3.1.6',   # hrStorageUsed
}

# Disk OIDs - HOST-RESOURCES-MIB (tabla hrStorageTable)
UBNT_DISK_OIDS: Dict[str, str] = {
    'hr_storage_descr':      '1.3.6.1.2.1.25.2.3.1.3',
    'hr_storage_alloc':      '1.3.6.1.2.1.25.2.3.1.4',
    'hr_storage_size':       '1.3.6.1.2.1.25.2.3.1.5',
    'hr_storage_used':       '1.3.6.1.2.1.25.2.3.1.6',
}

# IF-MIB para puertos
UBNT_INTERFACE_OIDS: Dict[str, str] = {
    'if_descr':           '1.3.6.1.2.1.2.2.1.2',           # ifDescr
    'if_type':            '1.3.6.1.2.1.2.2.1.3',           # ifType
    'if_speed':           '1.3.6.1.2.1.2.2.1.5',           # ifSpeed (bps)
    'if_admin_status':    '1.3.6.1.2.1.2.2.1.7',           # ifAdminStatus
    'if_oper_status':     '1.3.6.1.2.1.2.2.1.8',           # ifOperStatus
    'if_in_octets':       '1.3.6.1.2.1.2.2.1.10',          # ifInOctets
    'if_out_octets':      '1.3.6.1.2.1.2.2.1.16',          # ifOutOctets
    'if_in_errors':       '1.3.6.1.2.1.2.2.1.14',          # ifInErrors
    'if_out_errors':      '1.3.6.1.2.1.2.2.1.20',          # ifOutErrors
    'if_hc_in_octets':    '1.3.6.1.2.1.31.1.1.1.6',        # ifHCInOctets
    'if_hc_out_octets':   '1.3.6.1.2.1.31.1.1.1.10',       # ifHCOutOctets
    'if_high_speed':      '1.3.6.1.2.1.31.1.1.1.15',       # ifHighSpeed (Mbps)
    'if_name':            '1.3.6.1.2.1.31.1.1.1.1',        # ifName
    'if_alias':           '1.3.6.1.2.1.31.1.1.1.18',       # ifAlias
}

# POWER-ETHERNET-MIB (PoE) - Estándar IEEE 802.3af/at
POE_OIDS: Dict[str, str] = {
    # PSE (Power Sourcing Equipment) - El switch
    'pse_main_status':        '1.3.6.1.2.1.105.1.3.1.1.3',    # pethMainPseOperStatus
    'pse_power_consumption':  '1.3.6.1.2.1.105.1.3.1.1.4',    # pethMainPsePower (watts)
    'pse_power_available':    '1.3.6.1.2.1.105.1.3.1.1.2',    # pethMainPseAvailablePower
    # PSE por puerto
    'pse_port_admin':         '1.3.6.1.2.1.105.1.1.1.3',      # pethPsePortAdminEnable
    'pse_port_status':        '1.3.6.1.2.1.105.1.1.1.4',      # pethPsePortDetectionStatus
    'pse_port_power_class':   '1.3.6.1.2.1.105.1.1.1.7',      # pethPsePortPowerClass
    'pse_port_power':         '1.3.6.1.2.1.105.1.1.1.10',     # pethPsePortActualPower (mW)
}

# UBNT/UniFi específicos
UBNT_VENDOR_OIDS: Dict[str, str] = {
    # UniFi System Info
    'unifi_model':            '1.3.6.1.4.1.41112.1.6.3.2.0',  # Modelo
    'unifi_version':          '1.3.6.1.4.1.41112.1.6.3.6.0',  # Firmware version
    'unifi_mac':              '1.3.6.1.4.1.41112.1.6.3.5.0',  # MAC address
    'unifi_uptime':           '1.3.6.1.4.1.41112.1.6.3.4.0',  # Uptime
    
    # EdgeSwitch específicos (si aplica)
    'edge_cpu_util':          '1.3.6.1.4.1.4413.1.1.1.1.4.9.0',   # CPU utilization
    'edge_mem_total':         '1.3.6.1.4.1.4413.1.1.1.1.4.1.0',   # Total memory
    'edge_mem_free':          '1.3.6.1.4.1.4413.1.1.1.1.4.2.0',   # Free memory
    
    # Temperaturas UniFi (si soportado)
    'unifi_temp_board':       '1.3.6.1.4.1.41112.1.6.3.7.0',  # Board temperature
    'unifi_temp_cpu':         '1.3.6.1.4.1.41112.1.6.3.8.0',  # CPU temperature
    
    # Puertos UniFi
    'unifi_port_table':       '1.3.6.1.4.1.41112.1.6.2.1.1',  # Tabla de puertos
    
    # Agregar OIDs de interfaz estándar
    **UBNT_INTERFACE_OIDS,
    **POE_OIDS,
}

# Q-BRIDGE-MIB para VLANs
VLAN_OIDS: Dict[str, str] = {
    'dot1q_vlan_static_name':     '1.3.6.1.2.1.17.7.1.4.3.1.1',   # dot1qVlanStaticName
    'dot1q_vlan_static_ports':    '1.3.6.1.2.1.17.7.1.4.3.1.2',   # dot1qVlanStaticEgressPorts
    'dot1q_vlan_fdb_id':          '1.3.6.1.2.1.17.7.1.4.2.1.3',   # dot1qVlanFdbId
    'dot1q_pvid':                 '1.3.6.1.2.1.17.7.1.4.5.1.1',   # dot1qPvid (PVID por puerto)
}

# BRIDGE-MIB para tabla MAC
BRIDGE_OIDS: Dict[str, str] = {
    'dot1d_tp_fdb_address':   '1.3.6.1.2.1.17.4.3.1.1',   # dot1dTpFdbAddress (MAC)
    'dot1d_tp_fdb_port':      '1.3.6.1.2.1.17.4.3.1.2',   # dot1dTpFdbPort
    'dot1d_tp_fdb_status':    '1.3.6.1.2.1.17.4.3.1.3',   # dot1dTpFdbStatus
}

# sysObjectID prefijos para detección automática de Ubiquiti
UBNT_SYS_OBJECT_IDS = [
    '1.3.6.1.4.1.41112',  # Ubiquiti Networks (general)
    '1.3.6.1.4.1.4413',   # EdgeSwitch (Broadcom-based)
]


# ==============================================================================
# PROFILE CLASS
# ==============================================================================

class UbiquitiProfile(BaseDeviceProfile):
    """
    Perfil de dispositivo para switches Ubiquiti (EdgeSwitch, UniFi Switch).

    Características:
    - CPU via HOST-RESOURCES-MIB o UBNT-MIB
    - Memoria via HOST-RESOURCES-MIB o UBNT-MIB
    - Puertos via IF-MIB (estado, tráfico, errores)
    - PoE via POWER-ETHERNET-MIB (estado, consumo por puerto)
    - VLANs via Q-BRIDGE-MIB
    - Tabla MAC via BRIDGE-MIB
    - Temperatura (si soportado)
    """

    vendor = "ubnt"
    vendor_display_name = "Ubiquiti Networks"
    device_type = "switch"

    # ==========================================================================
    # OIDs
    # ==========================================================================

    def get_vendor_oids(self) -> Dict[str, str]:
        """Retorna OIDs específicos de UBNT + PoE + VLANs."""
        return {**UBNT_VENDOR_OIDS, **VLAN_OIDS, **BRIDGE_OIDS}

    def get_cpu_oids(self) -> Dict[str, str]:
        """Retorna OIDs de CPU via HOST-RESOURCES-MIB."""
        return UBNT_CPU_OIDS.copy()

    def get_memory_oids(self) -> Dict[str, str]:
        """Retorna OIDs de memoria via HOST-RESOURCES-MIB."""
        return UBNT_MEMORY_OIDS.copy()

    def get_disk_oids(self) -> Dict[str, str]:
        """Retorna OIDs de storage via HOST-RESOURCES-MIB."""
        return UBNT_DISK_OIDS.copy()

    # ==========================================================================
    # NORMALIZACIÓN DE CPU
    # ==========================================================================

    def normalize_cpu_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de CPU de Ubiquiti switches.

        Usa hrProcessorLoad (HOST-RESOURCES-MIB) si disponible.
        EdgeSwitch puede usar OID propio (edge_cpu_util).

        Args:
            raw_data: Datos crudos {oid_name: value}.

        Returns:
            Datos normalizados de CPU.
        """
        cpu_usage = safe_float(raw_data.get('hr_processor_load'))
        
        # Fallback a EdgeSwitch OID
        if cpu_usage == 0:
            cpu_usage = safe_float(raw_data.get('edge_cpu_util'))

        return {
            "cpu_usage_percent": round(cpu_usage, 2),
            "load_1min": None,
            "load_5min": None,
            "load_15min": None,
        }

    # ==========================================================================
    # NORMALIZACIÓN DE MEMORIA
    # ==========================================================================

    def normalize_memory_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de memoria de Ubiquiti switches.

        Usa hrStorageTable para extraer memoria RAM.

        Args:
            raw_data: Datos crudos {oid_name: value}.

        Returns:
            Datos normalizados de memoria.
        """
        # EdgeSwitch puede tener OIDs directos
        mem_total = safe_int(raw_data.get('edge_mem_total'))
        mem_free = safe_int(raw_data.get('edge_mem_free'))

        if mem_total > 0:
            mem_used = mem_total - mem_free
            return {
                "mem_usage_percent": round(calculate_percentage(mem_used, mem_total), 2),
                "mem_total_mb": round(mem_total / 1024.0, 2),
                "mem_used_mb": round(mem_used / 1024.0, 2),
                "mem_free_mb": round(mem_free / 1024.0, 2),
                "swap_total_mb": 0.0,
                "swap_free_mb": 0.0,
            }
        
        # Fallback: buscar en hrStorageTable (se procesa en normalize_disk_data)
        return {"error": "Memory data from hrStorageTable - check disk data"}

    # ==========================================================================
    # NORMALIZACIÓN DE DISCO/STORAGE
    # ==========================================================================

    def normalize_disk_data(
        self, raw_disk_entries: Dict[str, Dict[str, Any]]
    ) -> Dict[str, Dict[str, Any]]:
        """
        Normaliza datos de almacenamiento de Ubiquiti switches.

        Usa hrStorageTable. Separa RAM de Flash/Disk.

        Args:
            raw_disk_entries: Datos del bulk scan.

        Returns:
            Diccionario normalizado de storage.
        """
        disk_data: Dict[str, Dict[str, Any]] = {}
        disk_counter = 0

        # Keywords para filtrar RAM
        ram_keywords = ('real memory', 'ram', 'virtual memory', 'swap', 'buffers')

        for idx, raw in raw_disk_entries.items():
            descr = str(raw.get('hr_storage_descr', '')).strip()
            alloc_units = safe_int(raw.get('hr_storage_alloc'))
            size_units = safe_int(raw.get('hr_storage_size'))
            used_units = safe_int(raw.get('hr_storage_used'))

            if alloc_units <= 0 or size_units <= 0:
                continue

            # Omitir RAM
            if any(kw in descr.lower() for kw in ram_keywords):
                continue

            total_bytes = size_units * alloc_units
            used_bytes = used_units * alloc_units
            free_bytes = max(0, total_bytes - used_bytes)

            total_gb = round(total_bytes / (1024.0 ** 3), 3)
            used_gb = round(used_bytes / (1024.0 ** 3), 3)
            free_gb = round(free_bytes / (1024.0 ** 3), 3)
            percent = round(calculate_percentage(used_bytes, total_bytes), 2)

            disk_counter += 1
            disk_data[str(disk_counter)] = {
                "index": str(disk_counter),
                "path": descr or f"storage_{idx}",
                "total_gb": total_gb,
                "used_gb": used_gb,
                "free_gb": free_gb,
                "percent_used": percent,
            }

        return disk_data

    # ==========================================================================
    # DATOS ESPECÍFICOS DE UBIQUITI
    # ==========================================================================

    async def collect_vendor_specific_data(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta datos específicos de switches Ubiquiti.

        Obtiene:
        - Información del sistema (modelo, firmware, MAC)
        - Estado de puertos (admin, oper, velocidad, tráfico)
        - PoE por puerto (si disponible)
        - VLANs configuradas
        - Tabla de direcciones MAC
        - Temperatura (si soportado)

        Args:
            client: Instancia de SnmpClient conectada al switch.

        Returns:
            Diccionario con datos específicos de Ubiquiti.
        """
        logger.info("Recolectando datos específicos de Ubiquiti switch...")
        vendor_oids = self.get_vendor_oids()

        ubnt_data: Dict[str, Any] = {
            "system_info": {},
            "ports": [],
            "poe": {
                "available": False,
                "total_power_watts": 0,
                "used_power_watts": 0,
                "ports": [],
            },
            "vlans": [],
            "mac_table": {
                "total_entries": 0,
                "sample_entries": [],  # Primeras 50 para no saturar
            },
            "temperature": {},
            "collection_timestamp": now_iso(),
        }

        # ===== Sistema: Modelo, Firmware, MAC =====
        system_oid_names = ('unifi_model', 'unifi_version', 'unifi_mac', 'unifi_uptime')
        for oid_name in system_oid_names:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    ubnt_data["system_info"][oid_name] = str(res.value)
                else:
                    ubnt_data["system_info"][oid_name] = None

        # ===== Puertos =====
        ubnt_data["ports"] = await self._collect_ports(client, vendor_oids)

        # ===== PoE =====
        ubnt_data["poe"] = await self._collect_poe(client, vendor_oids)

        # ===== VLANs =====
        ubnt_data["vlans"] = await self._collect_vlans(client)

        # ===== Tabla MAC =====
        ubnt_data["mac_table"] = await self._collect_mac_table(client)

        # ===== Temperatura =====
        temp_oids = ('unifi_temp_board', 'unifi_temp_cpu')
        for oid_name in temp_oids:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    # Temperatura en décimas de grado
                    temp_val = safe_int(res.value)
                    ubnt_data["temperature"][oid_name] = round(temp_val / 10.0, 1) if temp_val > 100 else temp_val

        logger.info(f"Datos de Ubiquiti recolectados: {len(ubnt_data['ports'])} puertos")
        return ubnt_data

    # --------------------------------------------------------------------------
    # HELPER: Recolectar puertos
    # --------------------------------------------------------------------------

    async def _collect_ports(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> List[Dict[str, Any]]:
        """
        Recolecta información de puertos del switch.

        Returns:
            Lista de diccionarios con datos de cada puerto.
        """
        ports: List[Dict[str, Any]] = []

        try:
            # Obtener descripción de puertos
            if_descr_oid = vendor_oids.get('if_descr')
            if not if_descr_oid:
                return ports

            descr_results, error = await client.bulk(if_descr_oid)
            if error or not descr_results:
                return ports

            port_map: Dict[str, Dict[str, Any]] = {}
            for oid_str, value in descr_results:
                idx = oid_str.split('.')[-1]
                port_map[idx] = {
                    "index": idx,
                    "name": str(value).strip(),
                    "admin_status": "unknown",
                    "oper_status": "unknown",
                    "speed_mbps": 0,
                    "traffic_in_mb": 0,
                    "traffic_out_mb": 0,
                    "errors_in": 0,
                    "errors_out": 0,
                }

            # Recolectar métricas
            bulk_columns = {
                'if_admin_status':   ('admin_status',  lambda v: "UP" if str(v) == '1' else "DOWN"),
                'if_oper_status':    ('oper_status',   lambda v: "UP" if str(v) == '1' else "DOWN"),
                'if_high_speed':     ('speed_mbps',    lambda v: safe_int(v)),
                'if_hc_in_octets':   ('traffic_in',    lambda v: safe_int(v)),
                'if_hc_out_octets':  ('traffic_out',   lambda v: safe_int(v)),
                'if_in_errors':      ('errors_in',     lambda v: safe_int(v)),
                'if_out_errors':     ('errors_out',    lambda v: safe_int(v)),
            }

            for oid_name, (field, converter) in bulk_columns.items():
                oid = vendor_oids.get(oid_name)
                if not oid:
                    continue
                results, _ = await client.bulk(oid)
                for oid_str, value in (results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in port_map:
                        port_map[idx][field] = converter(value)

            # Convertir tráfico a MB
            for port in port_map.values():
                if 'traffic_in' in port:
                    port['traffic_in_mb'] = round(port.pop('traffic_in', 0) / (1024.0 * 1024.0), 2)
                if 'traffic_out' in port:
                    port['traffic_out_mb'] = round(port.pop('traffic_out', 0) / (1024.0 * 1024.0), 2)
                ports.append(port)

        except Exception as e:
            logger.warning(f"Error recolectando puertos: {e}")

        return ports

    # --------------------------------------------------------------------------
    # HELPER: Recolectar PoE
    # --------------------------------------------------------------------------

    async def _collect_poe(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> Dict[str, Any]:
        """
        Recolecta información de Power over Ethernet.

        Returns:
            Diccionario con datos de PoE del switch.
        """
        poe_data: Dict[str, Any] = {
            "available": False,
            "total_power_watts": 0,
            "used_power_watts": 0,
            "efficiency_percent": 0,
            "ports": [],
        }

        try:
            # Estado general del PSE
            pse_status_oid = vendor_oids.get('pse_main_status')
            if pse_status_oid:
                res = await client.get(pse_status_oid)
                if not res.error and res.value is not None:
                    poe_data["available"] = True
                    poe_data["pse_status"] = "on" if safe_int(res.value) == 1 else "off"

            # Potencia total y disponible
            power_consumption_oid = vendor_oids.get('pse_power_consumption')
            if power_consumption_oid:
                res = await client.get(power_consumption_oid)
                if not res.error and res.value is not None:
                    poe_data["used_power_watts"] = safe_int(res.value)

            power_available_oid = vendor_oids.get('pse_power_available')
            if power_available_oid:
                res = await client.get(power_available_oid)
                if not res.error and res.value is not None:
                    poe_data["total_power_watts"] = safe_int(res.value)

            # Eficiencia
            if poe_data["total_power_watts"] > 0:
                poe_data["efficiency_percent"] = round(
                    calculate_percentage(poe_data["used_power_watts"], poe_data["total_power_watts"]), 2
                )

            # PoE por puerto
            port_status_oid = vendor_oids.get('pse_port_status')
            if port_status_oid:
                port_results, _ = await client.bulk(port_status_oid)
                port_map: Dict[str, Dict[str, Any]] = {}
                
                status_map = {
                    1: 'disabled',
                    2: 'searching',
                    3: 'delivering_power',
                    4: 'fault',
                    5: 'test',
                    6: 'other_fault',
                }

                for oid_str, value in (port_results or []):
                    idx = oid_str.split('.')[-1]
                    status_code = safe_int(value)
                    port_map[idx] = {
                        "port_index": idx,
                        "status": status_map.get(status_code, f"unknown({status_code})"),
                        "status_code": status_code,
                        "power_mw": 0,
                        "power_class": None,
                    }

                # Potencia por puerto
                port_power_oid = vendor_oids.get('pse_port_power')
                if port_power_oid:
                    power_results, _ = await client.bulk(port_power_oid)
                    for oid_str, value in (power_results or []):
                        idx = oid_str.split('.')[-1]
                        if idx in port_map:
                            port_map[idx]["power_mw"] = safe_int(value)
                            port_map[idx]["power_watts"] = round(safe_int(value) / 1000.0, 2)

                # Clase de potencia por puerto
                port_class_oid = vendor_oids.get('pse_port_power_class')
                if port_class_oid:
                    class_results, _ = await client.bulk(port_class_oid)
                    for oid_str, value in (class_results or []):
                        idx = oid_str.split('.')[-1]
                        if idx in port_map:
                            port_map[idx]["power_class"] = safe_int(value)

                poe_data["ports"] = list(port_map.values())

        except Exception as e:
            logger.warning(f"Error recolectando PoE: {e}")

        return poe_data

    # --------------------------------------------------------------------------
    # HELPER: Recolectar VLANs
    # --------------------------------------------------------------------------

    async def _collect_vlans(self, client: 'SnmpClient') -> List[Dict[str, Any]]:
        """
        Recolecta información de VLANs configuradas via Q-BRIDGE-MIB.

        Returns:
            Lista de VLANs con nombre y ID.
        """
        vlans: List[Dict[str, Any]] = []

        try:
            vlan_name_oid = VLAN_OIDS.get('dot1q_vlan_static_name')
            if not vlan_name_oid:
                return vlans

            results, error = await client.bulk(vlan_name_oid)
            if error or not results:
                return vlans

            for oid_str, value in results:
                vlan_id = oid_str.split('.')[-1]
                vlans.append({
                    "vlan_id": safe_int(vlan_id),
                    "name": str(value).strip() if value else f"VLAN{vlan_id}",
                })

        except Exception as e:
            logger.warning(f"Error recolectando VLANs: {e}")

        return vlans

    # --------------------------------------------------------------------------
    # HELPER: Recolectar tabla MAC
    # --------------------------------------------------------------------------

    async def _collect_mac_table(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta la tabla de direcciones MAC via BRIDGE-MIB.

        Returns:
            Diccionario con conteo y muestra de MACs.
        """
        mac_data: Dict[str, Any] = {
            "total_entries": 0,
            "sample_entries": [],
        }

        try:
            mac_addr_oid = BRIDGE_OIDS.get('dot1d_tp_fdb_address')
            if not mac_addr_oid:
                return mac_data

            results, error = await client.bulk(mac_addr_oid)
            if error or not results:
                return mac_data

            mac_data["total_entries"] = len(results)

            # Solo guardar muestra de las primeras 50
            for i, (oid_str, value) in enumerate(results[:50]):
                # MAC address viene como bytes, convertir a formato legible
                mac_str = self._format_mac_address(value)
                mac_data["sample_entries"].append({
                    "index": i + 1,
                    "mac_address": mac_str,
                })

        except Exception as e:
            logger.warning(f"Error recolectando tabla MAC: {e}")

        return mac_data

    # --------------------------------------------------------------------------
    # HELPER: Formatear MAC address
    # --------------------------------------------------------------------------

    def _format_mac_address(self, value: Any) -> str:
        """Formatea un valor SNMP de MAC address a formato legible."""
        try:
            if isinstance(value, bytes):
                return ':'.join(f'{b:02x}' for b in value).upper()
            elif isinstance(value, str):
                # Puede venir como string con caracteres especiales
                if len(value) == 6:
                    return ':'.join(f'{ord(c):02x}' for c in value).upper()
                return value
            return str(value)
        except Exception:
            return str(value)

    # ==========================================================================
    # DETECCIÓN AUTOMÁTICA
    # ==========================================================================

    @classmethod
    def matches_sys_object_id(cls, sys_object_id: str) -> bool:
        """
        Detecta si un sysObjectID corresponde a Ubiquiti.

        Ubiquiti usa enterprise OID 1.3.6.1.4.1.41112.
        EdgeSwitch usa 1.3.6.1.4.1.4413 (Broadcom).
        """
        if not sys_object_id:
            return False
        return any(
            sys_object_id.startswith(prefix)
            for prefix in UBNT_SYS_OBJECT_IDS
        )