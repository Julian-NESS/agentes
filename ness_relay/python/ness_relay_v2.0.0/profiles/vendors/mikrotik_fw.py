# ==============================================================================
# NESS Relay v2.0.0 - MikroTik Firewall Profile
# ==============================================================================
# Perfil de monitoreo para MikroTik RouterOS en modo Firewall/Gateway.
#
# Aplica a dispositivos MikroTik usados como firewall empresarial:
#   - CHR (Cloud Hosted Router) — VM/cloud
#   - CCR series (CCR2004, CCR2116, CCR1036, CCR1016...)
#   - RB series en uso como gateway (RB4011, RB3011, RB1100AHx4, L009)
#   - hAP/hEX series en redes SOHO
#
# NOTA SOBRE OIDs:
#   MikroTik NO cuenta con una MIB separada para firewalls. Todos los modelos
#   usan la misma MIKROTIK-MIB (Enterprise OID 1.3.6.1.4.1.14988). La
#   diferencia entre este perfil y 'mikrotik' (RouterOS) es funcional:
#   — Monitorea interfaces WAN/ISP por nombre de patrón (IF-MIB)
#   — Monitorea Netwatch (probes de conectividad configurados en RouterOS)
#   — Monitorea Queue Simple/Tree tables para tráfico por canal
#   — Construye resumen de canales de Internet (ISP detectado por nombre)
#
# CPU:     HOST-RESOURCES-MIB (hrProcessorLoad — promedio de cores)
# Memory:  MIKROTIK-MIB (mtxrHlTotalMemory / mtxrHlFreeMemory)
# Disk:    HOST-RESOURCES-MIB (hrStorageTable) + MIKROTIK-MIB fallback
# WAN:     IF-MIB (RFC 2863) — detectado por patrón en nombre de interfaz
# Netwatch: MIKROTIK-MIB 1.1.8 — probes de conectividad (ISP monitoring)
# Queues:  MIKROTIK-MIB 1.1.2 — tráfico por Queue Simple
#
# Enterprise OID base: 1.3.6.1.4.1.14988 (MikroTik)
# sysObjectID típico:  1.3.6.1.4.1.14988.1.*
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


def _snmp_addr_to_str(value) -> str:
    """
    Convierte un valor SNMP de dirección a string legible.

    MikroTik retorna src_addr/dst_addr de Queue Simple como OctetString
    conteniendo la IP en bytes crudos (4 bytes = IP, 8 bytes = IP+máscara).
    Ejemplo: bytes \xc0\xa8\x04\x00 → "192.168.4.0"

    PostgreSQL rechaza \u0000 (null byte) en campos JSONB, así que
    debemos convertir a notación decimal con puntos.
    """
    try:
        # pysnmp OctetString: .asNumbers() retorna tupla de enteros (bytes)
        if hasattr(value, 'asNumbers'):
            nums = value.asNumbers()
            if len(nums) == 4:
                return '.'.join(str(n) for n in nums)
            if len(nums) == 8:
                ip = '.'.join(str(n) for n in nums[:4])
                # Convertir máscara a CIDR
                mask_int = sum(bin(n).count('1') for n in nums[4:])
                return f"{ip}/{mask_int}"
            # Longitud no estándar: decodificar como texto
            if nums:
                # Si tiene null bytes, es data binaria → mostrar como hex
                if 0 in nums:
                    return '.'.join(str(n) for n in nums)
                return ''.join(chr(n) for n in nums)

        # prettyPrint() a veces funciona mejor para IpAddress
        if hasattr(value, 'prettyPrint'):
            pp = value.prettyPrint()
            if pp and '\x00' not in pp and '\u0000' not in pp:
                return pp

        # Fallback: str() con limpieza de null bytes
        s = str(value)
        return s.replace('\x00', '').replace('\u0000', '')
    except Exception:
        try:
            return str(value).replace('\x00', '').replace('\u0000', '')
        except Exception:
            return ''


# ==============================================================================
# OIDs DE PERFORMANCE (reutilizados de RouterOS)
# ==============================================================================

# CPU — HOST-RESOURCES-MIB (idéntico al perfil RouterOS)
MIKROTIK_FW_CPU_OIDS: Dict[str, str] = {
    'hr_processor_load': '1.3.6.1.2.1.25.3.3.1.2',  # hrProcessorLoad (tabla, % por core)
}

# Memoria — MikroTik NO tiene OIDs específicos de memoria en MIKROTIK-MIB.
# Los OIDs .3.12.0 y .3.13.0 son en realidad mtxrHlPowerConsumption y
# mtxrHlFanSpeed1 respectivamente. La memoria real se obtiene de
# HOST-RESOURCES-MIB hrStorageTable (entry con path "main memory").
# El método post_process_performance() extrae la memoria del disk data.
MIKROTIK_FW_MEMORY_OIDS: Dict[str, str] = {
    # Sin OIDs — memoria se extrae en post_process_performance desde hrStorageTable
}

# Disco — HOST-RESOURCES-MIB (hrStorageTable)
MIKROTIK_FW_DISK_OIDS: Dict[str, str] = {
    'hr_storage_descr':      '1.3.6.1.2.1.25.2.3.1.3',   # hrStorageDescr
    'hr_storage_allocation': '1.3.6.1.2.1.25.2.3.1.4',   # hrStorageAllocationUnits (bytes)
    'hr_storage_size':       '1.3.6.1.2.1.25.2.3.1.5',   # hrStorageSize (units)
    'hr_storage_used':       '1.3.6.1.2.1.25.2.3.1.6',   # hrStorageUsed (units)
}

# ==============================================================================
# OIDs DE MONITOREO WAN / CANALES DE INTERNET
# ==============================================================================

