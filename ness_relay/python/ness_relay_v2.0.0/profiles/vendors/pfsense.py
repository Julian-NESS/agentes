# ==============================================================================
# NESS Relay v2.0.0 - pfSense Vendor Profile
# ==============================================================================
# Perfil completo para dispositivos pfSense (FreeBSD-based).
#
# CPU/Memory/Disk: UCD-SNMP-MIB (estándar en FreeBSD/net-snmp)
# Firewall States: PF-MIB (específico de pfSense/OpenBSD Packet Filter)
# WAN/Internet:    IF-MIB (RFC 2863) — detección de interfaces WAN
#                  + Gateway Monitoring via pfSense custom OIDs (si disponibles)
#
# Este perfil incluye monitoreo de canales de Internet (WAN interfaces)
# con detección automática de ISP, estado operativo y throughput.
# ==============================================================================

import logging
from typing import Any, Dict, List, Optional

from profiles.base_profile import BaseDeviceProfile
from utils.conversions import (
    calculate_percentage,
    kb_to_gb,
    safe_division,
    safe_float,
    safe_int,
)
from utils.helpers import now_iso

logger = logging.getLogger("ness_relay")


# ==============================================================================
# OIDs ESPECÍFICOS DE pfSense
# ==============================================================================

# CPU OIDs - UCD-SNMP-MIB (FreeBSD/net-snmp)
PFSENSE_CPU_OIDS: Dict[str, str] = {
    'cpu_load_1min':  '1.3.6.1.4.1.2021.10.1.3.1',  # laLoad.1 (1 min load average)
    'cpu_load_5min':  '1.3.6.1.4.1.2021.10.1.3.2',  # laLoad.2 (5 min load average)
    'cpu_load_15min': '1.3.6.1.4.1.2021.10.1.3.3',  # laLoad.3 (15 min load average)
    'cpu_user':       '1.3.6.1.4.1.2021.11.9.0',     # ssCpuUser
    'cpu_system':     '1.3.6.1.4.1.2021.11.10.0',    # ssCpuSystem
    'cpu_idle':       '1.3.6.1.4.1.2021.11.11.0',    # ssCpuIdle
    'cpu_interrupt':  '1.3.6.1.4.1.2021.11.56.0',    # ssCpuRawInterrupt
    'cpu_num_cpus':   '1.3.6.1.2.1.25.3.3.1.2',     # hrProcessorLoad (tabla)
}

# Memory OIDs - UCD-SNMP-MIB
PFSENSE_MEMORY_OIDS: Dict[str, str] = {
    'mem_total_real': '1.3.6.1.4.1.2021.4.5.0',   # memTotalReal (KB)
    'mem_avail_real': '1.3.6.1.4.1.2021.4.6.0',   # memAvailReal (KB) - Memoria disponible real
    'mem_total_free': '1.3.6.1.4.1.2021.4.11.0',  # memTotalFree (KB) - Solo free puro
    'mem_cached':     '1.3.6.1.4.1.2021.4.15.0',  # memCached (KB)
    'mem_buffer':     '1.3.6.1.4.1.2021.4.14.0',  # memBuffer (KB)
    'swap_total':     '1.3.6.1.4.1.2021.4.3.0',   # memTotalSwap (KB)
    'swap_free':      '1.3.6.1.4.1.2021.4.4.0',   # memAvailSwap (KB)
}

# Disk OIDs - UCD-SNMP-MIB (tabla)
PFSENSE_DISK_OIDS: Dict[str, str] = {
    'disk_path':    '1.3.6.1.4.1.2021.9.1.2',   # dskPath
    'disk_device':  '1.3.6.1.4.1.2021.9.1.3',   # dskDevice
    'disk_total':   '1.3.6.1.4.1.2021.9.1.6',   # dskTotal (KB)
    'disk_used':    '1.3.6.1.4.1.2021.9.1.8',   # dskUsed (KB)
    'disk_percent': '1.3.6.1.4.1.2021.9.1.9',   # dskPercent
}

