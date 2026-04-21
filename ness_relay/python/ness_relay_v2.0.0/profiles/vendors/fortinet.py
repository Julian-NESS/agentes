# ==============================================================================
# NESS Relay v2.0.0 - Fortinet FortiGate Vendor Profile
# ==============================================================================
# Perfil completo para dispositivos Fortinet FortiGate (FortiOS).
#
# CPU/Memory/Disk: FORTINET-FORTIGATE-MIB (fgSystem)
# Sesiones:        FORTINET-FORTIGATE-MIB (fgSysSesCount, fgSysSes6Count)
# HA:              FORTINET-FORTIGATE-MIB (fgHaSystemMode)
# VPN:             FORTINET-FORTIGATE-MIB (fgVpnTunTable)
# Security:        FORTINET-FORTIGATE-MIB (fgAvVirusDetected, fgIpsIntrusionsDetected)
# WAN/Internet:    FORTINET-FORTIGATE-MIB (SD-WAN SLA, Interface stats, Link Health)
#
# Enterprise OID base: 1.3.6.1.4.1.12356 (Fortinet Inc.)
# sysObjectID típico:  1.3.6.1.4.1.12356.101.1.*
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
# OIDs ESPECÍFICOS DE FORTINET FortiGate
# ==============================================================================

# CPU OIDs - FORTINET-FORTIGATE-MIB
# FortiGate reporta CPU como un porcentaje global (fgSysCpuUsage).
FORTINET_CPU_OIDS: Dict[str, str] = {
    'fg_sys_cpu_usage':    '1.3.6.1.4.1.12356.101.4.1.3.0',  # fgSysCpuUsage (porcentaje 0-100)
}

# Memory OIDs - FORTINET-FORTIGATE-MIB
# Memoria como porcentaje de uso + capacidad total en KB.
FORTINET_MEMORY_OIDS: Dict[str, str] = {
    'fg_sys_mem_usage':    '1.3.6.1.4.1.12356.101.4.1.4.0',  # fgSysMemUsage (porcentaje 0-100)
    'fg_sys_mem_capacity': '1.3.6.1.4.1.12356.101.4.1.5.0',  # fgSysMemCapacity (KB)
}

# Disk OIDs - FORTINET-FORTIGATE-MIB
# Disco global (MB). Sin .0 porque performance_collector usa bulk().
FORTINET_DISK_OIDS: Dict[str, str] = {
    'fg_sys_disk_usage':    '1.3.6.1.4.1.12356.101.4.1.6',   # fgSysDiskUsage (MB)
    'fg_sys_disk_capacity': '1.3.6.1.4.1.12356.101.4.1.7',   # fgSysDiskCapacity (MB)
}

# ==============================================================================
# OIDs PARA MONITOREO DE CANALES DE INTERNET / WAN
# ==============================================================================

# IF-MIB estándar para interfaces (complementa el network_collector)
FORTINET_INTERFACE_OIDS: Dict[str, str] = {
    # IF-MIB estándar (RFC 2863)
    'if_descr':              '1.3.6.1.2.1.2.2.1.2',           # ifDescr
    'if_type':               '1.3.6.1.2.1.2.2.1.3',           # ifType
    'if_speed':              '1.3.6.1.2.1.2.2.1.5',           # ifSpeed (bps)
    'if_admin_status':       '1.3.6.1.2.1.2.2.1.7',           # ifAdminStatus
    'if_oper_status':        '1.3.6.1.2.1.2.2.1.8',           # ifOperStatus
    'if_in_octets':          '1.3.6.1.2.1.2.2.1.10',          # ifInOctets
    'if_out_octets':         '1.3.6.1.2.1.2.2.1.16',          # ifOutOctets
    'if_in_errors':          '1.3.6.1.2.1.2.2.1.14',          # ifInErrors
    'if_out_errors':         '1.3.6.1.2.1.2.2.1.20',          # ifOutErrors
    'if_in_discards':        '1.3.6.1.2.1.2.2.1.13',          # ifInDiscards
    'if_out_discards':       '1.3.6.1.2.1.2.2.1.19',          # ifOutDiscards
    # IF-MIB High Capacity (64-bit counters)
    'if_hc_in_octets':       '1.3.6.1.2.1.31.1.1.1.6',        # ifHCInOctets
    'if_hc_out_octets':      '1.3.6.1.2.1.31.1.1.1.10',       # ifHCOutOctets
    'if_high_speed':         '1.3.6.1.2.1.31.1.1.1.15',       # ifHighSpeed (Mbps)
    'if_name':               '1.3.6.1.2.1.31.1.1.1.1',        # ifName (alias corto)
    'if_alias':              '1.3.6.1.2.1.31.1.1.1.18',       # ifAlias (descripción user-defined)
    # In/Out Unicast Packets
    'if_in_ucast_pkts':      '1.3.6.1.2.1.2.2.1.11',          # ifInUcastPkts
    'if_out_ucast_pkts':     '1.3.6.1.2.1.2.2.1.17',          # ifOutUcastPkts
    'if_hc_in_ucast_pkts':   '1.3.6.1.2.1.31.1.1.1.7',        # ifHCInUcastPkts
    'if_hc_out_ucast_pkts':  '1.3.6.1.2.1.31.1.1.1.11',       # ifHCOutUcastPkts
}

