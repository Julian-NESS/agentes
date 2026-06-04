// ==============================================================================
// NESS Relay v2.0.3 — Registro de perfiles (ProfileLoader)
// ==============================================================================
//
// Perfiles soportados:
//   - Firewalls: pfSense, Fortinet, MikroTik FW, Sophos, Check Point, Palo Alto
//   - Routers:   MikroTik RouterOS, Cisco, Juniper MX/SRX
//   - Switches:  UBNT, Huawei, TP-Link, Dell, Datacom, Aruba, Juniper EX/QFX, Extreme
//   - APs:       Cambium Networks
//   - Impresoras (genérico): detección por OID/sysDescr
//   - Fallback:  Generic (métricas básicas SNMP estándar)
//
// Detección inteligente:
//   1. Primero: sysObjectID (OID enterprise) — detección exacta
//   2. Segundo: sysDescr (texto) — heurística por palabras clave
//   3. Fallback: perfil genérico
// ==============================================================================

use std::collections::HashMap;
use std::sync::Arc;

use super::base::DeviceProfile;
use super::vendors::{
    access_points::c_n::CambiumProfile,
    firewalls::fortinet::FortinetProfile,
    firewalls::mikrotik_fw::MikroTikFwProfile,
    firewalls::pfsense::PfSenseProfile,
    firewalls::sophos::SophosProfile,
    firewalls::checkpoint::CheckPointProfile,
    firewalls::palo_alto::PaloAltoProfile,
    routers::cisco::CiscoProfile,
    routers::mikrotik::MikroTikProfile,
    routers::juniper_mx::JuniperMxProfile,
    shared::generic::GenericProfile,
    switches::datacomm::DatacomProfile,
    switches::dell::DellProfile,
    switches::huawei::HuaweiProfile,
    switches::tp_link::TpLinkProfile,
    switches::ubnt::UbntProfile,
    switches::aruba::ArubaProfile,
    switches::juniper_ex::JuniperExProfile,
    switches::extreme::ExtremeProfile,
};

// ==============================================================================
// PROFILE LOADER
// ==============================================================================

/// Registro central de perfiles de dispositivo.
/// Usa un HashMap para lookup por nombre y un Vec ordenado para auto-detección
/// determinista por sysObjectID.
pub struct ProfileLoader {
    profiles: HashMap<String, Arc<dyn DeviceProfile>>,
    /// Vector ordenado por prioridad para auto-detección por sysObjectID.
    /// Se itera en orden: el primer match gana.
    detection_order: Vec<Arc<dyn DeviceProfile>>,
}

impl ProfileLoader {
    /// Crea e inicializa el loader con todos los vendors soportados.
    pub fn new() -> Self {
        let mut loader = Self {
            profiles: HashMap::new(),
            detection_order: Vec::new(),
        };
        loader.register_all();
        loader
    }

