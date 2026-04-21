# ==============================================================================
# NESS Relay v2.0.0 - Standard OIDs (RFC Universales)
# ==============================================================================
# OIDs estándar RFC que son comunes a TODOS los dispositivos SNMP,
# independientemente del vendor (pfSense, Cisco, Fortinet, MikroTik, etc.).
#
# REGLA DE ORO: Estos OIDs son "universales" y cualquier dispositivo que
# soporte SNMP debería responderlos.
#
# OIDs específicos de vendor (PF-MIB, CISCO-MIB, etc.) van en cada
# archivo de perfil en profiles/vendors/*.py
# ==============================================================================

from typing import Dict

# ==============================================================================
# SYSTEM OIDs (RFC 1213 / SNMPv2-MIB)
# ==============================================================================

SYSTEM_OIDS: Dict[str, str] = {
    'sys_descr':    '1.3.6.1.2.1.1.1.0',     # sysDesc - Descripción del sistema
    'sys_objectid': '1.3.6.1.2.1.1.2.0',     # sysObjectID - OID del fabricante
    'sys_uptime':   '1.3.6.1.2.1.1.3.0',     # sysUpTime - Tiempo encendido (TimeTicks)
    'sys_contact':  '1.3.6.1.2.1.1.4.0',     # sysContact - Contacto administrativo
    'sys_name':     '1.3.6.1.2.1.1.5.0',     # sysName - Nombre del dispositivo
    'sys_location': '1.3.6.1.2.1.1.6.0',     # sysLocation - Ubicación física
}

# ==============================================================================
# INTERFACE OIDs (RFC 2863 - IF-MIB)
# ==============================================================================

INTERFACE_OIDS: Dict[str, str] = {
    # Información básica de interfaz
    'if_descr':         '1.3.6.1.2.1.2.2.1.2',   # ifDescr - Nombre/descripción
    'if_type':          '1.3.6.1.2.1.2.2.1.3',   # ifType - Tipo de interfaz
    'if_speed':         '1.3.6.1.2.1.2.2.1.5',   # ifSpeed - Velocidad (bps)
    'if_admin_status':  '1.3.6.1.2.1.2.2.1.7',   # ifAdminStatus - Estado admin
    'if_oper_status':   '1.3.6.1.2.1.2.2.1.8',   # ifOperStatus - Estado operativo
    
    # Contadores de tráfico (32-bit)
    'if_in_octets':     '1.3.6.1.2.1.2.2.1.10',  # ifInOctets
    'if_out_octets':    '1.3.6.1.2.1.2.2.1.16',  # ifOutOctets
    
    # Contadores de errores
    'if_in_errors':     '1.3.6.1.2.1.2.2.1.14',  # ifInErrors
    'if_out_errors':    '1.3.6.1.2.1.2.2.1.20',  # ifOutErrors
    'if_in_discards':   '1.3.6.1.2.1.2.2.1.13',  # ifInDiscards
    'if_out_discards':  '1.3.6.1.2.1.2.2.1.19',  # ifOutDiscards
}

# ==============================================================================
# HIGH-CAPACITY INTERFACE OIDs (RFC 2863 - IF-MIB, 64-bit counters)
# ==============================================================================

HC_INTERFACE_OIDS: Dict[str, str] = {
    'if_high_speed':    '1.3.6.1.2.1.31.1.1.1.15',  # ifHighSpeed (Mbps)
    'if_hc_in_octets':  '1.3.6.1.2.1.31.1.1.1.6',   # ifHCInOctets (64-bit)
    'if_hc_out_octets': '1.3.6.1.2.1.31.1.1.1.10',  # ifHCOutOctets (64-bit)
}

# ==============================================================================
# TCP OIDs (RFC 4022 - TCP-MIB)
# ==============================================================================

TCP_OIDS: Dict[str, str] = {
    'tcp_active_opens':  '1.3.6.1.2.1.6.5.0',   # tcpActiveOpens
    'tcp_passive_opens': '1.3.6.1.2.1.6.6.0',   # tcpPassiveOpens
    'tcp_attempt_fails': '1.3.6.1.2.1.6.7.0',   # tcpAttemptFails
    'tcp_estab_resets':  '1.3.6.1.2.1.6.8.0',   # tcpEstabResets
    'tcp_curr_estab':    '1.3.6.1.2.1.6.9.0',   # tcpCurrEstab
    'tcp_in_segs':       '1.3.6.1.2.1.6.10.0',  # tcpInSegs
    'tcp_out_segs':      '1.3.6.1.2.1.6.11.0',  # tcpOutSegs
    'tcp_retrans_segs':  '1.3.6.1.2.1.6.12.0',  # tcpRetransSegs
    'tcp_in_errs':       '1.3.6.1.2.1.6.14.0',  # tcpInErrs
    'tcp_out_rsts':      '1.3.6.1.2.1.6.15.0',  # tcpOutRsts
}

# ==============================================================================
# UDP OIDs (RFC 4113 - UDP-MIB)
# ==============================================================================

UDP_OIDS: Dict[str, str] = {
    'udp_in_datagrams':  '1.3.6.1.2.1.7.1.0',   # udpInDatagrams
    'udp_out_datagrams': '1.3.6.1.2.1.7.4.0',   # udpOutDatagrams
    'udp_no_ports':      '1.3.6.1.2.1.7.2.0',   # udpNoPorts
    'udp_in_errors':     '1.3.6.1.2.1.7.3.0',   # udpInErrors
}