# pfSense PF-MIB OIDs (Packet Filter - Firewall)
PFSENSE_PF_OIDS: Dict[str, str] = {
    'pf_states_current':  '1.3.6.1.4.1.12325.1.200.1.3.1.0',  # pfStateTableCount
    'pf_states_searches': '1.3.6.1.4.1.12325.1.200.1.3.2.0',  # pfStateTableSearches
    'pf_states_inserts':  '1.3.6.1.4.1.12325.1.200.1.3.3.0',  # pfStateTableInserts
    'pf_states_removals': '1.3.6.1.4.1.12325.1.200.1.3.4.0',  # pfStateTableRemovals
    'pf_log_entries':     '1.3.6.1.4.1.12325.1.200.1.5.2.0',  # pfLogInterfaceBytesIn
    'pf_log_bytes':       '1.3.6.1.4.1.12325.1.200.1.5.3.0',  # pfLogInterfaceBytesOut
    'pf_block_packets':   '1.3.6.1.4.1.12325.1.200.1.2.1.0',  # pfCounterMatch (blocked)
    'pf_block_bytes':     '1.3.6.1.4.1.12325.1.200.1.2.2.0',  # pfCounterBadOffset
}

# ==============================================================================
# OIDs PARA MONITOREO DE CANALES DE INTERNET / WAN
# ==============================================================================

# IF-MIB estándar para interfaces (RFC 2863)
PFSENSE_INTERFACE_OIDS: Dict[str, str] = {
    # IF-MIB estándar
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
    'if_alias':              '1.3.6.1.2.1.31.1.1.1.18',       # ifAlias (descripción user)
    # Unicast packets
    'if_in_ucast_pkts':      '1.3.6.1.2.1.2.2.1.11',          # ifInUcastPkts
    'if_out_ucast_pkts':     '1.3.6.1.2.1.2.2.1.17',          # ifOutUcastPkts
    'if_hc_in_ucast_pkts':   '1.3.6.1.2.1.31.1.1.1.7',        # ifHCInUcastPkts
    'if_hc_out_ucast_pkts':  '1.3.6.1.2.1.31.1.1.1.11',       # ifHCOutUcastPkts
}

# ==============================================================================
# PATRONES DE DETECCIÓN DE INTERFACES WAN EN pfSense
# ==============================================================================
# pfSense (FreeBSD) usa nombres de interfaz del driver de red:
#   - igb0, igb1 (Intel I210/I350/I211)
#   - em0, em1   (Intel e1000/PRO 1000)
#   - ix0, ix1   (Intel X520/X540/X710 10GbE)
#   - bge0       (Broadcom)
#   - re0        (Realtek)
#   - lagg0      (LACP/failover agregación)
#   - vlan100    (VLAN tags)
#   - ovpnc1     (OpenVPN client — túnel WAN)
#   - pppoe0     (PPPoE WAN)
#   - gif0       (tunnel)
#
# En pfSense, la primera interfaz (igb0/em0/re0) es el WAN por defecto.
# Administradores nombran interfaces: WAN, WAN2, WANTIGO, WANETB, OPT1, etc.
# El ifAlias (descripción) suele llevar el nombre del ISP.

WAN_INTERFACE_PATTERNS = [
    # Nombres de asignación de pfSense
    'wan', 'opt1', 'opt2', 'opt3', 'opt4',
    'internet', 'isp', 'uplink', 'externa', 'external',
    'primary', 'secondary', 'backup', 'principal', 'respaldo',
    # PPPoE / Túneles WAN
    'pppoe', 'ovpn', 'gif', 'gre',
    # Proveedores Colombia y LATAM (en alias/descr)
    'etb', 'tigo', 'claro', 'movistar', 'azteca', 'att',
    'une', 'millicom', 'telefonica', 'supercanal', 'edatel',
    'fibra', 'fiber', 'dsl', 'adsl', 'cable', 'mpls',
    'lte', '4g', '5g', 'celular',
]