# IF-MIB RFC 2863 — Interfaces de red (estándar, igual que Fortinet)
MIKROTIK_FW_INTERFACE_OIDS: Dict[str, str] = {
    'if_descr':              '1.3.6.1.2.1.2.2.1.2',           # ifDescr
    'if_type':               '1.3.6.1.2.1.2.2.1.3',           # ifType
    'if_speed':              '1.3.6.1.2.1.2.2.1.5',           # ifSpeed (bps)
    'if_admin_status':       '1.3.6.1.2.1.2.2.1.7',           # ifAdminStatus (1=up, 2=down)
    'if_oper_status':        '1.3.6.1.2.1.2.2.1.8',           # ifOperStatus (1=up, 2=down)
    'if_in_octets':          '1.3.6.1.2.1.2.2.1.10',          # ifInOctets
    'if_out_octets':         '1.3.6.1.2.1.2.2.1.16',          # ifOutOctets
    'if_in_errors':          '1.3.6.1.2.1.2.2.1.14',          # ifInErrors
    'if_out_errors':         '1.3.6.1.2.1.2.2.1.20',          # ifOutErrors
    'if_in_discards':        '1.3.6.1.2.1.2.2.1.13',          # ifInDiscards
    'if_out_discards':       '1.3.6.1.2.1.2.2.1.19',          # ifOutDiscards
    # IF-MIB HC (64-bit counters — imprescindible en links de alta velocidad)
    'if_hc_in_octets':       '1.3.6.1.2.1.31.1.1.1.6',        # ifHCInOctets
    'if_hc_out_octets':      '1.3.6.1.2.1.31.1.1.1.10',       # ifHCOutOctets
    'if_high_speed':         '1.3.6.1.2.1.31.1.1.1.15',       # ifHighSpeed (Mbps)
    'if_name':               '1.3.6.1.2.1.31.1.1.1.1',        # ifName (alias corto)
    'if_alias':              '1.3.6.1.2.1.31.1.1.1.18',       # ifAlias (descripción user)
    'if_hc_in_ucast_pkts':   '1.3.6.1.2.1.31.1.1.1.7',        # ifHCInUcastPkts
    'if_hc_out_ucast_pkts':  '1.3.6.1.2.1.31.1.1.1.11',       # ifHCOutUcastPkts
}

# ==============================================================================
# OIDs DE NETWATCH (MIKROTIK-MIB) — Probes de conectividad
# ==============================================================================
# Netwatch es la función nativa de RouterOS para monitorear conectividad.
# El administrador configura entradas (host/IP a testear + intervalo + timeout).
# Cada entrada reporta su estado (up/down) via SNMP.
#
# Árbol OID: 1.3.6.1.4.1.14988.1.1.8
#  .1 = mtxrNetwatchTable
#   .1 = mtxrNetwatchEntry
#    .1 = mtxrNetwatchIndex
#    .2 = mtxrNetwatchName        (DisplayString — nombre del probe)
#    .3 = mtxrNetwatchIp          (IpAddress — IP del host monitoreado)
#    .4 = mtxrNetwatchInterval    (Gauge32 — intervalo en ms)
#    .5 = mtxrNetwatchTimeout     (Gauge32 — timeout en ms)
#    .6 = mtxrNetwatchStatus      (INTEGER: 1=up, 2=down, 3=unknown)

MIKROTIK_FW_NETWATCH_OIDS: Dict[str, str] = {
    'netwatch_name':     '1.3.6.1.4.1.14988.1.1.8.1.1.2',   # mtxrNetwatchName
    'netwatch_ip':       '1.3.6.1.4.1.14988.1.1.8.1.1.3',   # mtxrNetwatchIp
    'netwatch_interval': '1.3.6.1.4.1.14988.1.1.8.1.1.4',   # mtxrNetwatchInterval (ms)
    'netwatch_timeout':  '1.3.6.1.4.1.14988.1.1.8.1.1.5',   # mtxrNetwatchTimeout (ms)
    'netwatch_status':   '1.3.6.1.4.1.14988.1.1.8.1.1.6',   # mtxrNetwatchStatus (1=up, 2=down)
}

# ==============================================================================
# OIDs DE QUEUE SIMPLE TABLE (MIKROTIK-MIB) — Gestión de ancho de banda
# ==============================================================================
# Queue Simple es la forma más común de gestión de tráfico en RouterOS.
# Permite monitorear el ancho de banda asignado/utilizado por dirección o
# interfaz. Útil para ver el throughput real de cada ISP/canal.
#
# Árbol OID: 1.3.6.1.4.1.14988.1.1.2.1
#  .1 = mtxrQueueSimpleTable
#   .1 = mtxrQueueSimpleEntry
#    .1 = mtxrQueueSimpleIndex
#    .2 = mtxrQueueSimpleName       (DisplayString — nombre de la cola)
#    .3 = mtxrQueueSimpleSrcAddr    (DisplayString — dirección fuente)
#    .4 = mtxrQueueSimpleDstAddr    (DisplayString — dirección destino)
#    .5 = mtxrQueueSimpleIface      (DisplayString — interfaz aplicada)
#    .6 = mtxrQueueSimplePriority   (Gauge32 — prioridad 1-8)
#    .7 = mtxrQueueSimpleTxByte     (Counter64 — bytes TX)
#    .8 = mtxrQueueSimpleTxPacket   (Counter64 — paquetes TX)
#    .9 = mtxrQueueSimpleRxByte     (Counter64 — bytes RX)
#   .10 = mtxrQueueSimpleRxPacket   (Counter64 — paquetes RX)
#   .11 = mtxrQueueSimpleTxDrop     (Counter64 — drops TX)
#   .12 = mtxrQueueSimpleRxDrop     (Counter64 — drops RX)

MIKROTIK_FW_QUEUE_OIDS: Dict[str, str] = {
    'queue_name':        '1.3.6.1.4.1.14988.1.1.2.1.1.2',   # mtxrQueueSimpleName
    'queue_src_addr':    '1.3.6.1.4.1.14988.1.1.2.1.1.3',   # mtxrQueueSimpleSrcAddr
    'queue_dst_addr':    '1.3.6.1.4.1.14988.1.1.2.1.1.4',   # mtxrQueueSimpleDstAddr
    'queue_iface':       '1.3.6.1.4.1.14988.1.1.2.1.1.5',   # mtxrQueueSimpleIface
    'queue_tx_bytes':    '1.3.6.1.4.1.14988.1.1.2.1.1.7',   # mtxrQueueSimpleTxByte
    'queue_tx_packets':  '1.3.6.1.4.1.14988.1.1.2.1.1.8',   # mtxrQueueSimpleTxPacket
    'queue_rx_bytes':    '1.3.6.1.4.1.14988.1.1.2.1.1.9',   # mtxrQueueSimpleRxByte
    'queue_rx_packets':  '1.3.6.1.4.1.14988.1.1.2.1.1.10',  # mtxrQueueSimpleRxPacket
    'queue_tx_drop':     '1.3.6.1.4.1.14988.1.1.2.1.1.11',  # mtxrQueueSimpleTxDrop
    'queue_rx_drop':     '1.3.6.1.4.1.14988.1.1.2.1.1.12',  # mtxrQueueSimpleRxDrop
}

# ==============================================================================
# OIDs DE SISTEMA / HEALTH (reutilizados de RouterOS)
# ==============================================================================

