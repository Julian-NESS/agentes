// ==============================================================================
// NESS Relay v2.0.0 — OIDs Estándar RFC
// Equivalente Python: profiles/standard_oids.py
// ==============================================================================
// Colección completa de OIDs estándar de los MIBs RFC más comunes
// usados en monitoreo de red multi-vendor.
// ==============================================================================

use std::collections::HashMap;

// ==============================================================================
// SYSTEM OIDS (RFC 1213 / SNMPv2-MIB)
// ==============================================================================
pub fn system_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("sysDescr",     "1.3.6.1.2.1.1.1.0");
    m.insert("sysObjectID",  "1.3.6.1.2.1.1.2.0");
    m.insert("sysUpTime",    "1.3.6.1.2.1.1.3.0");
    m.insert("sysContact",   "1.3.6.1.2.1.1.4.0");
    m.insert("sysName",      "1.3.6.1.2.1.1.5.0");
    m.insert("sysLocation",  "1.3.6.1.2.1.1.6.0");
    m.insert("sysServices",  "1.3.6.1.2.1.1.7.0");
    m
}

// ==============================================================================
// INTERFACE OIDS — IF-MIB (RFC 2863)
// ==============================================================================
pub fn interface_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    // Tabla de interfaces (ifTable)
    m.insert("ifNumber",        "1.3.6.1.2.1.2.1.0");
    m.insert("ifTable",         "1.3.6.1.2.1.2.2");
    m.insert("ifIndex",         "1.3.6.1.2.1.2.2.1.1");
    m.insert("ifDescr",         "1.3.6.1.2.1.2.2.1.2");
    m.insert("ifType",          "1.3.6.1.2.1.2.2.1.3");
    m.insert("ifMtu",           "1.3.6.1.2.1.2.2.1.4");
    m.insert("ifSpeed",         "1.3.6.1.2.1.2.2.1.5");
    m.insert("ifPhysAddress",   "1.3.6.1.2.1.2.2.1.6");
    m.insert("ifAdminStatus",   "1.3.6.1.2.1.2.2.1.7");
    m.insert("ifOperStatus",    "1.3.6.1.2.1.2.2.1.8");
    m.insert("ifLastChange",    "1.3.6.1.2.1.2.2.1.9");
    m.insert("ifInOctets",      "1.3.6.1.2.1.2.2.1.10");
    m.insert("ifInUcastPkts",   "1.3.6.1.2.1.2.2.1.11");
    m.insert("ifInErrors",      "1.3.6.1.2.1.2.2.1.14");
    m.insert("ifInDiscards",    "1.3.6.1.2.1.2.2.1.13");
    m.insert("ifOutOctets",     "1.3.6.1.2.1.2.2.1.16");
    m.insert("ifOutUcastPkts",  "1.3.6.1.2.1.2.2.1.17");
    m.insert("ifOutErrors",     "1.3.6.1.2.1.2.2.1.20");
    m.insert("ifOutDiscards",   "1.3.6.1.2.1.2.2.1.19");
    m
}

/// High-Capacity (64-bit) interface counters — IF-MIB (RFC 2863)
pub fn hc_interface_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("ifName",           "1.3.6.1.2.1.31.1.1.1.1");
    m.insert("ifHCInOctets",     "1.3.6.1.2.1.31.1.1.1.6");
    m.insert("ifHCInUcastPkts",  "1.3.6.1.2.1.31.1.1.1.7");
    m.insert("ifHCOutOctets",    "1.3.6.1.2.1.31.1.1.1.10");
    m.insert("ifHCOutUcastPkts", "1.3.6.1.2.1.31.1.1.1.11");
    m.insert("ifHighSpeed",      "1.3.6.1.2.1.31.1.1.1.15");
    m.insert("ifAlias",          "1.3.6.1.2.1.31.1.1.1.18");
    m
}

// ==============================================================================
// TCP/UDP STATS (RFC 4022 / 4113)
// ==============================================================================
pub fn tcp_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("tcpActiveOpens",   "1.3.6.1.2.1.6.5.0");
    m.insert("tcpPassiveOpens",  "1.3.6.1.2.1.6.6.0");
    m.insert("tcpAttemptFails",  "1.3.6.1.2.1.6.7.0");
    m.insert("tcpEstabResets",   "1.3.6.1.2.1.6.8.0");
    m.insert("tcpCurrEstab",     "1.3.6.1.2.1.6.9.0");
    m.insert("tcpInSegs",        "1.3.6.1.2.1.6.10.0");
    m.insert("tcpOutSegs",       "1.3.6.1.2.1.6.11.0");
    m.insert("tcpRetransSegs",   "1.3.6.1.2.1.6.12.0");
    m.insert("tcpInErrs",        "1.3.6.1.2.1.6.14.0");
    m.insert("tcpOutRsts",       "1.3.6.1.2.1.6.15.0");
    m
}

pub fn udp_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("udpInDatagrams",  "1.3.6.1.2.1.7.1.0");
    m.insert("udpNoPorts",      "1.3.6.1.2.1.7.2.0");
    m.insert("udpInErrors",     "1.3.6.1.2.1.7.3.0");
    m.insert("udpOutDatagrams", "1.3.6.1.2.1.7.4.0");
    m
}