# sysObjectID prefijos para detección automática de pfSense
PFSENSE_SYS_OBJECT_IDS = [
    '1.3.6.1.4.1.12325',     # FreeBSD Foundation
    '1.3.6.1.4.1.8072.3.2',  # net-snmp (usado por pfSense)
]


# ==============================================================================
# PROFILE CLASS
# ==============================================================================

class PfSenseProfile(BaseDeviceProfile):
    """
    Perfil de dispositivo para pfSense (FreeBSD + Packet Filter).
    
    Características:
    - CPU via UCD-SNMP-MIB (laLoad, ssCpu*)
    - Memoria via UCD-SNMP-MIB (memTotalReal, memAvailReal)
    - Disco via UCD-SNMP-MIB (dskTable)
    - Firewall via PF-MIB (estados PF, logs, bloqueos)
    """
    
    vendor = "pfsense"
    vendor_display_name = "pfSense (FreeBSD)"
    device_type = "firewall"
    
    # ==========================================================================
    # OIDs
    # ==========================================================================
    
    def get_vendor_oids(self) -> Dict[str, str]:
        """Retorna OIDs específicos de PF-MIB + IF-MIB para canales WAN."""
        oids = PFSENSE_PF_OIDS.copy()
        oids.update(PFSENSE_INTERFACE_OIDS)
        return oids
    
    def get_cpu_oids(self) -> Dict[str, str]:
        """Retorna OIDs de CPU via UCD-SNMP-MIB."""
        return PFSENSE_CPU_OIDS.copy()
    
    def get_memory_oids(self) -> Dict[str, str]:
        """Retorna OIDs de memoria via UCD-SNMP-MIB."""
        return PFSENSE_MEMORY_OIDS.copy()
    
    def get_disk_oids(self) -> Dict[str, str]:
        """Retorna OIDs de disco via UCD-SNMP-MIB."""
        return PFSENSE_DISK_OIDS.copy()
    
    # ==========================================================================
    # NORMALIZACIÓN DE CPU
    # ==========================================================================
    
    def normalize_cpu_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de CPU de pfSense/FreeBSD.
        
        pfSense usa UCD-SNMP-MIB:
        - ssCpuIdle es el porcentaje de CPU libre
        - cpu_usage = 100 - ssCpuIdle
        - laLoad.* son los load averages de 1/5/15 min
        
        Args:
            raw_data: Datos crudos {oid_name: value} de las queries SNMP.
            
        Returns:
            Datos normalizados de CPU en formato estándar NESS.
        """
        cpu_idle = safe_float(raw_data.get('cpu_idle'))
        
        # Si tenemos cpu_idle, calcular uso
        if cpu_idle > 0 or raw_data.get('cpu_idle') is not None:
            cpu_usage = round(100.0 - cpu_idle, 2)
        else:
            cpu_usage = 0.0
        
        return {
            "cpu_usage_percent": cpu_usage,
            "load_1min": safe_float(raw_data.get('cpu_load_1min')),
            "load_5min": safe_float(raw_data.get('cpu_load_5min')),
            "load_15min": safe_float(raw_data.get('cpu_load_15min')),
        }
    
    # ==========================================================================
    # NORMALIZACIÓN DE MEMORIA
    # ==========================================================================
    
    def normalize_memory_data(self, raw_data: Dict[str, Any]) -> Dict[str, Any]:
        """
        Normaliza datos de memoria de pfSense/FreeBSD.
        
        CORREGIDO (herencia de v1.0.4): En FreeBSD/pfSense, mem_total_free
        es muy pequeño porque no incluye buffers/caché. mem_avail_real
        incluye memoria que puede ser liberada (caché, buffers, etc.)
        y es lo que pfSense muestra en su dashboard.
        
        Args:
            raw_data: Datos crudos {oid_name: value} de las queries SNMP.
            
        Returns:
            Datos normalizados de memoria en formato estándar NESS.
        """
        mem_total = safe_int(raw_data.get('mem_total_real'))
        mem_avail = safe_int(raw_data.get('mem_avail_real'))
        mem_free_raw = safe_int(raw_data.get('mem_total_free'))
        mem_cached = safe_int(raw_data.get('mem_cached'))
        mem_buffer = safe_int(raw_data.get('mem_buffer'))
        swap_total = safe_int(raw_data.get('swap_total'))
        swap_free = safe_int(raw_data.get('swap_free'))
        
        if mem_total > 0:
            # Cálculo correcto de memoria usada:
            # Si tenemos mem_avail_real, usarlo (es lo que pfSense muestra)
            # Si no, usar mem_total_free + cached + buffer como aproximación
            if mem_avail > 0:
                mem_used = max(0, mem_total - mem_avail)
                mem_free = mem_avail
            else:
                # Fallback: considerar cached y buffer como disponible
                mem_free = mem_free_raw + mem_cached + mem_buffer
                mem_used = max(0, mem_total - mem_free)
            
            return {
                "mem_usage_percent": round(calculate_percentage(mem_used, mem_total), 2),
                "mem_total_mb": round(mem_total / 1024.0, 2),
                "mem_used_mb": round(mem_used / 1024.0, 2),
                "mem_free_mb": round(mem_free / 1024.0, 2),
                "mem_available_mb": round(mem_avail / 1024.0, 2) if mem_avail > 0 else None,
                "mem_cached_mb": round(mem_cached / 1024.0, 2) if mem_cached > 0 else None,
                "mem_buffer_mb": round(mem_buffer / 1024.0, 2) if mem_buffer > 0 else None,
                "swap_total_mb": round(swap_total / 1024.0, 2),
                "swap_free_mb": round(swap_free / 1024.0, 2),
            }
        else:
            return {"error": "No memory data"}
    
    # ==========================================================================
    # NORMALIZACIÓN DE DISCO
    # ==========================================================================
    
    def normalize_disk_data(self, raw_disk_entries: Dict[str, Dict[str, Any]]) -> Dict[str, Dict[str, Any]]:
        """
        Normaliza datos de disco de pfSense/FreeBSD.
        
        Los valores crudos de UCD-SNMP-MIB están en KB.
        Convierte a GB y calcula porcentajes coherentes.
        
        Args:
            raw_disk_entries: Diccionario {idx: {oid_name: value}} de tablas SNMP.
            
        Returns:
            Diccionario normalizado {idx: datos_normalizados}.
        """
        disk_data: Dict[str, Dict[str, Any]] = {}
        
        for idx, raw in raw_disk_entries.items():
            path = raw.get('disk_path') or raw.get('disk_device') or f"idx_{idx}"
            total_raw = safe_int(raw.get('disk_total'))    # KB
            used_raw = safe_int(raw.get('disk_used'))      # KB
            percent_raw = safe_float(raw.get('disk_percent'))
            
            # Si total está vacío pero tenemos percent y used, inferir total
            inferred_total = total_raw
            if inferred_total == 0 and percent_raw > 0 and used_raw > 0:
                try:
                    inferred_total = int((used_raw * 100.0) / percent_raw)
                except Exception:
                    inferred_total = total_raw
            
            total_gb = kb_to_gb(inferred_total)
            used_gb = kb_to_gb(used_raw)
            
            # Porcentaje: preferir disk_percent si disponible y plausible
            percent_calc = percent_raw if percent_raw > 0 else (
                safe_division(used_raw, inferred_total) if inferred_total > 0 else 0.0
            )
            
            disk_data[idx] = {
                "index": idx,
                "path": str(path),
                "total_gb": total_gb,
                "used_gb": used_gb,
                "free_gb": round(max(0.0, total_gb - used_gb), 3),
                "percent_used": round(percent_calc, 2),
                "source_raw": {
                    "disk_total_raw_gb": kb_to_gb(total_raw),
                    "disk_used_raw_gb": kb_to_gb(used_raw),
                    "disk_percent_raw": percent_raw
                }
            }
        
        return disk_data
    
    # ==========================================================================
    # DATOS ESPECÍFICOS DE pfSense (PF-MIB)
    # ==========================================================================
    
    async def collect_vendor_specific_data(self, client: 'SnmpClient') -> Dict[str, Any]:
        """
        Recolecta datos específicos del Packet Filter de pfSense + Canales de Internet.
        
        Obtiene:
        - Estados del firewall (conexiones activas, búsquedas, inserciones, remociones)
        - Logs del firewall (entradas, bytes, paquetes bloqueados)
        - **CANALES DE INTERNET/WAN:**
          - Interfaces WAN identificadas por patrón de nombre (igb0, em0, wan, opt1...)
          - Estado operativo, velocidad, tráfico por interfaz WAN
          - Detección automática de ISP (ETB, Tigo, Claro, etc.)
        
        Args:
            client: Instancia de SnmpClient conectada al pfSense.
            
        Returns:
            Diccionario con datos de PF-MIB + canales de Internet.
        """
        logger.info("Recolectando datos específicos de pfSense (PF-MIB + WAN)...")
        
        vendor_oids = self.get_vendor_oids()
        
        pfsense_data: Dict[str, Any] = {
            "firewall_states": {},
            "firewall_logs": {},
            "wan_interfaces": [],
            "internet_channels": {},
            "collection_timestamp": now_iso()
        }
        
        # ===== Estados del Packet Filter =====
        pf_state_oid_names = ['pf_states_current', 'pf_states_searches', 'pf_states_inserts', 'pf_states_removals']
        
        for oid_name in pf_state_oid_names:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    pfsense_data["firewall_states"][oid_name] = safe_int(res.value)
                else:
                    pfsense_data["firewall_states"][oid_name] = {"error": res.error}
        
        # ===== Logs del Packet Filter =====
        pf_log_oid_names = ['pf_log_entries', 'pf_log_bytes', 'pf_block_packets', 'pf_block_bytes']
        
        for oid_name in pf_log_oid_names:
            oid = vendor_oids.get(oid_name)
            if oid:
                res = await client.get(oid)
                if not res.error and res.value is not None:
                    pfsense_data["firewall_logs"][oid_name] = safe_int(res.value)
                else:
                    pfsense_data["firewall_logs"][oid_name] = {"error": res.error}
        
        # =====================================================================
        # MONITOREO DE CANALES DE INTERNET / WAN
        # =====================================================================
        
        logger.info("Recolectando interfaces WAN y canales de Internet...")
        
        # 1. Interfaces WAN (IF-MIB — detectadas por patrón de nombre)
        pfsense_data["wan_interfaces"] = await self._collect_wan_interfaces(client, vendor_oids)
        
        # 2. Resumen de canales de Internet (basado en WAN interfaces)
        pfsense_data["internet_channels"] = self._build_internet_channels_summary(
            pfsense_data["wan_interfaces"],
        )
        
        logger.info("Datos específicos de pfSense recolectados exitosamente")
        return pfsense_data
    
    # ==========================================================================
    # HELPER: Interfaces WAN (IF-MIB)
    # ==========================================================================

    async def _collect_wan_interfaces(
        self, client: 'SnmpClient', vendor_oids: Dict[str, str]
    ) -> List[Dict[str, Any]]:
        """
        Detecta y recolecta métricas de interfaces WAN via IF-MIB.

        pfSense (FreeBSD) usa:
        - ifDescr: nombre del driver (igb0, em0, ix0, re0, lagg0)
        - ifName:  nombre corto de la interfaz
        - ifAlias: descripción del administrador (suele tener WAN, ISP, etc.)

        La detección combina patrones en los tres campos. En pfSense,
        el administrador asigna nombres como WAN, WAN2, WANETB en la GUI,
        que se reflejan en ifAlias o ifDescr dependiendo de la configuración.

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
                logger.debug(f"pfSense: ifDescr no disponible: {error}")
                return wan_interfaces

            # Construir mapa de interfaces
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

            # Añadir ifName (nombre corto de FreeBSD)
            if_name_oid = vendor_oids.get('if_name')
            if if_name_oid:
                name_results, _ = await client.bulk(if_name_oid)
                for oid_str, value in (name_results or []):
                    idx = oid_str.split('.')[-1]
                    if idx in interface_map:
                        short_name = str(value).strip()
                        interface_map[idx]["if_name"] = short_name
                        if any(p in short_name.lower() for p in WAN_INTERFACE_PATTERNS):
                            interface_map[idx]["is_wan"] = True
                        if not interface_map[idx]["isp_detected"]:
                            interface_map[idx]["isp_detected"] = self._detect_isp_from_name(short_name)

            # Añadir ifAlias (descripción del administrador — clave para ISP detection)
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

            # Recolectar métricas solo para interfaces WAN
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

            # Fallback: if_speed (bps) si ifHighSpeed no disponible
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

            logger.info(f"pfSense: {len(wan_interfaces)} interfaces WAN detectadas")

        except Exception as e:
            logger.warning(f"pfSense: Error recolectando interfaces WAN: {e}")

        return wan_interfaces

    # ==========================================================================
    # HELPER: Resumen de Canales de Internet
    # ==========================================================================

    def _build_internet_channels_summary(
        self,
        wan_interfaces: List[Dict[str, Any]],
    ) -> Dict[str, Any]:
        """
        Construye un resumen consolidado de canales de Internet.

        pfSense NO tiene Netwatch ni SD-WAN como MikroTik/Fortinet.
        Los canales se construyen exclusivamente desde IF-MIB:
        - Estado operativo (up/down) de cada interfaz WAN
        - Throughput medido en la interfaz
        - ISP detectado por nombre de interfaz/alias

        Returns:
            Diccionario con canales y resumen global.
        """
        channels: List[Dict[str, Any]] = []

        for iface in wan_interfaces:
            channel_name = iface.get("alias") or iface.get("if_name") or iface.get("name", "")
            isp = iface.get("isp_detected")

            channel: Dict[str, Any] = {
                "channel_name": channel_name,
                "interface_name": iface.get("name", ""),
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
                "latency_ms": None,         # pfSense no expone latencia via SNMP
                "packet_loss_percent": None, # pfSense no expone packet loss via SNMP
                "health_score": None,        # Sin métricas de salud SNMP
                "netwatch_status": None,     # N/A en pfSense
                "alerts": [],
            }

            # Generar alertas
            channel["alerts"] = self._check_channel_alerts(channel)
            channels.append(channel)

        # Calcular summary global
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
                "netwatch_available": False,  # pfSense no tiene Netwatch
                "queues_available": False,    # pfSense no tiene Queue Simple
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

        errors_in = channel.get("errors_in", 0) or 0
        errors_out = channel.get("errors_out", 0) or 0
        if errors_in + errors_out > 100:
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
        con los nombres que los administradores de red configuran en pfSense
        (via GUI: Interfaces > WAN > Description).

        Args:
            name: Nombre de la interfaz, alias o descripción.

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
            'Level3':     ['level3', 'lumen', 'centurylink'],
            'GTD':        ['gtd'],
            'IFX':        ['ifx'],
            'Columbus':   ['columbus'],
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
        Detecta si un sysObjectID corresponde a pfSense/FreeBSD.
        
        pfSense usa OIDs de FreeBSD Foundation (1.3.6.1.4.1.12325)
        o net-snmp genérico (1.3.6.1.4.1.8072.3.2).
        """
        if not sys_object_id:
            return False
        return any(sys_object_id.startswith(prefix) for prefix in PFSENSE_SYS_OBJECT_IDS)
