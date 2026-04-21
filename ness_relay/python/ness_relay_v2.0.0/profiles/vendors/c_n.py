# ==============================================================================
# NESS Relay v2.0.0 - Cambium Networks Vendor Profile
# ==============================================================================
# Perfil completo para Access Points Cambium Networks (cnPilot, ePMP, PMP).
#
# CPU/Memory:    HOST-RESOURCES-MIB + CAMBIUM-MIB
# Wireless:      CAMBIUM-CNS-AP-MIB (clientes, señal, canales)
# Interfaces:    IF-MIB
# Radio:         CAMBIUM-CNS-AP-MIB (potencia, canales, SSIDs)
#
# Enterprise OID base: 1.3.6.1.4.1.17713 (Cambium Networks)
# sysObjectID típico:  1.3.6.1.4.1.17713.*
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
# OIDs ESPECÍFICOS DE CAMBIUM NETWORKS
# ==============================================================================

# CPU OIDs - HOST-RESOURCES-MIB
CAMBIUM_CPU_OIDS: Dict[str, str] = {
    'hr_processor_load': '1.3.6.1.2.1.25.3.3.1.2',  # hrProcessorLoad (tabla)
}

# Memory OIDs - HOST-RESOURCES-MIB
CAMBIUM_MEMORY_OIDS: Dict[str, str] = {
    'hr_storage_descr':  '1.3.6.1.2.1.25.2.3.1.3',
    'hr_storage_alloc':  '1.3.6.1.2.1.25.2.3.1.4',
    'hr_storage_size':   '1.3.6.1.2.1.25.2.3.1.5',
    'hr_storage_used':   '1.3.6.1.2.1.25.2.3.1.6',
}

# Disk OIDs - HOST-RESOURCES-MIB
CAMBIUM_DISK_OIDS: Dict[str, str] = {
    'hr_storage_descr':  '1.3.6.1.2.1.25.2.3.1.3',
    'hr_storage_alloc':  '1.3.6.1.2.1.25.2.3.1.4',
    'hr_storage_size':   '1.3.6.1.2.1.25.2.3.1.5',
    'hr_storage_used':   '1.3.6.1.2.1.25.2.3.1.6',
}

# IF-MIB para interfaces
CAMBIUM_INTERFACE_OIDS: Dict[str, str] = {
    'if_descr':          '1.3.6.1.2.1.2.2.1.2',           # ifDescr
    'if_type':           '1.3.6.1.2.1.2.2.1.3',           # ifType
    'if_admin_status':   '1.3.6.1.2.1.2.2.1.7',           # ifAdminStatus
    'if_oper_status':    '1.3.6.1.2.1.2.2.1.8',           # ifOperStatus
    'if_in_octets':      '1.3.6.1.2.1.2.2.1.10',          # ifInOctets
    'if_out_octets':     '1.3.6.1.2.1.2.2.1.16',          # ifOutOctets
    'if_hc_in_octets':   '1.3.6.1.2.1.31.1.1.1.6',        # ifHCInOctets
    'if_hc_out_octets':  '1.3.6.1.2.1.31.1.1.1.10',       # ifHCOutOctets
    'if_name':           '1.3.6.1.2.1.31.1.1.1.1',        # ifName
}

# IEEE 802.11 MIB - Estándar para wireless
IEEE80211_OIDS: Dict[str, str] = {
    'dot11_ssid':                    '1.2.840.10036.1.1.1.9',        # dot11DesiredSSID
    'dot11_channel':                 '1.2.840.10036.1.1.1.14',       # dot11CurrentChannel
    'dot11_station_id':              '1.2.840.10036.1.1.1.1',        # dot11StationID (MAC)
    'dot11_tx_power':                '1.2.840.10036.2.1.1.2',        # dot11CurrentTxPowerLevel
    'dot11_associations':            '1.2.840.10036.4.2.1.1',        # dot11AssociatedStationCount
}