# FORTINET-FORTIGATE-MIB: SD-WAN SLA Health Check
# Estos OIDs permiten monitorear la salud de los enlaces WAN via SD-WAN
FORTINET_SDWAN_OIDS: Dict[str, str] = {
    # fgVWLHealthCheckLinkTable - Estado de salud de enlaces SD-WAN
    'sdwan_link_name':          '1.3.6.1.4.1.12356.101.4.9.2.1.2',   # fgVWLHealthCheckLinkName
    'sdwan_link_state':         '1.3.6.1.4.1.12356.101.4.9.2.1.4',   # fgVWLHealthCheckLinkState (0=dead, 1=alive)
    'sdwan_link_latency':       '1.3.6.1.4.1.12356.101.4.9.2.1.5',   # fgVWLHealthCheckLinkLatency (ms * 1000)
    'sdwan_link_jitter':        '1.3.6.1.4.1.12356.101.4.9.2.1.6',   # fgVWLHealthCheckLinkJitter (ms * 1000)
    'sdwan_link_pkt_loss':      '1.3.6.1.4.1.12356.101.4.9.2.1.8',   # fgVWLHealthCheckLinkPacketLoss (% * 100)
    'sdwan_link_pkt_sent':      '1.3.6.1.4.1.12356.101.4.9.2.1.9',   # fgVWLHealthCheckLinkPacketSend
    'sdwan_link_pkt_recv':      '1.3.6.1.4.1.12356.101.4.9.2.1.10',  # fgVWLHealthCheckLinkPacketRecv
    'sdwan_link_bandwidth_in':  '1.3.6.1.4.1.12356.101.4.9.2.1.13',  # fgVWLHealthCheckLinkBandwidthIn (Kbps)
    'sdwan_link_bandwidth_out': '1.3.6.1.4.1.12356.101.4.9.2.1.14',  # fgVWLHealthCheckLinkBandwidthOut (Kbps)
    
    # fgVWLHealthCheckTable - Configuración del Health Check
    'sdwan_hc_name':            '1.3.6.1.4.1.12356.101.4.9.1.1.2',   # fgVWLHealthCheckName
    'sdwan_hc_protocol':        '1.3.6.1.4.1.12356.101.4.9.1.1.3',   # fgVWLHealthCheckProtocol (1=ping, 2=tcp-echo, etc)
    
    # fgVWLMemberTable - Miembros SD-WAN (interfaces WAN)
    'sdwan_member_ifname':      '1.3.6.1.4.1.12356.101.4.9.3.1.2',   # fgVWLMemberIfname
    'sdwan_member_state':       '1.3.6.1.4.1.12356.101.4.9.3.1.4',   # fgVWLMemberState
    'sdwan_member_volume_in':   '1.3.6.1.4.1.12356.101.4.9.3.1.5',   # fgVWLMemberVolumeIn (bytes)
    'sdwan_member_volume_out':  '1.3.6.1.4.1.12356.101.4.9.3.1.6',   # fgVWLMemberVolumeOut (bytes)
    'sdwan_member_sessions':    '1.3.6.1.4.1.12356.101.4.9.3.1.7',   # fgVWLMemberSessions
}

# Vendor-specific OIDs - Sesiones, HA, Firmware, Serial, VPN, Security
FORTINET_VENDOR_OIDS: Dict[str, str] = {
    # Sistema / Firmware
    'fg_sys_version':              '1.3.6.1.4.1.12356.101.4.1.1.0',   # fgSysVersion (firmware string)
    'fn_sys_serial':               '1.3.6.1.4.1.12356.100.1.1.1.0',   # fnSysSerial (número de serie)

    # Sesiones activas
    'fg_sys_ses_count':            '1.3.6.1.4.1.12356.101.4.1.8.0',   # fgSysSesCount (IPv4)
    'fg_sys_ses6_count':           '1.3.6.1.4.1.12356.101.4.1.11.0',  # fgSysSes6Count (IPv6)
    'fg_sys_ses_rate':             '1.3.6.1.4.1.12356.101.4.1.14.0',  # fgSysSesRate1 (sesiones/seg)

    # Alta Disponibilidad (HA)
    'fg_ha_system_mode':           '1.3.6.1.4.1.12356.101.13.1.1.0',  # fgHaSystemMode (1=standalone, 2=a-a, 3=a-p)
    'fg_ha_group_id':              '1.3.6.1.4.1.12356.101.13.1.2.0',  # fgHaGroupId
    'fg_ha_priority':              '1.3.6.1.4.1.12356.101.13.1.3.0',  # fgHaPriority

    # VPN - Estado de túneles (tabla - se escanea con bulk)
    'fg_vpn_tun_ent_status':       '1.3.6.1.4.1.12356.101.12.2.2.1.20',  # fgVpnTunEntStatus (1=down, 2=up)
    'fg_vpn_tun_ent_name':         '1.3.6.1.4.1.12356.101.12.2.2.1.3',   # fgVpnTunEntPhase1Name
    'fg_vpn_tun_ent_in_oct':       '1.3.6.1.4.1.12356.101.12.2.2.1.18',  # fgVpnTunEntInOctets
    'fg_vpn_tun_ent_out_oct':      '1.3.6.1.4.1.12356.101.12.2.2.1.19',  # fgVpnTunEntOutOctets
    'fg_vpn_tun_ent_up_time':      '1.3.6.1.4.1.12356.101.12.2.2.1.21',  # fgVpnTunEntUpTime

    # Seguridad - Detecciones (AV, IPS)
    'fg_av_virus_detected':        '1.3.6.1.4.1.12356.101.8.2.1.1.0',    # fgAvVirusDetected (counter)
    'fg_av_virus_blocked':         '1.3.6.1.4.1.12356.101.8.2.1.2.0',    # fgAvVirusBlocked (counter)
    'fg_ips_intrusions_detected':  '1.3.6.1.4.1.12356.101.9.2.1.1.0',    # fgIpsIntrusionsDetected
    'fg_ips_intrusions_blocked':   '1.3.6.1.4.1.12356.101.9.2.1.2.0',    # fgIpsIntrusionsBlocked
    
    # Interfaces WAN/LAN (agregamos todos los OIDs de interfaces y SD-WAN)
    **FORTINET_INTERFACE_OIDS,
    **FORTINET_SDWAN_OIDS,
}

# Patrones de nombres de interfaz WAN típicos en FortiGate
# Estos se usan para identificar automáticamente las interfaces WAN
WAN_INTERFACE_PATTERNS = [
    'wan', 'internet', 'isp', 'eth0', 'port1', 'port2',
    'etb', 'tigo', 'claro', 'movistar', 'azteca', 'att',  # Proveedores Colombia y LATAM
    'fiber', 'fibra', 'dsl', 'mpls', 'lte', '4g', '5g',
    'primary', 'secondary', 'backup', 'principal', 'respaldo',
]

# sysObjectID prefijos para detección automática de Fortinet
FORTINET_SYS_OBJECT_IDS = [
    '1.3.6.1.4.1.12356.101.1',   # FortiGate (mayoría de modelos)
    '1.3.6.1.4.1.12356',          # Fortinet Inc. (general)
]


# ==============================================================================
# PROFILE CLASS
# ==============================================================================

