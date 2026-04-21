# ==============================================================================
# NESS Relay v2.0.0 - MikroTik RouterOS Vendor Profile
# ==============================================================================
# Perfil completo para dispositivos MikroTik RouterOS.
#
# CPU:     HOST-RESOURCES-MIB (hrProcessorLoad - tabla, promedio de cores)
# Memory:  MIKROTIK-MIB (mtxrHlTotalMemory, mtxrHlFreeMemory)
# Disk:    HOST-RESOURCES-MIB (hrStorageTable) + MIKROTIK-MIB fallback
# Vendor:  MIKROTIK-MIB (firmware, license, serial, temp, voltage, wireless)
#
# Enterprise OID base: 1.3.6.1.4.1.14988 (MikroTik)
# sysObjectID típico:  1.3.6.1.4.1.14988.1.*
# ==============================================================================

import logging
from typing import Any, Dict

from profiles.base_profile import BaseDeviceProfile
from utils.conversions import (
    calculate_percentage,
    safe_float,
    safe_int,
)
from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


# ==============================================================================
# OIDs ESPECÍFICOS DE MikroTik RouterOS
# ==============================================================================

# CPU OIDs - HOST-RESOURCES-MIB
# MikroTik expone carga de CPU por procesador via hrProcessorLoad (tabla).
# Se hace bulk walk y se promedia entre todos los cores.
MIKROTIK_CPU_OIDS: Dict[str, str] = {
    'hr_processor_load': '1.3.6.1.2.1.25.3.3.1.2',  # hrProcessorLoad (tabla, % por core)
}

# Memory OIDs - MIKROTIK-MIB (Health)
# Valores en bytes (escalares).
MIKROTIK_MEMORY_OIDS: Dict[str, str] = {
    'mtxr_hl_total_memory': '1.3.6.1.4.1.14988.1.1.3.12.0',  # mtxrHlTotalMemory (bytes)
    'mtxr_hl_free_memory':  '1.3.6.1.4.1.14988.1.1.3.13.0',  # mtxrHlFreeMemory (bytes)
}

# Disk OIDs - HOST-RESOURCES-MIB (hrStorageTable)
# Se usa hrStorageTable para obtener NAND/flash. Sin .0 porque es tabla.
MIKROTIK_DISK_OIDS: Dict[str, str] = {
    'hr_storage_descr':          '1.3.6.1.2.1.25.2.3.1.3',   # hrStorageDescr
    'hr_storage_allocation':     '1.3.6.1.2.1.25.2.3.1.4',   # hrStorageAllocationUnits (bytes)
    'hr_storage_size':           '1.3.6.1.2.1.25.2.3.1.5',   # hrStorageSize (units)
    'hr_storage_used':           '1.3.6.1.2.1.25.2.3.1.6',   # hrStorageUsed (units)
}