# CAMBIUM-CNS-AP-MIB OIDs específicos
# Nota: Cambium tiene variantes según producto (cnPilot E-series, PMP, ePMP)
CAMBIUM_VENDOR_OIDS: Dict[str, str] = {
    # Sistema
    'cambium_model':             '1.3.6.1.4.1.17713.1.1.1.0',        # Modelo AP
    'cambium_firmware':          '1.3.6.1.4.1.17713.1.1.2.0',        # Firmware version
    'cambium_serial':            '1.3.6.1.4.1.17713.1.1.3.0',        # Serial number
    'cambium_mac':               '1.3.6.1.4.1.17713.1.1.4.0',        # MAC address
    'cambium_uptime':            '1.3.6.1.4.1.17713.1.1.5.0',        # Uptime
    
    # CPU/Recursos
    'cambium_cpu_usage':         '1.3.6.1.4.1.17713.1.2.1.0',        # CPU usage %
    'cambium_mem_total':         '1.3.6.1.4.1.17713.1.2.2.0',        # Total memory KB
    'cambium_mem_free':          '1.3.6.1.4.1.17713.1.2.3.0',        # Free memory KB
    
    # Temperaturas
    'cambium_temp_board':        '1.3.6.1.4.1.17713.1.2.10.0',       # Board temperature
    'cambium_temp_cpu':          '1.3.6.1.4.1.17713.1.2.11.0',       # CPU temperature
    
    # Radio - General
    'cambium_radio_enable':      '1.3.6.1.4.1.17713.1.3.1.1',        # Radio enable state (tabla)
    'cambium_radio_channel':     '1.3.6.1.4.1.17713.1.3.1.2',        # Current channel
    'cambium_radio_tx_power':    '1.3.6.1.4.1.17713.1.3.1.3',        # TX power (dBm)
    'cambium_radio_frequency':   '1.3.6.1.4.1.17713.1.3.1.4',        # Frequency (MHz)
    'cambium_radio_bandwidth':   '1.3.6.1.4.1.17713.1.3.1.5',        # Channel width (MHz)
    
    # Radio - Estadísticas
    'cambium_rx_data_bytes':     '1.3.6.1.4.1.17713.1.3.2.1',        # RX bytes (tabla)
    'cambium_tx_data_bytes':     '1.3.6.1.4.1.17713.1.3.2.2',        # TX bytes
    'cambium_rx_packets':        '1.3.6.1.4.1.17713.1.3.2.3',        # RX packets
    'cambium_tx_packets':        '1.3.6.1.4.1.17713.1.3.2.4',        # TX packets
    'cambium_rx_errors':         '1.3.6.1.4.1.17713.1.3.2.5',        # RX errors
    'cambium_tx_errors':         '1.3.6.1.4.1.17713.1.3.2.6',        # TX errors
    
    # Clientes conectados
    'cambium_client_count':      '1.3.6.1.4.1.17713.1.4.1.0',        # Total connected clients
    'cambium_client_table':      '1.3.6.1.4.1.17713.1.4.2.1',        # Client table entry
    'cambium_client_mac':        '1.3.6.1.4.1.17713.1.4.2.1.1',      # Client MAC
    'cambium_client_rssi':       '1.3.6.1.4.1.17713.1.4.2.1.2',      # Client RSSI (dBm)
    'cambium_client_snr':        '1.3.6.1.4.1.17713.1.4.2.1.3',      # Client SNR (dB)
    'cambium_client_tx_rate':    '1.3.6.1.4.1.17713.1.4.2.1.4',      # Client TX rate (Mbps)
    'cambium_client_rx_rate':    '1.3.6.1.4.1.17713.1.4.2.1.5',      # Client RX rate (Mbps)
    'cambium_client_ssid':       '1.3.6.1.4.1.17713.1.4.2.1.6',      # Client SSID
    'cambium_client_uptime':     '1.3.6.1.4.1.17713.1.4.2.1.7',      # Client connected time
    
    # WLAN/SSID Config
    'cambium_ssid_name':         '1.3.6.1.4.1.17713.1.5.1.1.1',      # SSID name (tabla)
    'cambium_ssid_enable':       '1.3.6.1.4.1.17713.1.5.1.1.2',      # SSID enable
    'cambium_ssid_vlan':         '1.3.6.1.4.1.17713.1.5.1.1.3',      # SSID VLAN
    'cambium_ssid_broadcast':    '1.3.6.1.4.1.17713.1.5.1.1.4',      # SSID broadcast
    'cambium_ssid_security':     '1.3.6.1.4.1.17713.1.5.1.1.5',      # Security type
    
    # Interface utilization
    'cambium_channel_util':      '1.3.6.1.4.1.17713.1.3.3.1.0',      # Channel utilization %
    'cambium_noise_floor':       '1.3.6.1.4.1.17713.1.3.3.2.0',      # Noise floor (dBm)
    'cambium_interference':      '1.3.6.1.4.1.17713.1.3.3.3.0',      # Interference %
    
    # ePMP específicos (1.3.6.1.4.1.17713.21.*)
    'epmp_link_distance':        '1.3.6.1.4.1.17713.21.1.2.1.0',     # Link distance (m)
    'epmp_link_capacity':        '1.3.6.1.4.1.17713.21.1.2.2.0',     # Link capacity (Mbps)
    'epmp_uplink_rssi':          '1.3.6.1.4.1.17713.21.1.2.3.0',     # Uplink RSSI
    'epmp_downlink_rssi':        '1.3.6.1.4.1.17713.21.1.2.4.0',     # Downlink RSSI
    
    # cnPilot específicos (1.3.6.1.4.1.17713.7.*)
    'cnpilot_clients_2g':        '1.3.6.1.4.1.17713.7.1.4.1.0',      # 2.4GHz clients
    'cnpilot_clients_5g':        '1.3.6.1.4.1.17713.7.1.4.2.0',      # 5GHz clients
}