class FortinetProfile(BaseDeviceProfile):
    """
    Perfil de dispositivo para Fortinet FortiGate (FortiOS).

    Características:
    - CPU via FORTINET-FORTIGATE-MIB (fgSysCpuUsage - porcentaje directo)
    - Memoria via FORTINET-FORTIGATE-MIB (fgSysMemUsage + fgSysMemCapacity)
    - Disco via FORTINET-FORTIGATE-MIB (fgSysDiskUsage + fgSysDiskCapacity)
    - Sesiones activas IPv4/IPv6 y tasa de sesiones
    - Estado de HA (standalone, active-active, active-passive)
    - Túneles VPN IPSec (estado, tráfico, uptime)
    - Detecciones de AV e IPS
    """

    vendor = "fortinet"
    vendor_display_name = "Fortinet FortiGate"
    device_type = "firewall"

    # ==========================================================================
    # OIDs
    # ==========================================================================

    def get_vendor_oids(self) -> Dict[str, str]:
        """Retorna OIDs específicos de FORTINET-FORTIGATE-MIB."""
        return FORTINET_VENDOR_OIDS.copy()

    def get_cpu_oids(self) -> Dict[str, str]:
        """Retorna OIDs de CPU via FORTINET-FORTIGATE-MIB."""
        return FORTINET_CPU_OIDS.copy()

    def get_memory_oids(self) -> Dict[str, str]:
        """Retorna OIDs de memoria via FORTINET-FORTIGATE-MIB."""
        return FORTINET_MEMORY_OIDS.copy()

    def get_disk_oids(self) -> Dict[str, str]:
        """Retorna OIDs de disco via FORTINET-FORTIGATE-MIB."""
        return FORTINET_DISK_OIDS.copy()

    # ==========================================================================
    # NORMALIZACIÓN DE CPU
    # ==========================================================================

    def normalize_cpu_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de CPU de Fortinet FortiGate.

        FortiGate reporta CPU como un solo porcentaje global (fgSysCpuUsage).
        No proporciona load averages nativamente (1/5/15 min).

        Args:
            raw_data: Datos crudos {oid_name: value} de las queries SNMP.

        Returns:
            Datos normalizados de CPU en formato estándar NESS.
        """
        cpu_usage = safe_float(raw_data.get('fg_sys_cpu_usage'))

        return {
            "cpu_usage_percent": round(cpu_usage, 2),
            "load_1min": None,   # FortiGate no reporta load averages UNIX
            "load_5min": None,
            "load_15min": None,
        }

    # ==========================================================================
    # NORMALIZACIÓN DE MEMORIA
    # ==========================================================================

    def normalize_memory_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de memoria de Fortinet FortiGate.

        FortiGate reporta:
        - fgSysMemUsage:    Porcentaje de memoria usada (0-100)
        - fgSysMemCapacity: Capacidad total en KB

        De estos valores se derivan used_mb, free_mb, etc.

        Args:
            raw_data: Datos crudos {oid_name: value} de las queries SNMP.

        Returns:
            Datos normalizados de memoria en formato estándar NESS.
        """
        mem_usage_percent = safe_float(raw_data.get('fg_sys_mem_usage'))
        mem_capacity_kb = safe_int(raw_data.get('fg_sys_mem_capacity'))

        if mem_capacity_kb > 0:
            mem_total_mb = round(mem_capacity_kb / 1024.0, 2)
            mem_used_mb = round(mem_total_mb * (mem_usage_percent / 100.0), 2)
            mem_free_mb = round(mem_total_mb - mem_used_mb, 2)

            return {
                "mem_usage_percent": round(mem_usage_percent, 2),
                "mem_total_mb": mem_total_mb,
                "mem_used_mb": mem_used_mb,
                "mem_free_mb": mem_free_mb,
                "swap_total_mb": 0.0,   # FortiGate no tiene swap
                "swap_free_mb": 0.0,
            }
        else:
            # Sin capacidad: reportar solo el porcentaje
            return {
                "mem_usage_percent": round(mem_usage_percent, 2),
                "mem_total_mb": None,
                "mem_used_mb": None,
                "mem_free_mb": None,
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
        Normaliza datos de disco de Fortinet FortiGate.

        FortiGate reporta disco como valores escalares globales:
        - fgSysDiskUsage:    Uso en MB
        - fgSysDiskCapacity: Capacidad en MB

        El performance_collector usa bulk() por lo que los OIDs escalares
        retornan con índice '0'. Se agrupan bajo una única partición '/'.

        Args:
            raw_disk_entries: Datos del bulk scan: {idx: {oid_name: value}}.

        Returns:
            Diccionario normalizado de discos.
        """
        disk_data: Dict[str, Dict[str, Any]] = {}

        disk_usage_mb = 0
        disk_capacity_mb = 0

        # Extraer valores de los entries del bulk walk
        for idx, raw in raw_disk_entries.items():
            usage_val = raw.get('fg_sys_disk_usage')
            capacity_val = raw.get('fg_sys_disk_capacity')
            if usage_val is not None:
                disk_usage_mb = safe_int(usage_val)
            if capacity_val is not None:
                disk_capacity_mb = safe_int(capacity_val)

        if disk_capacity_mb > 0:
            total_gb = round(disk_capacity_mb / 1024.0, 3)
            used_gb = round(disk_usage_mb / 1024.0, 3)
            free_gb = round(max(0.0, total_gb - used_gb), 3)
            percent = round(calculate_percentage(disk_usage_mb, disk_capacity_mb), 2)

            disk_data["1"] = {
                "index": "1",
                "path": "/",
                "total_gb": total_gb,
                "used_gb": used_gb,
                "free_gb": free_gb,
                "percent_used": percent,
            }
        elif not raw_disk_entries:
            # Algunos modelos FortiGate no reportan disco via SNMP
            logger.debug("FortiGate: No se obtuvieron datos de disco via SNMP")

        return disk_data

    # ==========================================================================
    # DATOS ESPECÍFICOS DE Fortinet (FORTINET-FORTIGATE-MIB)
    # ==========================================================================

    async def collect_vendor_specific_data(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta datos específicos de Fortinet FortiGate.

        Obtiene:
        - Información del sistema (firmware, serial)
        - Sesiones activas (IPv4, IPv6, tasa)
        - Estado de HA
        - Túneles VPN IPSec (estado, nombre, tráfico, uptime)
        - Detecciones de seguridad (Antivirus, IPS)
        - **CANALES DE INTERNET/WAN:**
          - Interfaces WAN identificadas por patrón de nombre
          - SD-WAN Link Health (latencia, jitter, packet loss)
          - Bandwidth utilization por interfaz WAN
          - Estado operativo de cada canal

        Args:
            client: Instancia de SnmpClient conectada al FortiGate.

        Returns:
            Diccionario con datos de FORTINET-FORTIGATE-MIB.
        """
        logger.info("Recolectando datos específicos de Fortinet FortiGate...")
        vendor_oids = self.get_vendor_oids()

        fortinet_data: Dict[str, Any] = {
            "system_info": {},
            "sessions": {},
            "ha_status": {},
            "vpn_tunnels": [],
            "security_detections": {},
            "internet_channels": {},      # Monitoreo de canales WAN/Internet
            "sdwan_health": {},           # SD-WAN health checks
            "wan_interfaces": [],         # Interfaces WAN identificadas
            "collection_timestamp": now_iso(),
        }

        # ===== Sistema: Firmware y Serial =====
        for oid_name in ('fg_sys_version', 'fn_sys_serial'):
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    fortinet_data["system_info"][oid_name] = str(res.value)
                else:
                    fortinet_data["system_info"][oid_name] = None

        # ===== Sesiones activas =====
        for oid_name in ('fg_sys_ses_count', 'fg_sys_ses6_count', 'fg_sys_ses_rate'):
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    fortinet_data["sessions"][oid_name] = safe_int(res.value)
                else:
                    fortinet_data["sessions"][oid_name] = None

        # ===== Alta Disponibilidad (HA) =====
        ha_mode_map = {1: 'standalone', 2: 'active-active', 3: 'active-passive'}
        for oid_name in ('fg_ha_system_mode', 'fg_ha_group_id', 'fg_ha_priority'):
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    val = safe_int(res.value)
                    if oid_name == 'fg_ha_system_mode':
                        fortinet_data["ha_status"]["mode"] = ha_mode_map.get(val, f"unknown({val})")
                        fortinet_data["ha_status"]["mode_raw"] = val
                    else:
                        fortinet_data["ha_status"][oid_name] = val
                else:
                    fortinet_data["ha_status"][oid_name] = None

        # ===== Túneles VPN IPSec =====
        fortinet_data["vpn_tunnels"] = await self._collect_vpn_tunnels(client, vendor_oids)

        # ===== Detecciones de Seguridad (AV, IPS) =====
        sec_oid_names = (
            'fg_av_virus_detected', 'fg_av_virus_blocked',
            'fg_ips_intrusions_detected', 'fg_ips_intrusions_blocked',
        )
        for oid_name in sec_oid_names:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    fortinet_data["security_detections"][oid_name] = safe_int(res.value)
                else:
                    fortinet_data["security_detections"][oid_name] = None

        # =====================================================================
        # MONITOREO DE CANALES DE INTERNET / WAN
        # =====================================================================
        # Recolecta datos completos de interfaces WAN para monitorear ISPs:
        # - ETB, Tigo, Claro u otros proveedores configurados
        # - Latencia, jitter, packet loss (via SD-WAN)
        # - Bandwidth in/out, utilización
        # - Estado operativo (up/down)
        
        logger.info("Recolectando datos de canales de Internet/WAN...")
        
        # ===== Interfaces WAN (identificadas por patrón) =====
        fortinet_data["wan_interfaces"] = await self._collect_wan_interfaces(client, vendor_oids)
        
        # ===== SD-WAN Health Check (latencia, jitter, packet loss) =====
        fortinet_data["sdwan_health"] = await self._collect_sdwan_health(client, vendor_oids)
        
        # ===== Resumen de canales de Internet por ISP =====
        fortinet_data["internet_channels"] = self._build_internet_channels_summary(
            fortinet_data["wan_interfaces"],
            fortinet_data["sdwan_health"]
        )

        logger.info("Datos específicos de Fortinet recolectados exitosamente")
        return fortinet_data

    # --------------------------------------------------------------------------
    # HELPER: Recolectar túneles VPN
    # --------------------------------------------------------------------------

    async def _collect_vpn_tunnels(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> list:
        """
        Recolecta información de túneles VPN IPSec via bulk walk
        de la tabla fgVpnTunTable.

        Returns:
            Lista de diccionarios, uno por túnel VPN activo.
        """
        tunnels = []
        status_map = {1: 'down', 2: 'up'}

        try:
            # Obtener nombres de túneles (tabla base)
            names_oid = vendor_oids.get('fg_vpn_tun_ent_name')
            if not names_oid:
                return tunnels

            name_results, error = await client.bulk(names_oid)
            if error or not name_results:
                return tunnels

            # Mapa idx → túnel
            tunnel_map: Dict[str, Dict[str, Any]] = {}
            for oid_str, value in name_results:
                idx = oid_str.split('.')[-1]
                tunnel_map[idx] = {
                    "name": str(value),
                    "index": idx,
                    "status": "unknown",
                    "traffic_in_bytes": 0,
                    "traffic_out_bytes": 0,
                    "uptime_seconds": 0,
                }

            # Enriquecer con datos adicionales por columna de la tabla
            bulk_columns = {
                'fg_vpn_tun_ent_status':  ('status',            lambda v: status_map.get(safe_int(v), f"unknown({v})")),
                'fg_vpn_tun_ent_in_oct':  ('traffic_in_bytes',  lambda v: safe_int(v)),
                'fg_vpn_tun_ent_out_oct': ('traffic_out_bytes', lambda v: safe_int(v)),
                'fg_vpn_tun_ent_up_time': ('uptime_seconds',    lambda v: safe_int(v)),
            }

            for oid_name, (field, converter) in bulk_columns.items():
                oid = vendor_oids.get(oid_name)
                if not oid:
                    continue
                results, _ = await client.bulk(oid)
                for oid_str, value in (results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in tunnel_map:
                        tunnel_map[idx][field] = converter(value)

            tunnels = list(tunnel_map.values())

        except Exception as e:
            logger.warning(f"Error recolectando túneles VPN: {e}")

        return tunnels

    # --------------------------------------------------------------------------
    # HELPER: Recolectar interfaces WAN (Monitoreo de canales de Internet)
    # --------------------------------------------------------------------------

    async def _collect_wan_interfaces(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> List[Dict[str, Any]]:
        """
        Recolecta información completa de interfaces WAN detectadas.

        Identifica interfaces WAN por patrones en el nombre (wan, internet, isp,
        etb, tigo, claro, etc.) y recolecta métricas detalladas de cada una.

        Returns:
            Lista de diccionarios con datos de cada interfaz WAN.
        """
        wan_interfaces: List[Dict[str, Any]] = []
        
        try:
            # Obtener descripción de todas las interfaces
            if_descr_oid = vendor_oids.get('if_descr')
            if not if_descr_oid:
                logger.warning("OID if_descr no disponible para detectar interfaces WAN")
                return wan_interfaces

            descr_results, error = await client.bulk(if_descr_oid)
            if error or not descr_results:
                logger.debug(f"No se pudieron obtener descripciones de interfaz: {error}")
                return wan_interfaces

            # Construir mapa de interfaces con sus índices
            interface_map: Dict[str, Dict[str, Any]] = {}
            for oid_str, value in descr_results:
                idx = oid_str.split('.')[-1]
                if_name = str(value).strip()
                if_name_lower = if_name.lower()
                
                # Detectar si es una interfaz WAN basándose en patrones
                is_wan = any(pattern in if_name_lower for pattern in WAN_INTERFACE_PATTERNS)
                
                interface_map[idx] = {
                    "index": idx,
                    "name": if_name,
                    "is_wan": is_wan,
                    "alias": None,
                    "admin_status": "unknown",
                    "oper_status": "unknown",
                    "speed_mbps": 0,
                    "traffic_in_bytes": 0,
                    "traffic_out_bytes": 0,
                    "traffic_in_mbps": 0.0,
                    "traffic_out_mbps": 0.0,
                    "utilization_in_percent": 0.0,
                    "utilization_out_percent": 0.0,
                    "errors_in": 0,
                    "errors_out": 0,
                    "discards_in": 0,
                    "discards_out": 0,
                    "packets_in": 0,
                    "packets_out": 0,
                    "isp_detected": self._detect_isp_from_name(if_name),
                }

            # Recolectar alias/descripción del usuario (puede tener nombre del ISP)
            if_alias_oid = vendor_oids.get('if_alias')
            if if_alias_oid:
                alias_results, _ = await client.bulk(if_alias_oid)
                for oid_str, value in (alias_results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in interface_map:
                        alias = str(value).strip()
                        interface_map[idx]["alias"] = alias
                        # Re-detectar ISP con alias
                        if not interface_map[idx]["isp_detected"] and alias:
                            interface_map[idx]["isp_detected"] = self._detect_isp_from_name(alias)
                        # Si el alias sugiere WAN, marcar como WAN
                        if any(p in alias.lower() for p in WAN_INTERFACE_PATTERNS):
                            interface_map[idx]["is_wan"] = True

            # Recolectar métricas adicionales solo para interfaces WAN
            wan_indices = [idx for idx, data in interface_map.items() if data["is_wan"]]
            
            if not wan_indices:
                logger.info("No se detectaron interfaces WAN por patrón de nombre")
                return wan_interfaces

            # Métricas a recolectar para interfaces WAN
            bulk_columns = {
                'if_admin_status':      ('admin_status',      lambda v: "UP" if str(v) == '1' else "DOWN"),
                'if_oper_status':       ('oper_status',       lambda v: "UP" if str(v) == '1' else "DOWN"),
                'if_high_speed':        ('speed_mbps',        lambda v: safe_int(v)),
                'if_hc_in_octets':      ('traffic_in_bytes',  lambda v: safe_int(v)),
                'if_hc_out_octets':     ('traffic_out_bytes', lambda v: safe_int(v)),
                'if_in_errors':         ('errors_in',         lambda v: safe_int(v)),
                'if_out_errors':        ('errors_out',        lambda v: safe_int(v)),
                'if_in_discards':       ('discards_in',       lambda v: safe_int(v)),
                'if_out_discards':      ('discards_out',      lambda v: safe_int(v)),
                'if_hc_in_ucast_pkts':  ('packets_in',        lambda v: safe_int(v)),
                'if_hc_out_ucast_pkts': ('packets_out',       lambda v: safe_int(v)),
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

            # Fallback: si if_high_speed está vacío, usar if_speed
            if_speed_oid = vendor_oids.get('if_speed')
            if if_speed_oid:
                speed_results, _ = await client.bulk(if_speed_oid)
                for oid_str, value in (speed_results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in interface_map and interface_map[idx]["is_wan"]:
                        if interface_map[idx]["speed_mbps"] == 0:
                            bps = safe_int(value)
                            interface_map[idx]["speed_mbps"] = round(bps / 1_000_000, 2)

            # Calcular utilización y convertir a Mbps
            for idx, data in interface_map.items():
                if not data["is_wan"]:
                    continue
                    
                speed = data["speed_mbps"]
                in_bytes = data["traffic_in_bytes"]
                out_bytes = data["traffic_out_bytes"]
                
                # Convertir bytes a MB para visualización
                data["traffic_in_mb"] = round(in_bytes / (1024.0 * 1024.0), 2)
                data["traffic_out_mb"] = round(out_bytes / (1024.0 * 1024.0), 2)
                
                # Nota: La utilización real requiere delta de tiempo + delta de bytes.
                # Aquí calculamos el throughput instantáneo si el dispositivo lo soporta.
                # Para utilización precisa se necesita muestreo periódico.
                
                wan_interfaces.append(data)

            logger.info(f"Detectadas {len(wan_interfaces)} interfaces WAN")

        except Exception as e:
            logger.warning(f"Error recolectando interfaces WAN: {e}")

        return wan_interfaces

    # --------------------------------------------------------------------------
    # HELPER: Recolectar SD-WAN Health Check
    # --------------------------------------------------------------------------

    async def _collect_sdwan_health(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> Dict[str, Any]:
        """
        Recolecta datos de salud de enlaces SD-WAN.

        Incluye para cada enlace configurado en SD-WAN:
        - Estado (alive/dead)
        - Latencia (ms)
        - Jitter (ms)
        - Packet Loss (%)
        - Bandwidth medido (Kbps)

        Estos datos son críticos para monitorear la calidad de los
        canales de Internet (ETB, Tigo, Claro, etc.).

        Returns:
            Diccionario con health checks de SD-WAN por enlace.
        """
        sdwan_data: Dict[str, Any] = {
            "links": [],
            "members": [],
            "health_checks": [],
            "summary": {
                "total_links": 0,
                "links_up": 0,
                "links_down": 0,
                "avg_latency_ms": None,
                "avg_jitter_ms": None,
                "avg_packet_loss_percent": None,
            },
            "available": False,
        }

        try:
            # ===== SD-WAN Link Health Table (fgVWLHealthCheckLinkTable) =====
            link_name_oid = vendor_oids.get('sdwan_link_name')
            if not link_name_oid:
                logger.debug("SD-WAN OIDs no disponibles - puede no estar configurado")
                return sdwan_data

            name_results, error = await client.bulk(link_name_oid)
            if error or not name_results:
                logger.debug(f"SD-WAN no configurado o sin enlaces: {error}")
                return sdwan_data

            # Construir mapa de enlaces SD-WAN
            link_map: Dict[str, Dict[str, Any]] = {}
            for oid_str, value in name_results:
                # El índice de fgVWLHealthCheckLinkTable es compuesto: healthCheckIndex.linkIndex
                idx_parts = oid_str.split('.')[-2:]
                idx = '.'.join(idx_parts)
                
                link_map[idx] = {
                    "index": idx,
                    "name": str(value).strip(),
                    "state": "unknown",
                    "state_text": "unknown",
                    "latency_ms": None,
                    "jitter_ms": None,
                    "packet_loss_percent": None,
                    "packets_sent": 0,
                    "packets_received": 0,
                    "bandwidth_in_kbps": None,
                    "bandwidth_out_kbps": None,
                    "isp_detected": self._detect_isp_from_name(str(value)),
                }

            if not link_map:
                return sdwan_data

            sdwan_data["available"] = True

            # Recolectar métricas de salud por enlace
            health_columns = {
                'sdwan_link_state':         ('state',               lambda v: safe_int(v)),
                'sdwan_link_latency':       ('latency_raw',         lambda v: safe_int(v)),
                'sdwan_link_jitter':        ('jitter_raw',          lambda v: safe_int(v)),
                'sdwan_link_pkt_loss':      ('pkt_loss_raw',        lambda v: safe_int(v)),
                'sdwan_link_pkt_sent':      ('packets_sent',        lambda v: safe_int(v)),
                'sdwan_link_pkt_recv':      ('packets_received',    lambda v: safe_int(v)),
                'sdwan_link_bandwidth_in':  ('bandwidth_in_kbps',   lambda v: safe_int(v)),
                'sdwan_link_bandwidth_out': ('bandwidth_out_kbps',  lambda v: safe_int(v)),
            }

            for oid_name, (field, converter) in health_columns.items():
                oid = vendor_oids.get(oid_name)
                if not oid:
                    continue
                results, _ = await client.bulk(oid)
                for oid_str, value in (results or []):
                    idx_parts = oid_str.split('.')[-2:]
                    idx = '.'.join(idx_parts)
                    if idx in link_map:
                        link_map[idx][field] = converter(value)

            # Procesar y normalizar valores
            total_latency = 0
            total_jitter = 0
            total_pkt_loss = 0
            count_with_metrics = 0
            links_up = 0
            links_down = 0

            for idx, link in link_map.items():
                # Estado: 0=dead, 1=alive
                state = link.get('state', 0)
                link["state_text"] = "up" if state == 1 else "down"
                if state == 1:
                    links_up += 1
                else:
                    links_down += 1

                # Latencia: valor en microsegundos * 1000, dividir para obtener ms
                latency_raw = link.pop('latency_raw', None)
                if latency_raw is not None and latency_raw > 0:
                    link["latency_ms"] = round(latency_raw / 1000.0, 2)
                    total_latency += link["latency_ms"]
                    count_with_metrics += 1

                # Jitter: mismo formato que latencia
                jitter_raw = link.pop('jitter_raw', None)
                if jitter_raw is not None and jitter_raw > 0:
                    link["jitter_ms"] = round(jitter_raw / 1000.0, 2)
                    total_jitter += link["jitter_ms"]

                # Packet Loss: valor * 100, dividir para obtener %
                pkt_loss_raw = link.pop('pkt_loss_raw', None)
                if pkt_loss_raw is not None:
                    link["packet_loss_percent"] = round(pkt_loss_raw / 100.0, 2)
                    total_pkt_loss += link["packet_loss_percent"]

                # Bandwidth: ya en Kbps, convertir a Mbps para conveniencia
                bw_in = link.get("bandwidth_in_kbps")
                bw_out = link.get("bandwidth_out_kbps")
                if bw_in:
                    link["bandwidth_in_mbps"] = round(bw_in / 1000.0, 2)
                if bw_out:
                    link["bandwidth_out_mbps"] = round(bw_out / 1000.0, 2)

                sdwan_data["links"].append(link)

            # Calcular promedios para el summary
            total_links = len(link_map)
            sdwan_data["summary"]["total_links"] = total_links
            sdwan_data["summary"]["links_up"] = links_up
            sdwan_data["summary"]["links_down"] = links_down

            if count_with_metrics > 0:
                sdwan_data["summary"]["avg_latency_ms"] = round(total_latency / count_with_metrics, 2)
                sdwan_data["summary"]["avg_jitter_ms"] = round(total_jitter / count_with_metrics, 2)
                sdwan_data["summary"]["avg_packet_loss_percent"] = round(total_pkt_loss / count_with_metrics, 2)

            # ===== SD-WAN Members (interfaces WAN en SD-WAN) =====
            member_name_oid = vendor_oids.get('sdwan_member_ifname')
            if member_name_oid:
                member_results, _ = await client.bulk(member_name_oid)
                member_map: Dict[str, Dict[str, Any]] = {}
                
                for oid_str, value in (member_results or []):
                    idx = oid_str.split('.')[-1]
                    member_map[idx] = {
                        "index": idx,
                        "interface_name": str(value).strip(),
                        "state": "unknown",
                        "volume_in_bytes": 0,
                        "volume_out_bytes": 0,
                        "sessions": 0,
                        "isp_detected": self._detect_isp_from_name(str(value)),
                    }

                # Recolectar métricas de miembros
                member_columns = {
                    'sdwan_member_state':       ('state',           lambda v: "active" if safe_int(v) == 1 else "inactive"),
                    'sdwan_member_volume_in':   ('volume_in_bytes', lambda v: safe_int(v)),
                    'sdwan_member_volume_out':  ('volume_out_bytes',lambda v: safe_int(v)),
                    'sdwan_member_sessions':    ('sessions',        lambda v: safe_int(v)),
                }

                for oid_name, (field, converter) in member_columns.items():
                    oid = vendor_oids.get(oid_name)
                    if not oid:
                        continue
                    results, _ = await client.bulk(oid)
                    for oid_str, value in (results or []):
                        idx = oid_str.split('.')[-1]
                        if idx in member_map:
                            member_map[idx][field] = converter(value)

                # Convertir volumen a MB/GB
                for member in member_map.values():
                    vol_in = member["volume_in_bytes"]
                    vol_out = member["volume_out_bytes"]
                    member["volume_in_gb"] = round(vol_in / (1024.0 ** 3), 3)
                    member["volume_out_gb"] = round(vol_out / (1024.0 ** 3), 3)
                    sdwan_data["members"].append(member)

            logger.info(f"SD-WAN: {links_up} enlaces UP, {links_down} enlaces DOWN")

        except Exception as e:
            logger.warning(f"Error recolectando SD-WAN health: {e}")

        return sdwan_data

    # --------------------------------------------------------------------------
    # HELPER: Detectar ISP por nombre de interfaz
    # --------------------------------------------------------------------------

    def _detect_isp_from_name(self, name: str) -> Optional[str]:
        """
        Detecta el nombre del ISP basándose en el nombre de la interfaz.

        Soporta ISPs comunes de Colombia y LATAM.

        Args:
            name: Nombre de la interfaz o alias.

        Returns:
            Nombre del ISP detectado o None si no se reconoce.
        """
        if not name:
            return None
        
        name_lower = name.lower()
        
        isp_patterns = {
            'ETB': ['etb', 'empresa telefonos bogota'],
            'Tigo': ['tigo', 'une', 'millicom'],
            'Claro': ['claro', 'comcel', 'telmex'],
            'Movistar': ['movistar', 'telefonica'],
            'Azteca': ['azteca'],
            'AT&T': ['att', 'at&t'],
            'Level3': ['level3', 'lumen', 'centurylink'],
            'GTD': ['gtd'],
            'IFX': ['ifx'],
            'Internexa': ['internexa'],
            'Columbus': ['columbus'],
            'Fibra': ['fiber', 'fibra', 'ftth'],
            'MPLS': ['mpls'],
            'LTE/4G': ['lte', '4g'],
            '5G': ['5g'],
            'DSL': ['dsl', 'adsl', 'vdsl'],
            'Cable': ['cable', 'docsis'],
        }
        
        for isp_name, patterns in isp_patterns.items():
            if any(p in name_lower for p in patterns):
                return isp_name
        
        return None

    # --------------------------------------------------------------------------
    # HELPER: Construir resumen de canales de Internet
    # --------------------------------------------------------------------------

    def _build_internet_channels_summary(
        self,
        wan_interfaces: List[Dict[str, Any]],
        sdwan_health: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        Construye un resumen consolidado de canales de Internet.

        Combina datos de interfaces WAN + SD-WAN health para proveer
        una vista unificada del estado de todos los ISPs.

        El formato de salida es compatible con el servidor NESS:
        - internet_channels.channels[] con campos estándar
        - internet_channels.summary con conteos

        Args:
            wan_interfaces: Lista de interfaces WAN detectadas.
            sdwan_health: Datos de salud de SD-WAN.

        Returns:
            Diccionario con resumen de canales de Internet.
        """
        channels: List[Dict[str, Any]] = []
        seen_isps: Dict[str, int] = {}  # isp -> índice en channels

        # 1. Canales basados en interfaces WAN
        for wan_if in wan_interfaces:
            isp = wan_if.get("isp_detected") or "Desconocido"
            channel_name = wan_if.get("alias") or wan_if.get("name", "Unknown")
            
            channel: Dict[str, Any] = {
                "channel_name": channel_name,
                "interface_name": wan_if.get("name", ""),
                "isp": isp,
                "source": "wan_interface",
                "oper_status": wan_if.get("oper_status", "unknown"),
                "is_up": wan_if.get("oper_status", "").upper() == "UP",
                "speed_mbps": wan_if.get("speed_mbps", 0),
                "traffic_in_mb": wan_if.get("traffic_in_mb", 0.0),
                "traffic_out_mb": wan_if.get("traffic_out_mb", 0.0),
                "errors_in": wan_if.get("errors_in", 0),
                "errors_out": wan_if.get("errors_out", 0),
                "discards_in": wan_if.get("discards_in", 0),
                "discards_out": wan_if.get("discards_out", 0),
                # SD-WAN metrics (enriched below if available)
                "latency_ms": None,
                "jitter_ms": None,
                "packet_loss_percent": None,
                "health_score": None,
                "sdwan_state": None,
                "bandwidth_in_mbps": None,
                "bandwidth_out_mbps": None,
                "netwatch_status": None,    # N/A en Fortinet (usa SD-WAN)
                "alerts": [],
            }

            # Generar alertas básicas
            channel["alerts"] = self._check_channel_alerts_list(channel)

            if isp != "Desconocido" and isp not in seen_isps:
                seen_isps[isp] = len(channels)
            channels.append(channel)

        # 2. Enriquecer con datos de SD-WAN Health (latencia, jitter, pkt loss)
        if sdwan_health.get("available"):
            for link in sdwan_health.get("links", []):
                link_isp = link.get("isp_detected")
                link_name = link.get("name", "")

                if link_isp and link_isp in seen_isps:
                    # Enriquecer canal WAN existente con métricas SD-WAN
                    idx = seen_isps[link_isp]
                    channels[idx]["latency_ms"] = link.get("latency_ms")
                    channels[idx]["jitter_ms"] = link.get("jitter_ms")
                    channels[idx]["packet_loss_percent"] = link.get("packet_loss_percent")
                    channels[idx]["sdwan_state"] = link.get("state_text")
                    channels[idx]["health_score"] = self._calculate_health_score(link)
                    channels[idx]["bandwidth_in_mbps"] = link.get("bandwidth_in_mbps")
                    channels[idx]["bandwidth_out_mbps"] = link.get("bandwidth_out_mbps")
                    channels[idx]["source"] = "wan_interface+sdwan"
                else:
                    # Enlace SD-WAN sin interfaz WAN asociada — crear canal nuevo
                    channel = {
                        "channel_name": link_name,
                        "interface_name": link_name,
                        "isp": link_isp or "Desconocido",
                        "source": "sdwan",
                        "oper_status": link.get("state_text", "unknown"),
                        "is_up": link.get("state_text", "").lower() == "up",
                        "speed_mbps": 0,
                        "traffic_in_mb": 0.0,
                        "traffic_out_mb": 0.0,
                        "errors_in": 0,
                        "errors_out": 0,
                        "discards_in": 0,
                        "discards_out": 0,
                        "latency_ms": link.get("latency_ms"),
                        "jitter_ms": link.get("jitter_ms"),
                        "packet_loss_percent": link.get("packet_loss_percent"),
                        "health_score": self._calculate_health_score(link),
                        "sdwan_state": link.get("state_text"),
                        "bandwidth_in_mbps": link.get("bandwidth_in_mbps"),
                        "bandwidth_out_mbps": link.get("bandwidth_out_mbps"),
                        "netwatch_status": None,
                        "alerts": [],
                    }
                    channel["alerts"] = self._check_channel_alerts_list(channel)
                    if link_isp and link_isp not in seen_isps:
                        seen_isps[link_isp] = len(channels)
                    channels.append(channel)

        # 3. Enriquecer con SD-WAN Members (volumen, sesiones)
        if sdwan_health.get("available"):
            for member in sdwan_health.get("members", []):
                m_isp = member.get("isp_detected")
                if m_isp and m_isp in seen_isps:
                    idx = seen_isps[m_isp]
                    channels[idx]["sdwan_member_state"] = member.get("state")
                    channels[idx]["sdwan_volume_in_gb"] = member.get("volume_in_gb", 0.0)
                    channels[idx]["sdwan_volume_out_gb"] = member.get("volume_out_gb", 0.0)
                    channels[idx]["sdwan_sessions"] = member.get("sessions", 0)

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
                "sdwan_available": sdwan_health.get("available", False),
            },
        }

    # --------------------------------------------------------------------------
    # HELPER: Alertas por canal (formato lista de strings)
    # --------------------------------------------------------------------------

    def _check_channel_alerts_list(self, channel: Dict[str, Any]) -> List[str]:
        """
        Genera alertas para un canal como lista de strings (formato estándar NESS).

        Returns:
            Lista de strings con alertas detectadas.
        """
        alerts: List[str] = []

        # Canal caído
        if channel.get("oper_status", "").upper() == "DOWN":
            alerts.append(f"Canal WAN DOWN: {channel.get('channel_name', 'desconocido')}")

        # Errores altos
        errors_in = channel.get("errors_in", 0) or 0
        errors_out = channel.get("errors_out", 0) or 0
        if errors_in + errors_out > 100:
            alerts.append(
                f"Alto número de errores en interfaz WAN: "
                f"IN={errors_in} OUT={errors_out}"
            )

        # Descartes altos
        discards = (channel.get("discards_in", 0) or 0) + (channel.get("discards_out", 0) or 0)
        if discards > 500:
            alerts.append(f"Descartes elevados en interfaz WAN: {discards} paquetes")

        # Latencia alta
        latency = channel.get("latency_ms")
        if latency is not None and latency > 100:
            alerts.append(f"Latencia alta: {latency}ms")

        # Packet loss
        pkt_loss = channel.get("packet_loss_percent")
        if pkt_loss is not None and pkt_loss > 1:
            alerts.append(f"Pérdida de paquetes: {pkt_loss}%")

        # Jitter alto
        jitter = channel.get("jitter_ms")
        if jitter is not None and jitter > 30:
            alerts.append(f"Jitter alto: {jitter}ms")

        return alerts

    # --------------------------------------------------------------------------
    # HELPER: Calcular health score de un canal
    # --------------------------------------------------------------------------

    def _calculate_health_score(self, link_data: Dict[str, Any]) -> int:
        """
        Calcula un score de salud (0-100) para un enlace.

        Score basado en:
        - Estado (up/down): +50 puntos si up
        - Latencia: hasta -20 puntos
        - Jitter: hasta -15 puntos  
        - Packet Loss: hasta -15 puntos

        Args:
            link_data: Datos del enlace con métricas.

        Returns:
            Score de 0 a 100.
        """
        score = 100
        
        # Estado base: si down, score máximo es 50
        state = link_data.get("state_text", "").lower()
        if state != "up":
            score = 50
        
        # Penalizar por latencia alta (umbral: >100ms es malo)
        latency = link_data.get("latency_ms")
        if latency is not None:
            if latency > 200:
                score -= 20
            elif latency > 100:
                score -= 15
            elif latency > 50:
                score -= 10
            elif latency > 20:
                score -= 5
        
        # Penalizar por jitter alto (umbral: >30ms es malo)
        jitter = link_data.get("jitter_ms")
        if jitter is not None:
            if jitter > 50:
                score -= 15
            elif jitter > 30:
                score -= 10
            elif jitter > 15:
                score -= 5
        
        # Penalizar por packet loss (cualquier loss es malo)
        pkt_loss = link_data.get("packet_loss_percent")
        if pkt_loss is not None:
            if pkt_loss > 5:
                score -= 15
            elif pkt_loss > 2:
                score -= 10
            elif pkt_loss > 0.5:
                score -= 5
            elif pkt_loss > 0:
                score -= 2
        
        return max(0, min(100, score))

    # ==========================================================================
    # DETECCIÓN AUTOMÁTICA
    # ==========================================================================

    @classmethod
    def matches_sys_object_id(cls, sys_object_id: str) -> bool:
        """
        Detecta si un sysObjectID corresponde a Fortinet.

        Fortinet usa enterprise OID 1.3.6.1.4.1.12356.
        FortiGate específicamente: 1.3.6.1.4.1.12356.101.1.*
        """
        if not sys_object_id:
            return False
        return any(
            sys_object_id.startswith(prefix)
            for prefix in FORTINET_SYS_OBJECT_IDS
        )