    /// Registra todos los perfiles disponibles.
    /// NOTA: Solo perfiles de infraestructura/IoT y red. No servidores.
    fn register_all(&mut self) {
        // --- Firewalls ---
        let pfsense:     Arc<dyn DeviceProfile> = Arc::new(PfSenseProfile::new());
        let fortinet:    Arc<dyn DeviceProfile> = Arc::new(FortinetProfile::new());
        let mikrotik_fw: Arc<dyn DeviceProfile> = Arc::new(MikroTikFwProfile::new());
        let sophos:      Arc<dyn DeviceProfile> = Arc::new(SophosProfile::new());
        let checkpoint:  Arc<dyn DeviceProfile> = Arc::new(CheckPointProfile::new());
        let palo_alto:   Arc<dyn DeviceProfile> = Arc::new(PaloAltoProfile::new());

        // --- Routers ---
        let mikrotik:    Arc<dyn DeviceProfile> = Arc::new(MikroTikProfile::new());
        let cisco:       Arc<dyn DeviceProfile> = Arc::new(CiscoProfile::new());
        let juniper_mx:  Arc<dyn DeviceProfile> = Arc::new(JuniperMxProfile::new());

        // --- Switches ---
        let ubnt:        Arc<dyn DeviceProfile> = Arc::new(UbntProfile::new());
        let huawei:      Arc<dyn DeviceProfile> = Arc::new(HuaweiProfile);
        let tp_link:     Arc<dyn DeviceProfile> = Arc::new(TpLinkProfile);
        let dell:        Arc<dyn DeviceProfile> = Arc::new(DellProfile);
        let datacomm:    Arc<dyn DeviceProfile> = Arc::new(DatacomProfile);
        let aruba:       Arc<dyn DeviceProfile> = Arc::new(ArubaProfile::new());
        let juniper_ex:  Arc<dyn DeviceProfile> = Arc::new(JuniperExProfile::new());
        let extreme:     Arc<dyn DeviceProfile> = Arc::new(ExtremeProfile::new());

        // --- APs ---
        let cambium:     Arc<dyn DeviceProfile> = Arc::new(CambiumProfile::new());

        // --- Fallback ---
        let generic:     Arc<dyn DeviceProfile> = Arc::new(GenericProfile::new("generic"));
        let generic_firewall: Arc<dyn DeviceProfile> = Arc::new(GenericProfile::new("firewall"));
        let generic_router:   Arc<dyn DeviceProfile> = Arc::new(GenericProfile::new("router"));
        let generic_switch:   Arc<dyn DeviceProfile> = Arc::new(GenericProfile::new("switch"));
        let generic_ap:       Arc<dyn DeviceProfile> = Arc::new(GenericProfile::new("ap"));
        let generic_printer:  Arc<dyn DeviceProfile> = Arc::new(GenericProfile::new("printer"));

        // Registrar en el HashMap para lookup por nombre
        self.register("pfsense",     pfsense.clone());
        self.register("fortinet",    fortinet.clone());
        self.register("mikrotik_fw", mikrotik_fw.clone());
        self.register("sophos",      sophos.clone());
        self.register("checkpoint",  checkpoint.clone());
        self.register("palo_alto",   palo_alto.clone());
        self.register("mikrotik",    mikrotik.clone());
        self.register("cisco",       cisco.clone());
        self.register("juniper_mx",  juniper_mx.clone());
        self.register("ubnt",        ubnt.clone());
        self.register("huawei",      huawei.clone());
        self.register("tp_link",     tp_link.clone());
        self.register("dell",        dell.clone());
        self.register("datacomm",    datacomm.clone());
        self.register("aruba",       aruba.clone());
        self.register("juniper_ex",  juniper_ex.clone());
        self.register("extreme",     extreme.clone());
        self.register("c_n",         cambium.clone());
        self.register("generic",     generic.clone());
        self.register("firewall",    generic_firewall.clone());
        self.register("router",      generic_router.clone());
        self.register("switch",      generic_switch.clone());
        self.register("ap",          generic_ap.clone());
        self.register("printer",     generic_printer.clone());

        // Vector de detección por sysObjectID, en orden de prioridad.
        // IMPORTANTE: MikroTik RouterOS va primero que MikroTik FW porque
        // ambos comparten el OID enterprise 1.3.6.1.4.1.14988.
        // `auto_detect` retorna "mikrotik" por defecto y `resolve_profile`
        // decide si promover a "mikrotik_fw" según contexto del fallback.
        //
        // Juniper EX/QFX va antes que Juniper MX/SRX porque EX/QFX usa
        // subárboles más específicos (1.51.*, 1.62.*) que el MX catchall.
        self.detection_order = vec![
            pfsense,
            fortinet,
            sophos,        // OID 2604 — Sophos
            checkpoint,    // OID 2620 — Check Point
            palo_alto,     // OID 25461 — Palo Alto
            mikrotik,      // OID 14988 — router por defecto
            // mikrotik_fw se omite del auto-detect: mismo OID que mikrotik
            cisco,
            juniper_ex,    // OID 2636.1.51/62 — Juniper EX/QFX (antes que MX)
            juniper_mx,    // OID 2636 (catchall) — Juniper MX/SRX
            ubnt,
            aruba,         // OID 11.2.3.7 / 14823 — Aruba/HPE
            extreme,       // OID 1916 — Extreme Networks
            cambium,
            // Switches sin OID enterprise registrado:
            // huawei, tp_link, dell, datacomm — se detectan por sysDescr
        ];
    }

    /// Registra un perfil bajo una clave de vendor.
    pub fn register(&mut self, vendor: &str, profile: Arc<dyn DeviceProfile>) {
        self.profiles.insert(vendor.to_lowercase(), profile);
    }