# Productos/familias por sysObjectID
CAMBIUM_SYS_OBJECT_IDS = [
    '1.3.6.1.4.1.17713',      # Cambium Networks (general)
    '1.3.6.1.4.1.17713.7',    # cnPilot Enterprise APs
    '1.3.6.1.4.1.17713.21',   # ePMP
    '1.3.6.1.4.1.17713.22',   # PMP 450
]


# ==============================================================================
# PROFILE CLASS
# ==============================================================================

class CambiumProfile(BaseDeviceProfile):
    """
    Perfil de dispositivo para Access Points Cambium Networks.

    Productos soportados:
    - cnPilot E-series (Enterprise APs)
    - ePMP (Point-to-Multipoint)
    - PMP 450 series

    Características:
    - CPU via HR-MIB o CAMBIUM-MIB
    - Memoria via HR-MIB o CAMBIUM-MIB
    - Clientes conectados (cantidad, RSSI, SNR, tasa)
    - Radio (canal, potencia, ancho de banda)
    - SSIDs configurados
    - Utilización de canal e interferencia
    - Temperatura
    """

    vendor = "c_n"
    vendor_display_name = "Cambium Networks"
    device_type = "access_point"

    # ==========================================================================
    # OIDs
    # ==========================================================================

    def get_vendor_oids(self) -> Dict[str, str]:
        """Retorna OIDs específicos de Cambium + interfaces."""
        return {**CAMBIUM_VENDOR_OIDS, **CAMBIUM_INTERFACE_OIDS, **IEEE80211_OIDS}

    def get_cpu_oids(self) -> Dict[str, str]:
        """Retorna OIDs de CPU via HOST-RESOURCES-MIB."""
        return CAMBIUM_CPU_OIDS.copy()

    def get_memory_oids(self) -> Dict[str, str]:
        """Retorna OIDs de memoria via HOST-RESOURCES-MIB."""
        return CAMBIUM_MEMORY_OIDS.copy()

    def get_disk_oids(self) -> Dict[str, str]:
        """Retorna OIDs de storage via HOST-RESOURCES-MIB."""
        return CAMBIUM_DISK_OIDS.copy()

    # ==========================================================================
    # NORMALIZACIÓN DE CPU
    # ==========================================================================

    def normalize_cpu_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de CPU de Cambium APs.

        Intenta usar OID específico Cambium, fallback a hrProcessorLoad.

        Args:
            raw_data: Datos crudos {oid_name: value}.

        Returns:
            Datos normalizados de CPU.
        """
        # Intentar OID Cambium específico
        cpu_usage = safe_float(raw_data.get('cambium_cpu_usage'))
        
        # Fallback HOST-RESOURCES-MIB
        if cpu_usage == 0:
            cpu_usage = safe_float(raw_data.get('hr_processor_load'))

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
        Normaliza datos de memoria de Cambium APs.

        Intenta usar OIDs específicos Cambium, fallback a hrStorageTable.

        Args:
            raw_data: Datos crudos {oid_name: value}.

        Returns:
            Datos normalizados de memoria.
        """
        # OIDs Cambium específicos
        mem_total = safe_int(raw_data.get('cambium_mem_total'))  # KB
        mem_free = safe_int(raw_data.get('cambium_mem_free'))    # KB

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
        
        return {"error": "Memory data via hrStorageTable - check disk data"}

    # ==========================================================================
    # NORMALIZACIÓN DE DISCO/STORAGE
    # ==========================================================================

    def normalize_disk_data(
        self, raw_disk_entries: Dict[str, Dict[str, Any]]
    ) -> Dict[str, Dict[str, Any]]:
        """
        Normaliza datos de almacenamiento de Cambium APs.

        APs típicamente tienen flash storage mínimo.

        Args:
            raw_disk_entries: Datos del bulk scan.

        Returns:
            Diccionario normalizado de storage.
        """
        disk_data: Dict[str, Dict[str, Any]] = {}
        disk_counter = 0

        ram_keywords = ('real memory', 'ram', 'virtual memory', 'swap', 'buffers')

        for idx, raw in raw_disk_entries.items():
            descr = str(raw.get('hr_storage_descr', '')).strip()
            alloc_units = safe_int(raw.get('hr_storage_alloc'))
            size_units = safe_int(raw.get('hr_storage_size'))
            used_units = safe_int(raw.get('hr_storage_used'))

            if alloc_units <= 0 or size_units <= 0:
                continue

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
    # DATOS ESPECÍFICOS DE CAMBIUM
    # ==========================================================================

    async def collect_vendor_specific_data(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta datos específicos de Access Points Cambium.

        Obtiene:
        - Información del sistema (modelo, firmware, serial)
        - Clientes conectados (cantidad, señal, tasas)
        - Radio (canal, potencia, frecuencia, ancho banda)
        - SSIDs configurados
        - Utilización de canal e interferencia
        - Temperatura

        Args:
            client: Instancia de SnmpClient conectada al AP.

        Returns:
            Diccionario con datos específicos de Cambium.
        """
        logger.info("Recolectando datos específicos de Cambium AP...")
        vendor_oids = self.get_vendor_oids()

        cambium_data: Dict[str, Any] = {
            "system_info": {},
            "radio": [],
            "clients": {
                "total_count": 0,
                "count_2g": 0,
                "count_5g": 0,
                "details": [],  # Primeros 100 clientes
            },
            "ssids": [],
            "rf_environment": {
                "channel_utilization": 0,
                "noise_floor_dbm": None,
                "interference_percent": 0,
            },
            "temperature": {},
            "interfaces": [],
            "collection_timestamp": now_iso(),
        }

        # ===== Sistema: Modelo, Firmware, Serial =====
        system_oid_names = (
            'cambium_model', 'cambium_firmware', 'cambium_serial',
            'cambium_mac', 'cambium_uptime'
        )
        for oid_name in system_oid_names:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    cambium_data["system_info"][oid_name] = str(res.value)
                else:
                    cambium_data["system_info"][oid_name] = None

        # ===== Radio =====
        cambium_data["radio"] = await self._collect_radio_info(client, vendor_oids)

        # ===== Clientes =====
        cambium_data["clients"] = await self._collect_clients(client, vendor_oids)

        # ===== SSIDs =====
        cambium_data["ssids"] = await self._collect_ssids(client, vendor_oids)

        # ===== Ambiente RF =====
        cambium_data["rf_environment"] = await self._collect_rf_environment(client, vendor_oids)

        # ===== Temperatura =====
        temp_oids = ('cambium_temp_board', 'cambium_temp_cpu')
        for oid_name in temp_oids:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    temp_val = safe_int(res.value)
                    # Algunas unidades vienen en décimas de grado
                    cambium_data["temperature"][oid_name] = (
                        round(temp_val / 10.0, 1) if temp_val > 100 else temp_val
                    )

        # ===== Interfaces =====
        cambium_data["interfaces"] = await self._collect_interfaces(client, vendor_oids)

        logger.info(f"Datos de Cambium recolectados: {cambium_data['clients']['total_count']} clientes")
        return cambium_data

    # --------------------------------------------------------------------------
    # HELPER: Recolectar info de radio
    # --------------------------------------------------------------------------

    async def _collect_radio_info(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> List[Dict[str, Any]]:
        """
        Recolecta información de radios del AP.

        Returns:
            Lista de diccionarios con datos de cada radio.
        """
        radios: List[Dict[str, Any]] = []

        try:
            # Intentar obtener canal/frecuencia via tabla de radio
            radio_channel_oid = vendor_oids.get('cambium_radio_channel')
            if radio_channel_oid:
                results, error = await client.bulk(radio_channel_oid)
                
                if not error and results:
                    radio_map: Dict[str, Dict[str, Any]] = {}
                    
                    for oid_str, value in results:
                        idx = oid_str.split('.')[-1]
                        radio_map[idx] = {
                            "radio_index": idx,
                            "channel": safe_int(value),
                            "frequency_mhz": 0,
                            "tx_power_dbm": 0,
                            "bandwidth_mhz": 0,
                            "enabled": True,
                        }
                    
                    # Obtener otras propiedades
                    radio_props = {
                        'cambium_radio_frequency':  ('frequency_mhz',  lambda v: safe_int(v)),
                        'cambium_radio_tx_power':   ('tx_power_dbm',   lambda v: safe_int(v)),
                        'cambium_radio_bandwidth':  ('bandwidth_mhz',  lambda v: safe_int(v)),
                        'cambium_radio_enable':     ('enabled',        lambda v: str(v) == '1'),
                    }
                    
                    for oid_name, (field, converter) in radio_props.items():
                        oid = vendor_oids.get(oid_name)
                        if not oid:
                            continue
                        results_prop, _ = await client.bulk(oid)
                        for oid_str, value in (results_prop or []):
                            idx = oid_str.split('.')[-1]
                            if idx in radio_map:
                                radio_map[idx][field] = converter(value)
                    
                    # Determinar banda basado en frecuencia
                    for radio in radio_map.values():
                        freq = radio.get('frequency_mhz', 0)
                        if freq > 0:
                            radio['band'] = '5GHz' if freq >= 5000 else '2.4GHz'
                        else:
                            # Inferir de canal
                            channel = radio.get('channel', 0)
                            radio['band'] = '5GHz' if channel >= 36 else '2.4GHz'
                        radios.append(radio)

            # Si no hay datos vía tabla, intentar OIDs individuales IEEE 802.11
            if not radios:
                radio_data: Dict[str, Any] = {"radio_index": "1"}
                
                ieee_mappings = {
                    'dot11_channel':    ('channel',      lambda v: safe_int(v)),
                    'dot11_tx_power':   ('tx_power_dbm', lambda v: safe_int(v)),
                    'dot11_ssid':       ('ssid',         lambda v: str(v)),
                }
                
                for oid_name, (field, converter) in ieee_mappings.items():
                    oid = vendor_oids.get(oid_name)
                    if oid:
                        res = await client.get(oid)
                        if not res.error and res.value is not None:
                            radio_data[field] = converter(res.value)
                
                if radio_data.get('channel'):
                    radio_data['band'] = '5GHz' if radio_data.get('channel', 0) >= 36 else '2.4GHz'
                    radios.append(radio_data)

        except Exception as e:
            logger.warning(f"Error recolectando info de radio: {e}")

        return radios

    # --------------------------------------------------------------------------
    # HELPER: Recolectar clientes
    # --------------------------------------------------------------------------

    async def _collect_clients(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> Dict[str, Any]:
        """
        Recolecta información de clientes conectados.

        Returns:
            Diccionario con conteo y detalles de clientes.
        """
        client_data: Dict[str, Any] = {
            "total_count": 0,
            "count_2g": 0,
            "count_5g": 0,
            "details": [],
        }

        try:
            # Total de clientes
            count_oid = vendor_oids.get('cambium_client_count')
            if count_oid:
                res = await client.get(count_oid)
                if not res.error and res.value is not None:
                    client_data["total_count"] = safe_int(res.value)

            # Conteo por banda (cnPilot)
            for oid_name, field in [('cnpilot_clients_2g', 'count_2g'), ('cnpilot_clients_5g', 'count_5g')]:
                oid = vendor_oids.get(oid_name)
                if oid:
                    res = await client.get(oid)
                    if not res.error and res.value is not None:
                        client_data[field] = safe_int(res.value)

            # Detalles de clientes (tabla)
            client_mac_oid = vendor_oids.get('cambium_client_mac')
            if client_mac_oid:
                results, error = await client.bulk(client_mac_oid)
                
                if not error and results:
                    client_map: Dict[str, Dict[str, Any]] = {}
                    
                    for oid_str, value in results[:100]:  # Limitar a 100
                        idx = oid_str.split('.')[-1]
                        client_map[idx] = {
                            "client_index": idx,
                            "mac_address": self._format_mac_address(value),
                            "rssi_dbm": None,
                            "snr_db": None,
                            "tx_rate_mbps": 0,
                            "rx_rate_mbps": 0,
                            "ssid": None,
                            "uptime_seconds": 0,
                        }
                    
                    # Obtener propiedades adicionales
                    client_props = {
                        'cambium_client_rssi':     ('rssi_dbm',       lambda v: safe_int(v)),
                        'cambium_client_snr':      ('snr_db',         lambda v: safe_int(v)),
                        'cambium_client_tx_rate':  ('tx_rate_mbps',   lambda v: safe_int(v)),
                        'cambium_client_rx_rate':  ('rx_rate_mbps',   lambda v: safe_int(v)),
                        'cambium_client_ssid':     ('ssid',           lambda v: str(v)),
                        'cambium_client_uptime':   ('uptime_seconds', lambda v: safe_int(v)),
                    }
                    
                    for oid_name, (field, converter) in client_props.items():
                        oid = vendor_oids.get(oid_name)
                        if not oid:
                            continue
                        results_prop, _ = await client.bulk(oid)
                        for oid_str, value in (results_prop or []):
                            idx = oid_str.split('.')[-1]
                            if idx in client_map:
                                client_map[idx][field] = converter(value)
                    
                    client_data["details"] = list(client_map.values())
                    
                    # Actualizar total si no se obtuvo vía OID directo
                    if client_data["total_count"] == 0:
                        client_data["total_count"] = len(client_data["details"])

        except Exception as e:
            logger.warning(f"Error recolectando clientes: {e}")

        return client_data

    # --------------------------------------------------------------------------
    # HELPER: Recolectar SSIDs
    # --------------------------------------------------------------------------

    async def _collect_ssids(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> List[Dict[str, Any]]:
        """
        Recolecta SSIDs configurados.

        Returns:
            Lista de SSIDs con configuración.
        """
        ssids: List[Dict[str, Any]] = []

        try:
            ssid_name_oid = vendor_oids.get('cambium_ssid_name')
            if not ssid_name_oid:
                return ssids

            results, error = await client.bulk(ssid_name_oid)
            if error or not results:
                return ssids

            ssid_map: Dict[str, Dict[str, Any]] = {}
            
            for oid_str, value in results:
                idx = oid_str.split('.')[-1]
                ssid_map[idx] = {
                    "ssid_index": idx,
                    "name": str(value).strip() if value else f"SSID_{idx}",
                    "enabled": True,
                    "vlan_id": None,
                    "broadcast": True,
                    "security": "unknown",
                }

            # Propiedades adicionales
            ssid_props = {
                'cambium_ssid_enable':    ('enabled',    lambda v: str(v) == '1'),
                'cambium_ssid_vlan':      ('vlan_id',    lambda v: safe_int(v)),
                'cambium_ssid_broadcast': ('broadcast',  lambda v: str(v) == '1'),
                'cambium_ssid_security':  ('security',   lambda v: str(v)),
            }
            
            for oid_name, (field, converter) in ssid_props.items():
                oid = vendor_oids.get(oid_name)
                if not oid:
                    continue
                results_prop, _ = await client.bulk(oid)
                for oid_str, value in (results_prop or []):
                    idx = oid_str.split('.')[-1]
                    if idx in ssid_map:
                        ssid_map[idx][field] = converter(value)

            ssids = list(ssid_map.values())

        except Exception as e:
            logger.warning(f"Error recolectando SSIDs: {e}")

        return ssids

    # --------------------------------------------------------------------------
    # HELPER: Recolectar ambiente RF
    # --------------------------------------------------------------------------

    async def _collect_rf_environment(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> Dict[str, Any]:
        """
        Recolecta información del ambiente RF.

        Returns:
            Diccionario con métricas de RF.
        """
        rf_data: Dict[str, Any] = {
            "channel_utilization": 0,
            "noise_floor_dbm": None,
            "interference_percent": 0,
        }

        try:
            rf_oids = {
                'cambium_channel_util':  ('channel_utilization',  lambda v: safe_int(v)),
                'cambium_noise_floor':   ('noise_floor_dbm',      lambda v: safe_int(v)),
                'cambium_interference':  ('interference_percent', lambda v: safe_int(v)),
            }
            
            for oid_name, (field, converter) in rf_oids.items():
                oid = vendor_oids.get(oid_name)
                if oid:
                    res = await client.get(oid)
                    if not res.error and res.value is not None:
                        rf_data[field] = converter(res.value)

        except Exception as e:
            logger.warning(f"Error recolectando ambiente RF: {e}")

        return rf_data

    # --------------------------------------------------------------------------
    # HELPER: Recolectar interfaces
    # --------------------------------------------------------------------------

    async def _collect_interfaces(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> List[Dict[str, Any]]:
        """
        Recolecta información de interfaces (ethernet + wireless).

        Returns:
            Lista de interfaces con estado y tráfico.
        """
        interfaces: List[Dict[str, Any]] = []

        try:
            if_descr_oid = vendor_oids.get('if_descr')
            if not if_descr_oid:
                return interfaces

            results, error = await client.bulk(if_descr_oid)
            if error or not results:
                return interfaces

            if_map: Dict[str, Dict[str, Any]] = {}
            
            for oid_str, value in results:
                idx = oid_str.split('.')[-1]
                if_map[idx] = {
                    "index": idx,
                    "name": str(value).strip(),
                    "admin_status": "unknown",
                    "oper_status": "unknown",
                    "traffic_in_mb": 0,
                    "traffic_out_mb": 0,
                }

            # Recolectar métricas
            if_props = {
                'if_admin_status':   ('admin_status',  lambda v: "UP" if str(v) == '1' else "DOWN"),
                'if_oper_status':    ('oper_status',   lambda v: "UP" if str(v) == '1' else "DOWN"),
                'if_hc_in_octets':   ('traffic_in',    lambda v: safe_int(v)),
                'if_hc_out_octets':  ('traffic_out',   lambda v: safe_int(v)),
            }
            
            for oid_name, (field, converter) in if_props.items():
                oid = vendor_oids.get(oid_name)
                if not oid:
                    continue
                results_prop, _ = await client.bulk(oid)
                for oid_str, value in (results_prop or []):
                    idx = oid_str.split('.')[-1]
                    if idx in if_map:
                        if_map[idx][field] = converter(value)

            # Convertir bytes a MB
            for iface in if_map.values():
                if 'traffic_in' in iface:
                    iface['traffic_in_mb'] = round(iface.pop('traffic_in', 0) / (1024.0 * 1024.0), 2)
                if 'traffic_out' in iface:
                    iface['traffic_out_mb'] = round(iface.pop('traffic_out', 0) / (1024.0 * 1024.0), 2)
                interfaces.append(iface)

        except Exception as e:
            logger.warning(f"Error recolectando interfaces: {e}")

        return interfaces

    # --------------------------------------------------------------------------
    # HELPER: Formatear MAC address
    # --------------------------------------------------------------------------

    def _format_mac_address(self, value: Any) -> str:
        """Formatea un valor SNMP de MAC address a formato legible."""
        try:
            if isinstance(value, bytes):
                return ':'.join(f'{b:02x}' for b in value).upper()
            elif isinstance(value, str):
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
        Detecta si un sysObjectID corresponde a Cambium Networks.

        Enterprise OID: 1.3.6.1.4.1.17713
        """
        if not sys_object_id:
            return False
        return any(
            sys_object_id.startswith(prefix)
            for prefix in CAMBIUM_SYS_OBJECT_IDS
        )