# ==============================================================================
# IP OIDs (RFC 4293 - IP-MIB)
# ==============================================================================

IP_OIDS: Dict[str, str] = {
    'ip_in_receives':       '1.3.6.1.2.1.4.3.0',   # ipInReceives
    'ip_in_hdr_errors':     '1.3.6.1.2.1.4.4.0',   # ipInHdrErrors
    'ip_in_addr_errors':    '1.3.6.1.2.1.4.5.0',   # ipInAddrErrors
    'ip_forw_datagrams':    '1.3.6.1.2.1.4.6.0',   # ipForwDatagrams
    'ip_in_unknown_protos': '1.3.6.1.2.1.4.7.0',   # ipInUnknownProtos
    'ip_in_discards':       '1.3.6.1.2.1.4.8.0',   # ipInDiscards
    'ip_in_delivers':       '1.3.6.1.2.1.4.9.0',   # ipInDelivers
    'ip_out_requests':      '1.3.6.1.2.1.4.10.0',  # ipOutRequests
    'ip_out_discards':      '1.3.6.1.2.1.4.11.0',  # ipOutDiscards
    'ip_out_no_routes':     '1.3.6.1.2.1.4.12.0',  # ipOutNoRoutes
    'ip_frag_oks':          '1.3.6.1.2.1.4.17.0',  # ipFragOKs
    'ip_frag_fails':        '1.3.6.1.2.1.4.18.0',  # ipFragFails
    'ip_frag_creates':      '1.3.6.1.2.1.4.19.0',  # ipFragCreates
    'ip_routing_discards':  '1.3.6.1.2.1.4.23.0',  # ipRoutingDiscards
}

# ==============================================================================
# ICMP OIDs (RFC 2011 - ICMP group)
# ==============================================================================

ICMP_OIDS: Dict[str, str] = {
    'icmp_in_msgs':           '1.3.6.1.2.1.5.1.0',   # icmpInMsgs
    'icmp_in_errors':         '1.3.6.1.2.1.5.2.0',   # icmpInErrors
    'icmp_in_dest_unreachs':  '1.3.6.1.2.1.5.3.0',   # icmpInDestUnreachs
    'icmp_in_time_excds':     '1.3.6.1.2.1.5.4.0',   # icmpInTimeExcds
    'icmp_in_parm_probs':     '1.3.6.1.2.1.5.5.0',   # icmpInParmProbs
    'icmp_in_src_quenchs':    '1.3.6.1.2.1.5.6.0',   # icmpInSrcQuenchs
    'icmp_in_redirects':      '1.3.6.1.2.1.5.7.0',   # icmpInRedirects
    'icmp_in_echos':          '1.3.6.1.2.1.5.8.0',   # icmpInEchos
    'icmp_in_echo_reps':      '1.3.6.1.2.1.5.9.0',   # icmpInEchoReps
    'icmp_out_msgs':          '1.3.6.1.2.1.5.14.0',  # icmpOutMsgs
    'icmp_out_errors':        '1.3.6.1.2.1.5.15.0',  # icmpOutErrors
}

# ==============================================================================
# SNMP STATS OIDs (RFC 3418 - SNMPv2-MIB)
# ==============================================================================

SNMP_STATS_OIDS: Dict[str, str] = {
    'snmp_in_pkts':                '1.3.6.1.2.1.11.1.0',   # snmpInPkts
    'snmp_out_pkts':               '1.3.6.1.2.1.11.2.0',   # snmpOutPkts
    'snmp_in_bad_versions':        '1.3.6.1.2.1.11.3.0',   # snmpInBadVersions
    'snmp_in_bad_community_names': '1.3.6.1.2.1.11.4.0',   # snmpInBadCommunityNames
    'snmp_in_bad_community_uses':  '1.3.6.1.2.1.11.5.0',   # snmpInBadCommunityUses
    'snmp_in_asn_parse_errs':      '1.3.6.1.2.1.11.6.0',   # snmpInASNParseErrs
    'snmp_in_too_bigs':            '1.3.6.1.2.1.11.8.0',   # snmpInTooBigs
    'snmp_in_no_such_names':       '1.3.6.1.2.1.11.9.0',   # snmpInNoSuchNames
    'snmp_in_bad_values':          '1.3.6.1.2.1.11.10.0',  # snmpInBadValues
    'snmp_in_gen_errs':            '1.3.6.1.2.1.11.12.0',  # snmpInGenErrs
    'snmp_in_get_requests':        '1.3.6.1.2.1.11.15.0',  # snmpInGetRequests
}


# ==============================================================================
# DICCIONARIO COMPLETO DE OIDs ESTÁNDAR
# ==============================================================================

def get_all_standard_oids() -> Dict[str, str]:
    """
    Retorna todos los OIDs estándar RFC combinados en un solo diccionario.
    
    Returns:
        Diccionario con todos los OIDs estándar {nombre: oid_string}.
    """
    all_oids: Dict[str, str] = {}
    all_oids.update(SYSTEM_OIDS)
    all_oids.update(INTERFACE_OIDS)
    all_oids.update(HC_INTERFACE_OIDS)
    all_oids.update(TCP_OIDS)
    all_oids.update(UDP_OIDS)
    all_oids.update(IP_OIDS)
    all_oids.update(ICMP_OIDS)
    all_oids.update(SNMP_STATS_OIDS)
    return all_oids


# Instancia pre-calculada para acceso rápido
STANDARD_OIDS = get_all_standard_oids()