MIKROTIK_FW_VENDOR_OIDS: Dict[str, str] = {
    # Firmware / Sistema
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

    # Disco fallback via MIKROTIK-MIB
    'mtxr_hl_disk_total':        '1.3.6.1.4.1.14988.1.1.3.1.0',   # mtxrHlDiskTotal (bytes)
    'mtxr_hl_disk_used':         '1.3.6.1.4.1.14988.1.1.3.2.0',   # mtxrHlDiskUsed (bytes)

    # Interfaces totales
    'if_number':                 '1.3.6.1.2.1.2.1.0',              # ifNumber

    # WAN/Interfaces + Netwatch + Queues
    **MIKROTIK_FW_INTERFACE_OIDS,
    **MIKROTIK_FW_NETWATCH_OIDS,
    **MIKROTIK_FW_QUEUE_OIDS,
}

# ==============================================================================
# PATRONES DE DETECCIÓN DE INTERFACES WAN EN MIKROTIK
# ==============================================================================
# RouterOS usa nomenclatura libre para interfaces. Los patrones más comunes
# en despliegues empresariales colombianos son los siguientes.

WAN_INTERFACE_PATTERNS = [
    # Ethernet física — ether1 casi siempre es WAN en equipos SOHO/enterprise
    'ether1', 'sfp1', 'sfp-sfpplus1', 'sfpplus1',
    # Nombres descriptivos de WAN
    'wan', 'internet', 'isp', 'uplink', 'externa', 'external',
    'primary', 'secondary', 'backup', 'principal', 'respaldo',
    # PPPoE / DHCP WAN
    'pppoe', 'pppoe-out', 'pppoe-wan', 'dhcp-wan',
    # VLANs de WAN (ej. vlan200-wan, vlan.100)
    'vlan-wan', 'vlan_wan',
    # Proveedores Colombia y LATAM (nombre de ISP en alias)
    'etb', 'tigo', 'claro', 'movistar', 'azteca', 'att',
    'une', 'millicom', 'telefonica', 'supercanal', 'edatel',
    'fibra', 'fiber', 'dsl', 'adsl', 'cable', 'mpls',
    'lte', '4g', '5g', 'celular',
]

# sysObjectID para MikroTik (mismos que RouterOS — no hay OID diferente)
MIKROTIK_FW_SYS_OBJECT_IDS = [
    '1.3.6.1.4.1.14988.1',
    '1.3.6.1.4.1.14988',
]


# ==============================================================================
# PROFILE CLASS
# ==============================================================================

