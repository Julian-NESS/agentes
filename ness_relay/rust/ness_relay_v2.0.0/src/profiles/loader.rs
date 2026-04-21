// ==============================================================================
// NESS Relay v2.0.0 — Registro de perfiles (ProfileLoader)
// Equivalente Python: profiles/profile_loader.py
// ==============================================================================

use std::collections::HashMap;
use std::sync::Arc;

use super::base::DeviceProfile;
use super::vendors::{
    c_n::CambiumProfile,
    cisco::CiscoProfile,
    fortinet::FortinetProfile,
    generic::GenericProfile,
    mikrotik::MikroTikProfile,
    mikrotik_fw::MikroTikFwProfile,
    pfsense::PfSenseProfile,
    ubnt::UbntProfile,
};

// ==============================================================================
// PROFILE LOADER
// ==============================================================================

/// Registro central de perfiles de dispositivo.
/// Equivale al ProfileLoader de Python con patrón registry.
pub struct ProfileLoader {
    profiles: HashMap<String, Arc<dyn DeviceProfile>>,
}

impl ProfileLoader {
    /// Crea e inicializa el loader con todos los vendors soportados.
    pub fn new() -> Self {
        let mut loader = Self {
            profiles: HashMap::new(),
        };
        loader.register_all();
        loader
    }

    /// Registra todos los perfiles disponibles.
    fn register_all(&mut self) {
        self.register("pfsense",    Arc::new(PfSenseProfile::new()));
        self.register("fortinet",   Arc::new(FortinetProfile::new()));
        self.register("mikrotik",   Arc::new(MikroTikProfile::new()));
        self.register("mikrotik_fw", Arc::new(MikroTikFwProfile::new()));
        self.register("cisco",      Arc::new(CiscoProfile::new()));
        self.register("ubnt",       Arc::new(UbntProfile::new()));
        self.register("c_n",        Arc::new(CambiumProfile::new()));
        self.register("linux",      Arc::new(GenericProfile::new("linux")));
        self.register("windows",    Arc::new(GenericProfile::new("windows")));
        self.register("generic",    Arc::new(GenericProfile::new("generic")));
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
    /// Retorna None si no hay coincidencia.
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
