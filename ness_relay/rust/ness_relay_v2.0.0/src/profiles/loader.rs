// ==============================================================================
// NESS Relay v2.0.0 — Registro de perfiles (ProfileLoader)
// ==============================================================================

use std::collections::HashMap;
use std::sync::Arc;

use super::base::DeviceProfile;
// Importamos desde vendors usando la nueva estructura de carpetas
use super::vendors::{
    switches::{cisco::CiscoProfile, huawei::HuaweiProfile, dell::DellProfile, datacomm::DatacomProfile, tp_link::TpLinkProfile},
    firewalls::{fortinet::FortinetProfile, pfsense::PfSenseProfile, sophos::SophosProfile},
    // wireless::{ubnt::UbntProfile, mikrotik::MikroTikProfile}, // Comentado si wireless mod está inactivo
};

// Importación del perfil genérico
use super::vendors::generic::GenericProfile; 

pub struct ProfileLoader {
    profiles: HashMap<String, Arc<dyn DeviceProfile>>,
}

impl ProfileLoader {
    pub fn new() -> Self {
        let mut loader = Self {
            profiles: HashMap::new(),
        };
        loader.register_all();
        loader
    }

    fn register_all(&mut self) {
        // --- Firewalls ---
        self.register("pfsense",    Arc::new(PfSenseProfile::new()));
        self.register("fortinet",   Arc::new(FortinetProfile::new()));
        self.register("sophos",     Arc::new(SophosProfile)); 

        // --- Switches ---
        self.register("cisco",      Arc::new(CiscoProfile::new()));
        self.register("huawei",     Arc::new(HuaweiProfile));
        self.register("dell",       Arc::new(DellProfile));
        self.register("datacomm",   Arc::new(DatacomProfile)); 
        self.register("tp_link",    Arc::new(TpLinkProfile));

        // --- Wireless (Solo activar si tienes los archivos y el mod.rs listo) ---
        // self.register("mikrotik",   Arc::new(MikroTikProfile::new()));
        // self.register("ubnt",       Arc::new(UbntProfile::new()));

        // --- Genéricos / Otros ---
        self.register("generic",    Arc::new(GenericProfile::new("generic")));
        self.register("linux",      Arc::new(GenericProfile::new("linux")));
        self.register("windows",    Arc::new(GenericProfile::new("windows")));
    }

    pub fn register(&mut self, vendor: &str, profile: Arc<dyn DeviceProfile>) {
        self.profiles.insert(vendor.to_lowercase(), profile);
    }

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

    pub fn list_vendors(&self) -> Vec<&String> {
        let mut vendors: Vec<&String> = self.profiles.keys().collect();
        vendors.sort();
        vendors
    }

    pub fn auto_detect(&self, sys_object_id: &str) -> Option<Arc<dyn DeviceProfile>> {
        for profile in self.profiles.values() {
            if profile.matches_sys_object_id(sys_object_id) {
                return Some(profile.clone());
            }
        }
        None
    }
}

impl Default for ProfileLoader {
    fn default() -> Self {
        Self::new()
    }
}