# Vendor-specific OIDs - MIKROTIK-MIB
MIKROTIK_VENDOR_OIDS: Dict[str, str] = {
    # Firmware / Software
    'mtxr_firmware_version':     '1.3.6.1.4.1.14988.1.1.4.4.0',   # mtxrFirmwareVersion
    'mtxr_license_id':           '1.3.6.1.4.1.14988.1.1.4.3.0',   # mtxrLicenseId
    'mtxr_serial_number':        '1.3.6.1.4.1.14988.1.1.7.3.0',   # mtxrSerialNumber
    'mtxr_firmware_upgrade_ver': '1.3.6.1.4.1.14988.1.1.4.7.0',   # mtxrFirmwareUpgradeVersion
    'mtxr_board_name':           '1.3.6.1.4.1.14988.1.1.7.8.0',   # mtxrBoardName

    # Health: Temperatura y Voltaje
    'mtxr_hl_temperature':       '1.3.6.1.4.1.14988.1.1.3.10.0',  # mtxrHlTemperature (°C * 10)
    'mtxr_hl_voltage':           '1.3.6.1.4.1.14988.1.1.3.8.0',   # mtxrHlVoltage (mV * 10)
    'mtxr_hl_power_consumption': '1.3.6.1.4.1.14988.1.1.3.12.0',  # mtxrHlPowerConsumption (W * 10)
    'mtxr_hl_processor_temp':    '1.3.6.1.4.1.14988.1.1.3.11.0',  # mtxrHlProcessorTemperature (°C * 10)
    'mtxr_hl_current':           '1.3.6.1.4.1.14988.1.1.3.9.0',   # mtxrHlCurrent (mA)
    'mtxr_hl_fan_speed1':        '1.3.6.1.4.1.14988.1.1.3.17.0',  # mtxrHlFanSpeed1 (RPM)
    'mtxr_hl_fan_speed2':        '1.3.6.1.4.1.14988.1.1.3.18.0',  # mtxrHlFanSpeed2 (RPM)

    # Disco via MIKROTIK-MIB (fallback si hrStorageTable no funciona)
    'mtxr_hl_disk_total':        '1.3.6.1.4.1.14988.1.1.3.1.0',   # mtxrHlDiskTotal (bytes)  (Nota: algunos modelos)
    'mtxr_hl_disk_used':         '1.3.6.1.4.1.14988.1.1.3.2.0',   # mtxrHlDiskUsed (bytes)

    # Wireless - Cantidad de clientes registrados (tabla, primera interfaz)
    'mtxr_wl_rt_tab_addr':       '1.3.6.1.4.1.14988.1.1.1.2.1.1', # mtxrWlRtabAddr (tabla - registrations)
    'mtxr_wl_ap_client_count':   '1.3.6.1.4.1.14988.1.1.1.3.1.6', # mtxrWlApClientCount (tabla)

    # Interfaces activas
    'if_number':                 '1.3.6.1.2.1.2.1.0',              # ifNumber (total interfaces)
}

# sysObjectID prefijos para detección automática de MikroTik
MIKROTIK_SYS_OBJECT_IDS = [
    '1.3.6.1.4.1.14988.1',       # MikroTik RouterOS (modelos principales)
    '1.3.6.1.4.1.14988',          # MikroTik (general)
]


# ==============================================================================
# PROFILE CLASS
# ==============================================================================