// ==============================================================================
// IP STATS (RFC 4293)
// ==============================================================================
pub fn ip_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("ipInReceives",       "1.3.6.1.2.1.4.3.0");
    m.insert("ipInHdrErrors",      "1.3.6.1.2.1.4.4.0");
    m.insert("ipInAddrErrors",     "1.3.6.1.2.1.4.5.0");
    m.insert("ipForwDatagrams",    "1.3.6.1.2.1.4.6.0");
    m.insert("ipInUnknownProtos",  "1.3.6.1.2.1.4.7.0");
    m.insert("ipInDiscards",       "1.3.6.1.2.1.4.8.0");
    m.insert("ipInDelivers",       "1.3.6.1.2.1.4.9.0");
    m.insert("ipOutRequests",      "1.3.6.1.2.1.4.10.0");
    m.insert("ipOutDiscards",      "1.3.6.1.2.1.4.11.0");
    m.insert("ipOutNoRoutes",      "1.3.6.1.2.1.4.12.0");
    m.insert("ipReasmReqds",       "1.3.6.1.2.1.4.14.0");
    m.insert("ipReasmFails",       "1.3.6.1.2.1.4.16.0");
    m.insert("ipFragOKs",          "1.3.6.1.2.1.4.17.0");
    m.insert("ipFragFails",        "1.3.6.1.2.1.4.18.0");
    m.insert("ipFragCreates",      "1.3.6.1.2.1.4.19.0");
    m
}

// ==============================================================================
// ICMP STATS (RFC 2011)
// ==============================================================================
pub fn icmp_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("icmpInMsgs",       "1.3.6.1.2.1.5.1.0");
    m.insert("icmpInErrors",     "1.3.6.1.2.1.5.2.0");
    m.insert("icmpInEchos",      "1.3.6.1.2.1.5.8.0");
    m.insert("icmpInEchoReps",   "1.3.6.1.2.1.5.9.0");
    m.insert("icmpOutMsgs",      "1.3.6.1.2.1.5.14.0");
    m.insert("icmpOutErrors",    "1.3.6.1.2.1.5.15.0");
    m.insert("icmpOutEchos",     "1.3.6.1.2.1.5.21.0");
    m.insert("icmpOutEchoReps",  "1.3.6.1.2.1.5.22.0");
    m
}

// ==============================================================================
// SNMP STATS (RFC 3418)
// ==============================================================================
pub fn snmp_stats_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("snmpInPkts",                "1.3.6.1.2.1.11.1.0");
    m.insert("snmpOutPkts",               "1.3.6.1.2.1.11.2.0");
    m.insert("snmpInBadVersions",         "1.3.6.1.2.1.11.3.0");
    m.insert("snmpInBadCommunityNames",   "1.3.6.1.2.1.11.4.0");
    m.insert("snmpInBadCommunityUses",    "1.3.6.1.2.1.11.5.0");
    m.insert("snmpInASNParseErrs",        "1.3.6.1.2.1.11.6.0");
    m.insert("snmpInTooBigs",             "1.3.6.1.2.1.11.8.0");
    m.insert("snmpInNoSuchNames",         "1.3.6.1.2.1.11.9.0");
    m.insert("snmpInBadValues",           "1.3.6.1.2.1.11.10.0");
    m.insert("snmpInReadOnlys",           "1.3.6.1.2.1.11.11.0");
    m.insert("snmpInGenErrs",             "1.3.6.1.2.1.11.12.0");
    m.insert("snmpOutTooBigs",            "1.3.6.1.2.1.11.20.0");
    m.insert("snmpOutNoSuchNames",        "1.3.6.1.2.1.11.21.0");
    m.insert("snmpOutBadValues",          "1.3.6.1.2.1.11.22.0");
    m.insert("snmpOutGenErrs",            "1.3.6.1.2.1.11.24.0");
    m
}

// ==============================================================================
// HOST-RESOURCES-MIB (RFC 2790) — hrStorage y hrProcessor
// Usado por: MikroTik, UBNT, Cambium, Linux, Windows, genérico
// ==============================================================================
pub fn hr_storage_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("hrStorageTable",       "1.3.6.1.2.1.25.2.3");
    m.insert("hrStorageIndex",       "1.3.6.1.2.1.25.2.3.1.1");
    m.insert("hrStorageType",        "1.3.6.1.2.1.25.2.3.1.2");
    m.insert("hrStorageDescr",       "1.3.6.1.2.1.25.2.3.1.3");
    m.insert("hrStorageAllocationUnits", "1.3.6.1.2.1.25.2.3.1.4");
    m.insert("hrStorageSize",        "1.3.6.1.2.1.25.2.3.1.5");
    m.insert("hrStorageUsed",        "1.3.6.1.2.1.25.2.3.1.6");
    m
}

pub fn hr_processor_oids() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("hrProcessorTable",  "1.3.6.1.2.1.25.3.3");
    m.insert("hrProcessorLoad",   "1.3.6.1.2.1.25.3.3.1.2");
    m
}

// ==============================================================================
// TIPOS DE STORAGE (hrStorageType OID prefixes)
// ==============================================================================
pub mod storage_types {
    pub const RAM: &str              = "1.3.6.1.2.1.25.2.1.2";
    pub const VIRTUAL_MEMORY: &str  = "1.3.6.1.2.1.25.2.1.3";
    pub const FIXED_DISK: &str      = "1.3.6.1.2.1.25.2.1.4";
    pub const FLASH: &str           = "1.3.6.1.2.1.25.2.1.7";
    pub const NETWORK_DISK: &str    = "1.3.6.1.2.1.25.2.1.10";
}