class MikroTikFirewallProfile(BaseDeviceProfile):
    """
    Perfil de dispositivo para MikroTik RouterOS en modo Firewall/Gateway.

    Diseñado para CCR, CHR, RB4011, RB3011, RB1100, L009 y cualquier
    equipo MikroTik que actúe como firewall perimetral / gateway de Internet.

    Características adicionales vs MikroTikProfile (RouterOS):
    - Tipo de dispositivo: 'firewall' (en lugar de 'router')
    - Monitoreo de interfaces WAN por patrón de nombre  
    - Monitoreo de Netwatch (probes de conectividad → estado de ISPs)
    - Monitoreo de Queue Simple Table (consumo de ancho de banda por canal)
    - Resumen de canales de Internet con detección de ISP (ETB, Tigo, Claro)
    - Indicadores de calidad: estado up/down por canal, drops por queue
    """

    vendor = "mikrotik_fw"
    vendor_display_name = "MikroTik Firewall (RouterOS)"
    device_type = "firewall"

    # ==========================================================================
    # OIDs
    # ==========================================================================

    def get_vendor_oids(self) -> Dict[str, str]:
        """Retorna OIDs de MIKROTIK-MIB + IF-MIB + Netwatch + Queues."""
        return MIKROTIK_FW_VENDOR_OIDS.copy()

    def get_cpu_oids(self) -> Dict[str, str]:
        return MIKROTIK_FW_CPU_OIDS.copy()

    def get_memory_oids(self) -> Dict[str, str]:
        return MIKROTIK_FW_MEMORY_OIDS.copy()

    def get_disk_oids(self) -> Dict[str, str]:
        return MIKROTIK_FW_DISK_OIDS.copy()

    # ==========================================================================
    # NORMALIZACIÓN DE CPU
    # ==========================================================================

    def normalize_cpu_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza CPU de MikroTik (hrProcessorLoad — igual que RouterOS).

        hrProcessorLoad es una tabla; el performance_collector consulta
        con get() y retorna el primer core. El valor real multi-core se
        obtiene en collect_vendor_specific_data via bulk walk.
        """
        raw_cpu = raw_data.get('hr_processor_load')
        cpu_usage = safe_float(raw_cpu) if raw_cpu is not None else 0.0

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
        Normaliza memoria de MikroTik.

        MikroTik NO tiene OIDs de memoria en MIKROTIK-MIB.
        La memoria real se extrae en post_process_performance()
        desde hrStorageTable (entry "main memory" en el disk data).
        Aquí retornamos ceros como placeholder — serán sobreescritos.
        """
        return {
            "mem_usage_percent": 0.0,
            "mem_total_mb": 0.0,
            "mem_used_mb": 0.0,
            "mem_free_mb": 0.0,
            "swap_total_mb": 0.0,
            "swap_free_mb": 0.0,
        }

    # ==========================================================================
    # NORMALIZACIÓN DE DISCO
    # ==========================================================================

    def normalize_disk_data(
        self, raw_disk_entries: Dict[str, Dict[str, Any]]
    ) -> Dict[str, Dict[str, Any]]:
        """
        Normaliza disco via HOST-RESOURCES-MIB hrStorageTable.
        Filtra entradas de RAM/swap; solo muestra NAND/flash/disk.
        """
        disk_data: Dict[str, Dict[str, Any]] = {}
        disk_counter = 0
        ram_keywords = ('real memory', 'ram', 'virtual memory', 'swap', 'memory buffers')

        for idx, raw in raw_disk_entries.items():
            descr = str(raw.get('hr_storage_descr', '')).strip()
            alloc_units = safe_int(raw.get('hr_storage_allocation'))
            size_units = safe_int(raw.get('hr_storage_size'))
            used_units = safe_int(raw.get('hr_storage_used'))

            if alloc_units <= 0 or size_units <= 0:
                continue
            if any(kw in descr.lower() for kw in ram_keywords):
                continue

            total_bytes = size_units * alloc_units
            used_bytes = used_units * alloc_units
            free_bytes = max(0, total_bytes - used_bytes)

            disk_counter += 1
            disk_data[str(disk_counter)] = {
                "index": str(disk_counter),
                "path": descr or f"storage_{idx}",
                "total_gb": round(total_bytes / (1024.0 ** 3), 3),
                "used_gb": round(used_bytes / (1024.0 ** 3), 3),
                "free_gb": round(free_bytes / (1024.0 ** 3), 3),
                "percent_used": round(calculate_percentage(used_bytes, total_bytes), 2),
            }

        if not disk_data:
            logger.debug("MikroTik FW: hrStorageTable sin discos, intentando MIKROTIK-MIB fallback")

        return disk_data

    # ==========================================================================
    # POST-PROCESAMIENTO: Extraer memoria de hrStorageTable
    # ==========================================================================

    def post_process_performance(self, perf_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Extrae datos de memoria desde hrStorageTable ('main memory').

        MikroTik RouterOS NO tiene OIDs de memoria en MIKROTIK-MIB.
        En su lugar, reporta memoria como una entrada de hrStorageTable
        con hrStorageDescr = 'main memory'. Esta entrada ya fue recolectada
        por performance_collector como si fuera un 'disco'.

        Este método:
        1. Busca la entrada 'main memory' en perf_data['disk']
        2. La convierte a datos de memoria (MB, porcentaje)
        3. La elimina de disk para que no aparezca como disco
        """
        mem_data = perf_data.get("memory", {})
        disk_data = perf_data.get("disk", {})

        # Solo corregir si la memoria está vacía o en ceros
        mem_total = mem_data.get("mem_total_mb", 0)
        if isinstance(mem_total, (int, float)) and mem_total > 0:
            return perf_data  # Memoria ya tiene datos válidos

        # Buscar 'main memory' en las entradas de disco
        memory_key = None
        for idx, disk_entry in disk_data.items():
            path = str(disk_entry.get("path", "")).lower()
            if "main memory" in path or (path == "memory" and disk_entry.get("total_gb", 0) > 0.1):
                memory_key = idx
                break

        if memory_key:
            mem_entry = disk_data[memory_key]
            total_gb = mem_entry.get("total_gb", 0)
            used_gb = mem_entry.get("used_gb", 0)
            free_gb = mem_entry.get("free_gb", 0)
            percent = mem_entry.get("percent_used", 0)

            perf_data["memory"] = {
                "mem_usage_percent": round(percent, 2),
                "mem_total_mb": round(total_gb * 1024.0, 2),
                "mem_used_mb": round(used_gb * 1024.0, 2),
                "mem_free_mb": round(free_gb * 1024.0, 2),
                "swap_total_mb": 0.0,
                "swap_free_mb": 0.0,
            }

            # Eliminar de disco para que no aparezca como almacenamiento
            del disk_data[memory_key]

            # Re-indexar discos restantes
            remaining = list(disk_data.values())
            perf_data["disk"] = {}
            for i, entry in enumerate(remaining, 1):
                entry["index"] = str(i)
                perf_data["disk"][str(i)] = entry

            logger.info(
                f"MikroTik FW: Memoria extraída de hrStorageTable — "
                f"{perf_data['memory']['mem_total_mb']:.0f} MB total, "
                f"{perf_data['memory']['mem_usage_percent']:.1f}% usado"
            )

        return perf_data

    # ==========================================================================
    # FINALIZACIÓN: Enriquecer performance con datos vendor-specific
    # ==========================================================================

    def finalize_collected_data(self, all_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Post-procesamiento final tras recolectar TODO (incluyendo vendor data).

        Corrige CPU de performance usando cpu_detailed del vendor cuando
        hrProcessorLoad (table OID via GET) retorna 0. En MikroTik,
        hrProcessorLoad es una TABLA — el GET escalar no funciona en SNMPv1,
        pero collect_vendor_specific_data sí hace bulk walk de todos los cores.
        """
        vendor_key = f"{self.vendor}_specific"
        vendor_data = all_data.get(vendor_key, {})
        perf_data = all_data.get("performance", {})

        # --- Corregir CPU ---
        cpu_data = perf_data.get("cpu", {})
        current_cpu = cpu_data.get("cpu_usage_percent", 0)

        if isinstance(current_cpu, (int, float)) and current_cpu == 0:
            cpu_detailed = vendor_data.get("cpu_detailed", {})
            avg_cpu = cpu_detailed.get("average_percent", 0)
            if isinstance(avg_cpu, (int, float)) and avg_cpu > 0:
                cpu_data["cpu_usage_percent"] = round(avg_cpu, 2)
                logger.info(
                    f"MikroTik FW: CPU corregido de vendor data — "
                    f"{avg_cpu:.2f}% (promedio de {cpu_detailed.get('core_count', '?')} cores)"
                )

        return all_data

    # ==========================================================================
    # DATOS ESPECÍFICOS — Firewall + Canales de Internet
    # ==========================================================================

    async def collect_vendor_specific_data(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta datos específicos de MikroTik Firewall.

        Estructura del resultado:
        {
          "system_info":       { firmware, serial, license, board_name },
          "health":            { temperatura, voltaje, corriente, fans },
          "cpu_detailed":      { cores[], core_count, average_percent },
          "disk_fallback":     { total_gb, used_gb... si hrStorageTable falla },
          "interfaces_total":  int,
          "wan_interfaces":    [ {name, oper_status, speed_mbps, ...} ],
          "netwatch":          { probes[], summary: {total, up, down}, available },
          "queues":            { entries[], summary: {total, total_tx_mb, ...}, available },
          "internet_channels": { channels[], summary: {total, up, down, ...} },
          "collection_timestamp": ISO8601
        }

        Args:
            client: Instancia de SnmpClient conectada al MikroTik.

        Returns:
            Diccionario con todos los datos del firewall MikroTik.
        """
        logger.info("Recolectando datos específicos de MikroTik Firewall...")
        vendor_oids = self.get_vendor_oids()

        fw_data: Dict[str, Any] = {
            "system_info": {},
            "health": {},
            "cpu_detailed": {},
            "disk_fallback": {},
            "interfaces_total": 0,
            "wan_interfaces": [],
            "netwatch": {},
            "queues": {},
            "internet_channels": {},
            "collection_timestamp": now_iso(),
        }

        # ===== Sistema: Firmware, Serial, Board =====
        system_oids = (
            'mtxr_firmware_version', 'mtxr_license_id',
            'mtxr_serial_number', 'mtxr_firmware_upgrade_ver',
            'mtxr_board_name',
        )
        for oid_name in system_oids:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                fw_data["system_info"][oid_name] = (
                    str(res.value) if not res.error and res.value is not None else None
                )

        # ===== Health: Temperatura, Voltaje, Corriente, Fans =====
        health_oids = {
            'mtxr_hl_temperature':       ('temperature_celsius',    lambda v: round(safe_int(v) / 10.0, 1)),
            'mtxr_hl_processor_temp':    ('processor_temp_celsius', lambda v: round(safe_int(v) / 10.0, 1)),
            'mtxr_hl_voltage':           ('voltage_volts',          lambda v: round(safe_int(v) / 10.0, 2)),
            'mtxr_hl_current':           ('current_ma',             lambda v: safe_int(v)),
            'mtxr_hl_power_consumption': ('power_watts',            lambda v: round(safe_int(v) / 10.0, 1)),
            'mtxr_hl_fan_speed1':        ('fan1_rpm',               lambda v: safe_int(v)),
            'mtxr_hl_fan_speed2':        ('fan2_rpm',               lambda v: safe_int(v)),
        }
        for oid_name, (field, converter) in health_oids.items():
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                fw_data["health"][field] = (
                    converter(res.value) if not res.error and res.value is not None else None
                )

        # ===== CPU Detallado (bulk walk hrProcessorLoad) =====
        fw_data["cpu_detailed"] = await self._collect_cpu_per_core(client)

        # ===== Disco Fallback via MIKROTIK-MIB =====
        for oid_name in ('mtxr_hl_disk_total', 'mtxr_hl_disk_used'):
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                fw_data["disk_fallback"][oid_name] = (
                    safe_int(res.value) if not res.error and res.value is not None else None
                )

        disk_total = fw_data["disk_fallback"].get('mtxr_hl_disk_total') or 0
        disk_used = fw_data["disk_fallback"].get('mtxr_hl_disk_used') or 0
        if disk_total > 0:
            fw_data["disk_fallback"]["total_gb"] = round(disk_total / (1024.0 ** 3), 3)
            fw_data["disk_fallback"]["used_gb"] = round(disk_used / (1024.0 ** 3), 3)
            fw_data["disk_fallback"]["free_gb"] = round(
                max(0, disk_total - disk_used) / (1024.0 ** 3), 3
            )
            fw_data["disk_fallback"]["percent_used"] = round(
                calculate_percentage(disk_used, disk_total), 2
            )

        # ===== Total de interfaces =====
        if_oid = vendor_oids.get('if_number')
        if if_oid:
            res = await client.get(if_oid)
            if not res.error and res.value is not None:
                fw_data["interfaces_total"] = safe_int(res.value)

        # =====================================================================
        # MONITOREO DE CANALES DE INTERNET / WAN
        # =====================================================================

        logger.info("Recolectando interfaces WAN y canales de Internet...")

        # 1. Interfaces WAN (IF-MIB — detectadas por patrón de nombre)
        fw_data["wan_interfaces"] = await self._collect_wan_interfaces(client, vendor_oids)

        # 2. Netwatch (probes de conectividad — estado up/down de ISPs)
        fw_data["netwatch"] = await self._collect_netwatch(client, vendor_oids)

        # 3. Queue Simple Table (tráfico por cola de ancho de banda)
        fw_data["queues"] = await self._collect_queues(client, vendor_oids)

        # 4. Resumen de canales de Internet (combina WAN + Netwatch + Queues)
        fw_data["internet_channels"] = self._build_internet_channels_summary(
            fw_data["wan_interfaces"],
            fw_data["netwatch"],
            fw_data["queues"],
        )

        logger.info("Datos específicos de MikroTik Firewall recolectados exitosamente")
        return fw_data

    # ==========================================================================
    # HELPER: CPU por core (hrProcessorLoad bulk walk)
    # ==========================================================================

    async def _collect_cpu_per_core(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta carga por core via bulk walk de hrProcessorLoad.
        Retorna promedio global y detalle por core.
        """
        cpu_data: Dict[str, Any] = {
            "cores": [],
            "core_count": 0,
            "average_percent": 0.0,
        }
        try:
            results, error = await client.bulk('1.3.6.1.2.1.25.3.3.1.2')
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
            logger.warning(f"MikroTik FW: Error recolectando CPU por core: {e}")

        return cpu_data

    # ==========================================================================
    # HELPER: Interfaces WAN (IF-MIB)
    # ==========================================================================

    async def _collect_wan_interfaces(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> List[Dict[str, Any]]:
        """
        Detecta y recolecta métricas de interfaces WAN via IF-MIB.

        La detección se basa en patrones en ifDescr, ifName e ifAlias:
        - Nombres físicos: ether1, sfp1, pppoe-out1, etc.
        - Nombres descriptivos: wan, internet, etb, tigo, claro, etc.
        - Alias configurados por el administrador

        Returns:
            Lista de interfaces WAN con métricas detalladas.
        """
        wan_interfaces: List[Dict[str, Any]] = []

        try:
            # Obtener descripciones de todas las interfaces
            if_descr_oid = vendor_oids.get('if_descr')
            if not if_descr_oid:
                return wan_interfaces

            descr_results, error = await client.bulk(if_descr_oid)
            if error or not descr_results:
                logger.debug(f"MikroTik FW: ifDescr no disponible: {error}")
                return wan_interfaces

            # Construir mapa inicial de interfaces
            interface_map: Dict[str, Dict[str, Any]] = {}
            for oid_str, value in descr_results:
                idx = oid_str.split('.')[-1]
                if_name = str(value).strip()
                is_wan = any(p in if_name.lower() for p in WAN_INTERFACE_PATTERNS)

                interface_map[idx] = {
                    "index": idx,
                    "name": if_name,
                    "if_name": None,
                    "alias": None,
                    "is_wan": is_wan,
                    "admin_status": "unknown",
                    "oper_status": "unknown",
                    "speed_mbps": 0,
                    "traffic_in_bytes": 0,
                    "traffic_out_bytes": 0,
                    "traffic_in_mb": 0.0,
                    "traffic_out_mb": 0.0,
                    "errors_in": 0,
                    "errors_out": 0,
                    "discards_in": 0,
                    "discards_out": 0,
                    "packets_in": 0,
                    "packets_out": 0,
                    "isp_detected": self._detect_isp_from_name(if_name),
                }

            # Añadir ifName (alias corto de RouterOS, ej. "ether1", "pppoe-out1")
            if_name_oid = vendor_oids.get('if_name')
            if if_name_oid:
                name_results, _ = await client.bulk(if_name_oid)
                for oid_str, value in (name_results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in interface_map:
                        short_name = str(value).strip()
                        interface_map[idx]["if_name"] = short_name
                        # ifName puede revelar WAN: ether1, pppoe-out, etc.
                        if any(p in short_name.lower() for p in WAN_INTERFACE_PATTERNS):
                            interface_map[idx]["is_wan"] = True
                        if not interface_map[idx]["isp_detected"]:
                            interface_map[idx]["isp_detected"] = self._detect_isp_from_name(short_name)

            # Añadir ifAlias (descripción libre del administrador — suele tener el ISP)
            if_alias_oid = vendor_oids.get('if_alias')
            if if_alias_oid:
                alias_results, _ = await client.bulk(if_alias_oid)
                for oid_str, value in (alias_results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in interface_map:
                        alias = str(value).strip()
                        if alias:
                            interface_map[idx]["alias"] = alias
                            if any(p in alias.lower() for p in WAN_INTERFACE_PATTERNS):
                                interface_map[idx]["is_wan"] = True
                            if not interface_map[idx]["isp_detected"]:
                                interface_map[idx]["isp_detected"] = self._detect_isp_from_name(alias)

            # Recolectar métricas solo para interfaces WAN identificadas
            bulk_columns = {
                'if_admin_status':      ('admin_status',     lambda v: "UP" if str(v) == '1' else "DOWN"),
                'if_oper_status':       ('oper_status',      lambda v: "UP" if str(v) == '1' else "DOWN"),
                'if_high_speed':        ('speed_mbps',       lambda v: safe_int(v)),
                'if_hc_in_octets':      ('traffic_in_bytes', lambda v: safe_int(v)),
                'if_hc_out_octets':     ('traffic_out_bytes',lambda v: safe_int(v)),
                'if_in_errors':         ('errors_in',        lambda v: safe_int(v)),
                'if_out_errors':        ('errors_out',       lambda v: safe_int(v)),
                'if_in_discards':       ('discards_in',      lambda v: safe_int(v)),
                'if_out_discards':      ('discards_out',     lambda v: safe_int(v)),
                'if_hc_in_ucast_pkts':  ('packets_in',       lambda v: safe_int(v)),
                'if_hc_out_ucast_pkts': ('packets_out',      lambda v: safe_int(v)),
            }

            for oid_name, (field, converter) in bulk_columns.items():
                oid = vendor_oids.get(oid_name)
                if not oid:
                    continue
                results, _ = await client.bulk(oid)
                for oid_str, value in (results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in interface_map and interface_map[idx]["is_wan"]:
                        interface_map[idx][field] = converter(value)

            # Fallback: if_speed (bps) si ifHighSpeed no está disponible
            if_speed_oid = vendor_oids.get('if_speed')
            if if_speed_oid:
                speed_results, _ = await client.bulk(if_speed_oid)
                for oid_str, value in (speed_results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in interface_map and interface_map[idx]["is_wan"]:
                        if interface_map[idx]["speed_mbps"] == 0:
                            interface_map[idx]["speed_mbps"] = round(safe_int(value) / 1_000_000, 2)

            # Calcular campos derivados y filtrar solo WAN
            for data in interface_map.values():
                if not data["is_wan"]:
                    continue
                data["traffic_in_mb"] = round(data["traffic_in_bytes"] / (1024.0 * 1024.0), 2)
                data["traffic_out_mb"] = round(data["traffic_out_bytes"] / (1024.0 * 1024.0), 2)
                wan_interfaces.append(data)

            logger.info(f"MikroTik FW: {len(wan_interfaces)} interfaces WAN detectadas")

        except Exception as e:
            logger.warning(f"MikroTik FW: Error recolectando interfaces WAN: {e}")

        return wan_interfaces

    # ==========================================================================
    # HELPER: Netwatch — Probes de conectividad
    # ==========================================================================

    async def _collect_netwatch(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> Dict[str, Any]:
        """
        Recolecta estado de probes Netwatch configurados en RouterOS.

        Netwatch es el equivalente de MikroTik al SD-WAN health check de
        FortiGate: el administrador configura pings/probes hacia IPs de
        referencia por ISP (ej: 8.8.8.8 via ETB, 8.8.4.4 via Tigo).

        SNMP retorna el estado up/down de cada probe.

        Returns:
            Diccionario con probes y resumen de disponibilidad.
        """
        netwatch_data: Dict[str, Any] = {
            "probes": [],
            "summary": {
                "total": 0,
                "up": 0,
                "down": 0,
                "availability_percent": None,
            },
            "available": False,
        }

        try:
            name_oid = vendor_oids.get('netwatch_name')
            if not name_oid:
                return netwatch_data

            name_results, error = await client.bulk(name_oid)
            if error or not name_results:
                logger.debug("MikroTik FW: Netwatch no configurado o no accesible via SNMP")
                return netwatch_data

            # Construir mapa de probes
            probe_map: Dict[str, Dict[str, Any]] = {}
            for oid_str, value in name_results:
                idx = oid_str.split('.')[-1]
                probe_map[idx] = {
                    "index": idx,
                    "name": str(value).strip(),
                    "target_ip": None,
                    "interval_ms": None,
                    "timeout_ms": None,
                    "status": "unknown",
                    "status_text": "unknown",
                    "isp_detected": self._detect_isp_from_name(str(value)),
                }

            if not probe_map:
                return netwatch_data

            netwatch_data["available"] = True

            # Recolectar IP del target
            ip_oid = vendor_oids.get('netwatch_ip')
            if ip_oid:
                ip_results, _ = await client.bulk(ip_oid)
                for oid_str, value in (ip_results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in probe_map:
                        probe_map[idx]["target_ip"] = str(value)

            # Recolectar intervalo y timeout
            for oid_key, field in [('netwatch_interval', 'interval_ms'), ('netwatch_timeout', 'timeout_ms')]:
                oid = vendor_oids.get(oid_key)
                if oid:
                    results, _ = await client.bulk(oid)
                    for oid_str, value in (results or []):
                        idx = oid_str.split('.')[-1]
                        if idx in probe_map:
                            probe_map[idx][field] = safe_int(value)

            # Recolectar estado (1=up, 2=down)
            status_oid = vendor_oids.get('netwatch_status')
            if status_oid:
                status_results, _ = await client.bulk(status_oid)
                for oid_str, value in (status_results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in probe_map:
                        raw_status = safe_int(value)
                        probe_map[idx]["status"] = raw_status
                        probe_map[idx]["status_text"] = (
                            "up" if raw_status == 1 else
                            "down" if raw_status == 2 else
                            "unknown"
                        )

            # Calcular summary
            up_count = sum(1 for p in probe_map.values() if p.get("status") == 1)
            down_count = sum(1 for p in probe_map.values() if p.get("status") == 2)
            total = len(probe_map)

            netwatch_data["probes"] = list(probe_map.values())
            netwatch_data["summary"] = {
                "total": total,
                "up": up_count,
                "down": down_count,
                "availability_percent": (
                    round((up_count / total) * 100.0, 1) if total > 0 else None
                ),
            }

            logger.info(f"MikroTik FW Netwatch: {up_count}/{total} probes UP")

        except Exception as e:
            logger.warning(f"MikroTik FW: Error recolectando Netwatch: {e}")

        return netwatch_data

    # ==========================================================================
    # HELPER: Queue Simple Table — Ancho de banda por cola
    # ==========================================================================

    async def _collect_queues(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> Dict[str, Any]:
        """
        Recolecta estadísticas de Queue Simple Table (MIKROTIK-MIB).

        Queue Simple permite asignar límites de ancho de banda por
        dirección IP o interfaz. Las estadísticas de TX/RX bytes
        revelan el consumo real de ancho de banda por canal WAN.

        Returns:
            Diccionario con entradas de queue y resumen de tráfico.
        """
        queues_data: Dict[str, Any] = {
            "entries": [],
            "summary": {
                "total_queues": 0,
                "total_tx_gb": 0.0,
                "total_rx_gb": 0.0,
                "total_tx_drops": 0,
                "total_rx_drops": 0,
            },
            "available": False,
        }

        try:
            name_oid = vendor_oids.get('queue_name')
            if not name_oid:
                return queues_data

            name_results, error = await client.bulk(name_oid)
            if error or not name_results:
                logger.debug("MikroTik FW: Queue Simple Table no disponible o vacía")
                return queues_data

            # Construir mapa de queues
            queue_map: Dict[str, Dict[str, Any]] = {}
            for oid_str, value in name_results:
                idx = oid_str.split('.')[-1]
                queue_map[idx] = {
                    "index": idx,
                    "name": str(value).strip(),
                    "src_addr": None,
                    "dst_addr": None,
                    "interface": None,
                    "tx_bytes": 0,
                    "rx_bytes": 0,
                    "tx_packets": 0,
                    "rx_packets": 0,
                    "tx_drop": 0,
                    "rx_drop": 0,
                    "isp_detected": self._detect_isp_from_name(str(value)),
                }

            if not queue_map:
                return queues_data

            queues_data["available"] = True

            # Recolectar métricas de cada columna
            bulk_columns = {
                'queue_src_addr':   ('src_addr',    _snmp_addr_to_str),
                'queue_dst_addr':   ('dst_addr',    _snmp_addr_to_str),
                'queue_iface':      ('interface',   lambda v: str(v)),
                'queue_tx_bytes':   ('tx_bytes',    lambda v: safe_int(v)),
                'queue_rx_bytes':   ('rx_bytes',    lambda v: safe_int(v)),
                'queue_tx_packets': ('tx_packets',  lambda v: safe_int(v)),
                'queue_rx_packets': ('rx_packets',  lambda v: safe_int(v)),
                'queue_tx_drop':    ('tx_drop',     lambda v: safe_int(v)),
                'queue_rx_drop':    ('rx_drop',     lambda v: safe_int(v)),
            }

            for oid_name, (field, converter) in bulk_columns.items():
                oid = vendor_oids.get(oid_name)
                if not oid:
                    continue
                results, _ = await client.bulk(oid)
                for oid_str, value in (results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in queue_map:
                        queue_map[idx][field] = converter(value)

            # Calcular campos derivados y acumulados
            total_tx_bytes = 0
            total_rx_bytes = 0
            total_tx_drops = 0
            total_rx_drops = 0

            for entry in queue_map.values():
                tx = entry["tx_bytes"]
                rx = entry["rx_bytes"]
                entry["tx_gb"] = round(tx / (1024.0 ** 3), 4)
                entry["rx_gb"] = round(rx / (1024.0 ** 3), 4)
                total_tx_bytes += tx
                total_rx_bytes += rx
                total_tx_drops += entry["tx_drop"]
                total_rx_drops += entry["rx_drop"]

            queues_data["entries"] = list(queue_map.values())
            queues_data["summary"] = {
                "total_queues": len(queue_map),
                "total_tx_gb": round(total_tx_bytes / (1024.0 ** 3), 3),
                "total_rx_gb": round(total_rx_bytes / (1024.0 ** 3), 3),
                "total_tx_drops": total_tx_drops,
                "total_rx_drops": total_rx_drops,
            }

            logger.info(f"MikroTik FW: {len(queue_map)} queues recolectadas")

        except Exception as e:
            logger.warning(f"MikroTik FW: Error recolectando Queue Table: {e}")

        return queues_data

    # ==========================================================================
    # HELPER: Resumen de Canales de Internet
    # ==========================================================================

    def _build_internet_channels_summary(
        self,
        wan_interfaces: List[Dict[str, Any]],
        netwatch: Dict[str, Any],
        queues: Dict[str, Any],
    ) -> Dict[str, Any]:
        """
        Construye un resumen consolidado de canales de Internet.

        Combina:
        - Interfaces WAN detectadas (estado operativo, throughput)
        - Probes Netwatch (conectividad por ISP)
        - Queue stats (consumo real de ancho de banda)
        - Detección de ISP por nombre

        Returns:
            Diccionario con canales y resumen global.
        """
        channels: List[Dict[str, Any]] = []
        seen_isps: Dict[str, int] = {}  # isp -> índice en channels

        # 1. Canales basados en interfaces WAN
        for iface in wan_interfaces:
            isp = iface.get("isp_detected")
            channel_name = iface.get("alias") or iface.get("if_name") or iface.get("name", "")
            channel: Dict[str, Any] = {
                "channel_name": channel_name,
                "isp": isp or "Desconocido",
                "source": "wan_interface",
                "oper_status": iface.get("oper_status", "unknown"),
                "is_up": iface.get("oper_status", "").upper() == "UP",
                "speed_mbps": iface.get("speed_mbps", 0),
                "traffic_in_mb": iface.get("traffic_in_mb", 0.0),
                "traffic_out_mb": iface.get("traffic_out_mb", 0.0),
                "errors_in": iface.get("errors_in", 0),
                "errors_out": iface.get("errors_out", 0),
                "discards_in": iface.get("discards_in", 0),
                "discards_out": iface.get("discards_out", 0),
                "netwatch_status": None,    # Se enriquece más abajo
                "alerts": [],
            }

            # Generar alertas básicas
            channel["alerts"] = self._check_channel_alerts(channel)

            if isp and isp not in seen_isps:
                seen_isps[isp] = len(channels)
            channels.append(channel)

        # 2. Enriquecer con datos de Netwatch (por ISP detectado en el nombre del probe)
        if netwatch.get("available"):
            for probe in netwatch.get("probes", []):
                probe_isp = probe.get("isp_detected")
                probe_name = probe.get("name", "")
                probe_status = probe.get("status_text", "unknown")

                if probe_isp and probe_isp in seen_isps:
                    # Enriquecer canal existente
                    idx = seen_isps[probe_isp]
                    channels[idx]["netwatch_status"] = probe_status
                    channels[idx]["netwatch_probe"] = probe.get("target_ip")
                    channels[idx]["netwatch_probe_name"] = probe_name
                else:
                    # Probe sin interfaz WAN asociada — crear canal solo con netwatch
                    channel = {
                        "channel_name": probe_name,
                        "isp": probe_isp or "Desconocido",
                        "source": "netwatch",
                        "oper_status": "unknown",
                        "is_up": probe_status == "up",
                        "speed_mbps": 0,
                        "traffic_in_mb": 0.0,
                        "traffic_out_mb": 0.0,
                        "errors_in": 0,
                        "errors_out": 0,
                        "discards_in": 0,
                        "discards_out": 0,
                        "netwatch_status": probe_status,
                        "netwatch_probe": probe.get("target_ip"),
                        "netwatch_probe_name": probe_name,
                        "alerts": [],
                    }
                    if probe_isp:
                        seen_isps[probe_isp] = len(channels)
                    channels.append(channel)

        # 3. Enriquecer con datos de Queue (tráfico por ISP)
        if queues.get("available"):
            for entry in queues.get("entries", []):
                q_isp = entry.get("isp_detected")
                if q_isp and q_isp in seen_isps:
                    idx = seen_isps[q_isp]
                    channels[idx]["queue_tx_gb"] = entry.get("tx_gb", 0.0)
                    channels[idx]["queue_rx_gb"] = entry.get("rx_gb", 0.0)
                    channels[idx]["queue_tx_drops"] = entry.get("tx_drop", 0)
                    channels[idx]["queue_rx_drops"] = entry.get("rx_drop", 0)
                    # Alertas por drops en queue
                    if entry.get("tx_drop", 0) > 0 or entry.get("rx_drop", 0) > 0:
                        total_drops = entry.get("tx_drop", 0) + entry.get("rx_drop", 0)
                        channels[idx]["alerts"].append(
                            f"Queue drops detectados en canal {q_isp}: {total_drops} drops"
                        )

        # 4. Calcular summary global
        up_count = sum(1 for c in channels if c.get("is_up"))
        down_count = sum(1 for c in channels if not c.get("is_up") and c.get("oper_status") != "unknown")
        total_in_mb = sum(c.get("traffic_in_mb", 0.0) for c in channels)
        total_out_mb = sum(c.get("traffic_out_mb", 0.0) for c in channels)

        return {
            "channels": channels,
            "summary": {
                "total_channels": len(channels),
                "channels_up": up_count,
                "channels_down": down_count,
                "total_traffic_in_mb": round(total_in_mb, 2),
                "total_traffic_out_mb": round(total_out_mb, 2),
                "netwatch_available": netwatch.get("available", False),
                "queues_available": queues.get("available", False),
            },
        }

    # ==========================================================================
    # HELPER: Alertas por canal WAN
    # ==========================================================================

    def _check_channel_alerts(self, channel: Dict[str, Any]) -> List[str]:
        """
        Genera alertas básicas para un canal WAN.

        Returns:
            Lista de strings con alertas detectadas.
        """
        alerts = []

        if channel.get("oper_status", "").upper() == "DOWN":
            alerts.append(f"Canal WAN DOWN: {channel.get('channel_name', 'desconocido')}")

        errors_in = channel.get("errors_in", 0)
        errors_out = channel.get("errors_out", 0)
        if (errors_in or 0) + (errors_out or 0) > 100:
            alerts.append(
                f"Alto número de errores en interfaz WAN: "
                f"IN={errors_in} OUT={errors_out}"
            )

        discards = (channel.get("discards_in", 0) or 0) + (channel.get("discards_out", 0) or 0)
        if discards > 500:
            alerts.append(f"Descartes elevados en interfaz WAN: {discards} paquetes")

        return alerts

    # ==========================================================================
    # HELPER: Detectar ISP por nombre
    # ==========================================================================

    def _detect_isp_from_name(self, name: str) -> Optional[str]:
        """
        Detecta el ISP basándose en el nombre de interfaz, alias o probe.

        Soporta proveedores de Colombia y LATAM. Diseñado para funcionar
        con los nombres que los administradores de red dan a sus interfaces.

        Args:
            name: Nombre de la interfaz, alias o probe Netwatch.

        Returns:
            Nombre del ISP detectado, o None si no se reconoce.
        """
        if not name:
            return None

        name_lower = name.lower()

        isp_patterns = {
            'ETB':        ['etb', 'empresa telefonos bogota', 'empresa de telecomunicaciones'],
            'Tigo':       ['tigo', 'une', 'millicom', 'cable'],
            'Claro':      ['claro', 'comcel', 'telmex', 'amx'],
            'Movistar':   ['movistar', 'telefonica', 'telecom'],
            'Azteca':     ['azteca', 'teleazteca'],
            'InterNexa':  ['internexa', 'inter'],
            'Edatel':     ['edatel', 'epm telecomunicaciones'],
            'Coltel':     ['coltel', 'colombia telecomunicaciones'],
            'DirecTV':    ['directv', 'direct tv', 'at&t latam'],
            'Starlink':   ['starlink', 'spacex'],
            'AT&T':       ['at&t', 'att'],
            'Fibra':      ['fibra', 'fiber', 'ftth', 'fibra optica'],
            'DSL':        ['dsl', 'adsl', 'vdsl'],
            'LTE/4G':     ['lte', '4g', '5g', 'celular', 'movil'],
            'MPLS':       ['mpls', 'wan-mpls'],
        }

        for isp_name, patterns in isp_patterns.items():
            if any(p in name_lower for p in patterns):
                return isp_name

        return None

    # ==========================================================================
    # DETECCIÓN AUTOMÁTICA
    # ==========================================================================

    @classmethod
    def matches_sys_object_id(cls, sys_object_id: str) -> bool:
        """
        Detecta si un sysObjectID corresponde a MikroTik.

        Nota: MikroTik no usa un OID diferente para firewalls vs routers.
        La distinción entre MikroTikProfile y MikroTikFirewallProfile
        se hace a través del campo 'vendor' en devices.conf.
        """
        if not sys_object_id:
            return False
        return any(
            sys_object_id.startswith(prefix)
            for prefix in MIKROTIK_FW_SYS_OBJECT_IDS
        )