class MikroTikProfile(BaseDeviceProfile):
    """
    Perfil de dispositivo para MikroTik RouterOS.

    Características:
    - CPU via HOST-RESOURCES-MIB (hrProcessorLoad - promedio de cores)
    - Memoria via MIKROTIK-MIB (mtxrHlTotalMemory, mtxrHlFreeMemory)
    - Disco via HOST-RESOURCES-MIB (hrStorageTable) con fallback MIKROTIK-MIB
    - Health: temperatura, voltaje, corriente, ventiladores
    - System: firmware, serial, licencia, board name
    - Wireless: clientes conectados
    """

    vendor = "mikrotik"
    vendor_display_name = "MikroTik RouterOS"
    device_type = "router"

    # ==========================================================================
    # OIDs
    # ==========================================================================

    def get_vendor_oids(self) -> Dict[str, str]:
        """Retorna OIDs específicos de MIKROTIK-MIB."""
        return MIKROTIK_VENDOR_OIDS.copy()

    def get_cpu_oids(self) -> Dict[str, str]:
        """Retorna OIDs de CPU via HOST-RESOURCES-MIB."""
        return MIKROTIK_CPU_OIDS.copy()

    def get_memory_oids(self) -> Dict[str, str]:
        """Retorna OIDs de memoria via MIKROTIK-MIB."""
        return MIKROTIK_MEMORY_OIDS.copy()

    def get_disk_oids(self) -> Dict[str, str]:
        """Retorna OIDs de disco via HOST-RESOURCES-MIB (hrStorageTable)."""
        return MIKROTIK_DISK_OIDS.copy()

    # ==========================================================================
    # NORMALIZACIÓN DE CPU
    # ==========================================================================

    def normalize_cpu_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de CPU de MikroTik RouterOS.

        MikroTik expone hrProcessorLoad como tabla con un entry por core.
        El performance_collector consulta con get() por OID, pero
        hrProcessorLoad es tabla por lo que el valor puede ser
        del primer core. Calculamos promedio si hay múltiples valores.

        Nota: El performance_collector usa get() para CPU, no bulk().
        Para hrProcessorLoad esto retorna el primer core. Para obtener
        todos los cores se necesitaría bulk walk, lo cual hacemos
        en collect_vendor_specific_data y lo complementamos aquí.

        Args:
            raw_data: Datos crudos {oid_name: value} de las queries SNMP.

        Returns:
            Datos normalizados de CPU en formato estándar NESS.
        """
        # El performance_collector hace get() en hr_processor_load,
        # que es OID de tabla. Esto podría retornar el primer entry o None.
        raw_cpu = raw_data.get('hr_processor_load')
        cpu_usage = safe_float(raw_cpu) if raw_cpu is not None else 0.0

        return {
            "cpu_usage_percent": round(cpu_usage, 2),
            "load_1min": None,   # RouterOS no expone load averages via SNMP
            "load_5min": None,
            "load_15min": None,
        }

    # ==========================================================================
    # NORMALIZACIÓN DE MEMORIA
    # ==========================================================================

    def normalize_memory_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de memoria de MikroTik RouterOS.

        MikroTik reporta via MIKROTIK-MIB:
        - mtxrHlTotalMemory: Memoria total en bytes
        - mtxrHlFreeMemory:  Memoria libre en bytes

        Args:
            raw_data: Datos crudos {oid_name: value} de las queries SNMP.

        Returns:
            Datos normalizados de memoria en formato estándar NESS.
        """
        mem_total_bytes = safe_int(raw_data.get('mtxr_hl_total_memory'))
        mem_free_bytes = safe_int(raw_data.get('mtxr_hl_free_memory'))

        if mem_total_bytes > 0:
            mem_total_mb = round(mem_total_bytes / (1024.0 * 1024.0), 2)
            mem_free_mb = round(mem_free_bytes / (1024.0 * 1024.0), 2)
            mem_used_mb = round(mem_total_mb - mem_free_mb, 2)
            mem_usage_percent = round(
                calculate_percentage(mem_total_bytes - mem_free_bytes, mem_total_bytes), 2
            )

            return {
                "mem_usage_percent": mem_usage_percent,
                "mem_total_mb": mem_total_mb,
                "mem_used_mb": mem_used_mb,
                "mem_free_mb": mem_free_mb,
                "swap_total_mb": 0.0,   # RouterOS no tiene swap
                "swap_free_mb": 0.0,
            }
        else:
            return {"error": "No memory data from MIKROTIK-MIB"}

    # ==========================================================================
    # NORMALIZACIÓN DE DISCO
    # ==========================================================================

    def normalize_disk_data(
        self, raw_disk_entries: Dict[str, Dict[str, Any]]
    ) -> Dict[str, Dict[str, Any]]:
        """
        Normaliza datos de disco de MikroTik RouterOS.

        Usa HOST-RESOURCES-MIB hrStorageTable:
        - hrStorageDescr:           Descripción del storage
        - hrStorageAllocationUnits: Tamaño en bytes de cada unidad
        - hrStorageSize:            Tamaño total en unidades
        - hrStorageUsed:            Uso en unidades

        Total bytes = hrStorageSize * hrStorageAllocationUnits
        Used bytes  = hrStorageUsed * hrStorageAllocationUnits

        Filtra storage de tipo memoria RAM (por descripción) y
        solo muestra discos/flash.

        Args:
            raw_disk_entries: Datos del bulk scan: {idx: {oid_name: value}}.

        Returns:
            Diccionario normalizado de discos.
        """
        disk_data: Dict[str, Dict[str, Any]] = {}
        disk_counter = 0

        # Filtrar keywords que indican RAM/memoria (no disco)
        ram_keywords = ('real memory', 'ram', 'virtual memory', 'swap', 'memory buffers')

        for idx, raw in raw_disk_entries.items():
            descr = str(raw.get('hr_storage_descr', '')).strip()
            alloc_units = safe_int(raw.get('hr_storage_allocation'))
            size_units = safe_int(raw.get('hr_storage_size'))
            used_units = safe_int(raw.get('hr_storage_used'))

            # Omitir si no hay datos válidos
            if alloc_units <= 0 or size_units <= 0:
                continue

            # Omitir entries que son RAM, no disco
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

        if not disk_data:
            logger.debug("MikroTik: hrStorageTable no retornó discos, "
                         "datos de disco se intentarán via vendor-specific")

        return disk_data

    # ==========================================================================
    # DATOS ESPECÍFICOS DE MikroTik (MIKROTIK-MIB)
    # ==========================================================================

    async def collect_vendor_specific_data(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta datos específicos de MikroTik RouterOS.

        Obtiene:
        - Información del sistema (firmware, serial, licencia, board)
        - Health: temperatura, voltaje, corriente, ventiladores
        - CPU detallado: promedio real de todos los cores (hrProcessorLoad walk)
        - Disco fallback: mtxrHlDiskTotal/Used si hrStorageTable falló
        - Wireless: clientes conectados
        - Interfaces: cantidad total

        Args:
            client: Instancia de SnmpClient conectada al MikroTik.

        Returns:
            Diccionario con datos de MIKROTIK-MIB.
        """
        logger.info("Recolectando datos específicos de MikroTik RouterOS...")
        vendor_oids = self.get_vendor_oids()

        mikrotik_data: Dict[str, Any] = {
            "system_info": {},
            "health": {},
            "cpu_detailed": {},
            "disk_fallback": {},
            "wireless": {},
            "interfaces": {},
            "collection_timestamp": now_iso(),
        }

        # ===== Sistema: Firmware, Serial, Licencia, Board =====
        system_oids = (
            'mtxr_firmware_version', 'mtxr_license_id',
            'mtxr_serial_number', 'mtxr_firmware_upgrade_ver',
            'mtxr_board_name',
        )
        for oid_name in system_oids:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    mikrotik_data["system_info"][oid_name] = str(res.value)
                else:
                    mikrotik_data["system_info"][oid_name] = None

        # ===== Health: Temperatura, Voltaje, Corriente, Ventiladores =====
        health_oids = {
            'mtxr_hl_temperature':       ('temperature_celsius',      lambda v: round(safe_int(v) / 10.0, 1)),
            'mtxr_hl_processor_temp':    ('processor_temp_celsius',   lambda v: round(safe_int(v) / 10.0, 1)),
            'mtxr_hl_voltage':           ('voltage_volts',            lambda v: round(safe_int(v) / 10.0, 2)),
            'mtxr_hl_current':           ('current_ma',               lambda v: safe_int(v)),
            'mtxr_hl_power_consumption': ('power_watts',              lambda v: round(safe_int(v) / 10.0, 1)),
            'mtxr_hl_fan_speed1':        ('fan1_rpm',                 lambda v: safe_int(v)),
            'mtxr_hl_fan_speed2':        ('fan2_rpm',                 lambda v: safe_int(v)),
        }
        for oid_name, (field, converter) in health_oids.items():
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    mikrotik_data["health"][field] = converter(res.value)
                else:
                    mikrotik_data["health"][field] = None

        # ===== CPU Detallado: Promedio real de todos los cores =====
        mikrotik_data["cpu_detailed"] = await self._collect_cpu_per_core(client)

        # ===== Disco Fallback via MIKROTIK-MIB =====
        for oid_name in ('mtxr_hl_disk_total', 'mtxr_hl_disk_used'):
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    mikrotik_data["disk_fallback"][oid_name] = safe_int(res.value)
                else:
                    mikrotik_data["disk_fallback"][oid_name] = None

        # Calcular disco fallback si hay datos
        disk_total = mikrotik_data["disk_fallback"].get('mtxr_hl_disk_total') or 0
        disk_used = mikrotik_data["disk_fallback"].get('mtxr_hl_disk_used') or 0
        if disk_total > 0:
            mikrotik_data["disk_fallback"]["total_gb"] = round(disk_total / (1024.0 ** 3), 3)
            mikrotik_data["disk_fallback"]["used_gb"] = round(disk_used / (1024.0 ** 3), 3)
            mikrotik_data["disk_fallback"]["free_gb"] = round(
                max(0, disk_total - disk_used) / (1024.0 ** 3), 3
            )
            mikrotik_data["disk_fallback"]["percent_used"] = round(
                calculate_percentage(disk_used, disk_total), 2
            )

        # ===== Wireless: Clientes conectados =====
        mikrotik_data["wireless"] = await self._collect_wireless_clients(client, vendor_oids)

        # ===== Interfaces totales =====
        if_oid = vendor_oids.get('if_number')
        if if_oid:
            res = await client.get(if_oid)
            if not res.error and res.value is not None:
                mikrotik_data["interfaces"]["total_count"] = safe_int(res.value)

        logger.info("Datos específicos de MikroTik recolectados exitosamente")
        return mikrotik_data

    # --------------------------------------------------------------------------
    # HELPER: CPU por core (hrProcessorLoad bulk walk)
    # --------------------------------------------------------------------------

    async def _collect_cpu_per_core(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta carga de CPU por core via bulk walk de hrProcessorLoad.

        Retorna lista de cargas por core y el promedio global.
        """
        cpu_data: Dict[str, Any] = {
            "cores": [],
            "core_count": 0,
            "average_percent": 0.0,
        }

        try:
            cpu_oid = '1.3.6.1.2.1.25.3.3.1.2'  # hrProcessorLoad (tabla)
            results, error = await client.bulk(cpu_oid)
            if error or not results:
                return cpu_data

            loads = []
            for oid_str, value in results:
                load = safe_float(value)
                loads.append(load)
                cpu_data["cores"].append({
                    "index": oid_str.split('.')[-1],
                    "load_percent": round(load, 2),
                })

            if loads:
                cpu_data["core_count"] = len(loads)
                cpu_data["average_percent"] = round(sum(loads) / len(loads), 2)

        except Exception as e:
            logger.warning(f"Error recolectando CPU por core: {e}")

        return cpu_data

    # --------------------------------------------------------------------------
    # HELPER: Clientes wireless
    # --------------------------------------------------------------------------

    async def _collect_wireless_clients(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> Dict[str, Any]:
        """
        Recolecta información de clientes wireless conectados.

        Cuenta registrations en mtxrWlRtabAddr o usa mtxrWlApClientCount.
        """
        wireless_data: Dict[str, Any] = {
            "client_count": 0,
            "available": False,
        }

        try:
            # Intentar contar registrations via tabla
            reg_oid = vendor_oids.get('mtxr_wl_rt_tab_addr')
            if reg_oid:
                results, error = await client.bulk(reg_oid)
                if not error and results:
                    wireless_data["client_count"] = len(results)
                    wireless_data["available"] = True
                    return wireless_data

            # Fallback: usar AP client count
            ap_oid = vendor_oids.get('mtxr_wl_ap_client_count')
            if ap_oid:
                results, error = await client.bulk(ap_oid)
                if not error and results:
                    total = sum(safe_int(v) for _, v in results)
                    wireless_data["client_count"] = total
                    wireless_data["available"] = True

        except Exception as e:
            logger.warning(f"Error recolectando wireless clients: {e}")

        return wireless_data

    # ==========================================================================
    # DETECCIÓN AUTOMÁTICA
    # ==========================================================================

    @classmethod
    def matches_sys_object_id(cls, sys_object_id: str) -> bool:
        """
        Detecta si un sysObjectID corresponde a MikroTik.

        MikroTik usa enterprise OID 1.3.6.1.4.1.14988.
        RouterOS específicamente: 1.3.6.1.4.1.14988.1.*
        """
        if not sys_object_id:
            return False
        return any(
            sys_object_id.startswith(prefix)
            for prefix in MIKROTIK_SYS_OBJECT_IDS
        )