    /// Obtiene el perfil para un vendor específico.
    /// Si el vendor no se encuentra, retorna el perfil genérico.
    pub fn get_profile(&self, vendor: &str) -> Arc<dyn DeviceProfile> {
        let key = vendor.to_lowercase();
        self.profiles
            .get(&key)
            .cloned()
            .unwrap_or_else(|| {
                self.profiles
                    .get("generic")
                    .cloned()
                    .expect("Perfil genérico siempre debe estar registrado")
            })
    }

    /// Lista todos los vendors registrados.
    pub fn list_vendors(&self) -> Vec<&String> {
        let mut vendors: Vec<&String> = self.profiles.keys().collect();
        vendors.sort();
        vendors
    }

    /// Intenta auto-detectar el perfil por sysObjectID.
    /// Usa el vector ordenado `detection_order` para garantizar determinismo.
    /// Retorna None si no hay coincidencia.
    pub fn auto_detect(&self, sys_object_id: &str) -> Option<Arc<dyn DeviceProfile>> {
        if sys_object_id.is_empty() {
            return None;
        }
        for profile in &self.detection_order {
            if profile.matches_sys_object_id(sys_object_id) {
                return Some(profile.clone());
            }
        }
        None
    }

    /// Inferencia genérica por sysObjectID para familias no vendor-específicas.
    /// Se usa como fallback cuando no hay match exacto en `auto_detect`.
    fn infer_generic_vendor_from_oid(sys_object_id: &str) -> Option<&'static str> {
        let normalized = sys_object_id.trim();
        if normalized.is_empty() {
            return None;
        }

        // Enterprise OIDs conocidos de impresoras. Evitamos prefijos ambiguos
        // (como HP genérico) para reducir falsos positivos en switches/APs.
        const PRINTER_OID_PREFIXES: [&str; 5] = [
            "1.3.6.1.4.1.367.",  // Kyocera
            "1.3.6.1.4.1.1347.", // Lexmark
            "1.3.6.1.4.1.1602.", // Canon
            "1.3.6.1.4.1.2435.", // Brother
            "1.3.6.1.4.1.1248.", // Epson
        ];

        if PRINTER_OID_PREFIXES
            .iter()
            .any(|prefix| normalized.starts_with(prefix))
        {
            return Some("printer");
        }

        None
    }

    /// Inferencia genérica por sysDescr cuando no se puede identificar vendor.
    fn infer_generic_vendor_from_sys_descr(normalized_descr: &str) -> Option<&'static str> {
        if normalized_descr.contains("printer")
            || normalized_descr.contains("laserjet")
            || normalized_descr.contains("deskjet")
            || normalized_descr.contains("epson")
            || normalized_descr.contains("brother")
            || normalized_descr.contains("lexmark")
            || normalized_descr.contains("multifunction")
        {
            return Some("printer");
        }

        if normalized_descr.contains("firewall")
            || normalized_descr.contains("next-generation firewall")
            || normalized_descr.contains("utm")
        {
            return Some("firewall");
        }

        if normalized_descr.contains("access point")
            || normalized_descr.contains("wireless ap")
            || normalized_descr.contains("wifi ap")
        {
            return Some("ap");
        }

        if normalized_descr.contains("switch")
            || normalized_descr.contains("l2 switch")
            || normalized_descr.contains("l3 switch")
        {
            return Some("switch");
        }

        if normalized_descr.contains("router")
            || normalized_descr.contains("gateway")
            || normalized_descr.contains("edge device")
        {
            return Some("router");
        }

        None
    }

    fn is_generic_fallback_vendor(vendor: &str) -> bool {
        matches!(
            vendor,
            "" | "generic" | "linux" | "windows" | "router" | "switch" | "firewall" | "ap" | "access_point" | "other"
        )
    }

    /// Resuelve el mejor perfil disponible usando sysObjectID y como respaldo sysDescr.
    ///
    /// Cadena de detección:
    ///   1. sysObjectID → match exacto por OID enterprise
    ///   2. sysDescr → heurística por palabras clave del vendor
    ///   3. fallback_vendor → lo que venga del connection.config
    ///   4. generic → perfil genérico SNMP estándar
    ///
    /// NOTA: No se buscan "linux", "windows" ni "freebsd" en sysDescr.
    /// Muchos dispositivos de red (MikroTik, pfSense, UBNT) reportan "Linux"
    /// en su kernel, lo que causaba falsos positivos.
    pub fn resolve_profile(
        &self,
        fallback_vendor: &str,
        sys_object_id: &str,
        sys_descr: &str,
    ) -> Arc<dyn DeviceProfile> {
        let fallback_vendor_normalized = fallback_vendor.trim().to_lowercase();
        let fallback_is_generic = Self::is_generic_fallback_vendor(&fallback_vendor_normalized);

        // Si el vendor viene explícito desde connection.config, respetarlo para
        // evitar sobrescribir configuraciones intencionales (ej. mikrotik_fw).
        if !fallback_vendor_normalized.is_empty()
            && !fallback_is_generic
            && self.profiles.contains_key(&fallback_vendor_normalized)
        {
            return self.get_profile(&fallback_vendor_normalized);
        }

        // --- Paso 1: Detección por sysObjectID ---
        if let Some(profile) = self.auto_detect(sys_object_id) {
            // Cuando el fallback es genérico (generic/linux/windows/other), un
            // MikroTik detectado por OID se promueve al perfil firewall para
            // conservar métricas avanzadas (internet_channels, queues, health).
            if profile.vendor() == "mikrotik" && fallback_is_generic {
                return self.get_profile("mikrotik_fw");
            }
            return profile;
        }

        // --- Paso 1.5: Inferencia genérica por sysObjectID ---
        if let Some(generic_vendor) = Self::infer_generic_vendor_from_oid(sys_object_id) {
            return self.get_profile(generic_vendor);
        }

        // --- Paso 2: Heurística por sysDescr ---
        // Solo buscamos vendors de dispositivos de red conocidos.
        // NUNCA buscamos "linux", "windows" o "freebsd" para evitar
        // falsos positivos con dispositivos cuyo kernel es Linux.
        let normalized_descr = sys_descr.to_lowercase();
        let inferred_vendor = if normalized_descr.contains("pfsense") {
            Some("pfsense")
        } else if normalized_descr.contains("fortinet") || normalized_descr.contains("fortigate") {
            Some("fortinet")
        } else if normalized_descr.contains("sophos") || normalized_descr.contains("sfos") || normalized_descr.contains("cyberoam") {
            Some("sophos")
        } else if normalized_descr.contains("check point") || normalized_descr.contains("checkpoint") || normalized_descr.contains("gaia") {
            Some("checkpoint")
        } else if normalized_descr.contains("palo alto") || normalized_descr.contains("pan-os") || normalized_descr.contains("panos") {
            Some("palo_alto")
        } else if normalized_descr.contains("mikrotik") || normalized_descr.contains("routeros") {
            Some("mikrotik")
        } else if normalized_descr.contains("ubiquiti") || normalized_descr.contains("ubnt") || normalized_descr.contains("edgeswitch") {
            Some("ubnt")
        } else if normalized_descr.contains("cisco") || normalized_descr.contains("ios") {
            Some("cisco")
        } else if normalized_descr.contains("juniper") || normalized_descr.contains("junos") {
            // Para sysDescr genérico "Juniper" asignamos MX/SRX (router);
            // la detección por sysObjectID ya diferencia EX/QFX.
            Some("juniper_mx")
        } else if normalized_descr.contains("aruba") || normalized_descr.contains("procurve") || normalized_descr.contains("hpe switch") {
            Some("aruba")
        } else if normalized_descr.contains("extreme") || normalized_descr.contains("extremexos") || normalized_descr.contains("exos") {
            Some("extreme")
        } else if normalized_descr.contains("huawei") || normalized_descr.contains("vrp") {
            Some("huawei")
        } else if normalized_descr.contains("tp-link") || normalized_descr.contains("tplink") {
            Some("tp_link")
        } else if normalized_descr.contains("dell") || normalized_descr.contains("force10") {
            Some("dell")
        } else if normalized_descr.contains("datacom") {
            Some("datacomm")
        } else if normalized_descr.contains("cambium") || normalized_descr.contains("cnpilot") {
            Some("c_n")
        } else {
            None
        };

        if let Some(vendor) = inferred_vendor {
            if vendor == "mikrotik" && fallback_is_generic {
                return self.get_profile("mikrotik_fw");
            }
            return self.get_profile(vendor);
        }

        if let Some(generic_vendor) = Self::infer_generic_vendor_from_sys_descr(&normalized_descr) {
            return self.get_profile(generic_vendor);
        }

        // --- Paso 3: Fallback al vendor del connection.config ---
        // Si el config dice "generic", caerá al perfil genérico.
        self.get_profile(&fallback_vendor_normalized)
    }
}

impl Default for ProfileLoader {
    fn default() -> Self {
        Self::new()
    }
}
