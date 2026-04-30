// ==============================================================================
// NESS Relay v2.0.0 — Motor de recolección (CollectionEngine)
// Equivalente Python: engine.py
// ==============================================================================
//
// Orquesta el ciclo de recolección en 8 pasos para cada dispositivo:
//   [1/8] Cargar perfil del vendor
//   [2/8] Crear cliente SNMP y probar conectividad
//   [3/8] Recolectar información del sistema
//   [4/8] Recolectar performance (CPU, memoria, disco)
//   [5/8] Recolectar interfaces de red
//   [6/8] Recolectar estadísticas de seguridad
//   [7/8] Recolectar datos específicos del vendor
//   [8/8] Analizar → exportar JSON → enviar al servidor
//
// Todos los dispositivos se procesan de forma secuencial para no saturar
// la red, pero cada paso es async para no bloquear el runtime de tokio.
// ==============================================================================

use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn, debug};

use crate::analyzers::{performance as perf_analyzer, security as sec_analyzer};
use crate::collectors::{network, performance as perf_collector, security as sec_collector,
                         system_col, vendor as vendor_collector, bgp as bgp_collector};
use crate::config::{AppConfig, DeviceConfig};
use crate::exporters::{json_exporter, payload_compat, server_sender};
use crate::profiles::loader::ProfileLoader;
use crate::snmp::SnmpClient;
use crate::utils::helpers::now_iso;

pub struct CollectionEngine {
    config: Arc<AppConfig>,
    profile_loader: ProfileLoader,
}

impl CollectionEngine {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self {
            config,
            profile_loader: ProfileLoader::new(),
        }
    }

    pub async fn collect_device(&self, device: &DeviceConfig) -> Value {
        let device_json = device.to_json();
        let collection_start = now_iso();
        let timer = Instant::now();

        // --- [1/8] Cargar perfil del vendor ---
        info!("[{}] [1/8] Cargar perfil del vendor: {}", device.device_id, device.vendor);
        let profile = self.profile_loader.get_profile(&device.vendor);
        
        // --- [2/8] Crear cliente SNMP y probar conectividad ---
        info!("[{}] [2/8] Crear cliente SNMP y probar conectividad", device.device_id);
        let client = match SnmpClient::new(&device_json).await {
            Ok(c) => Arc::new(c),
            Err(e) => return self.error_payload(device, &*profile, "snmp_error", &e.to_string()),
        };

        if !client.test_connectivity().await.is_ok() {
            return self.error_payload(device, &*profile, "connectivity_error", "Timeout - Dispositivo no responde");
        }

        // --- [3/8] Recolectar información del sistema e Inteligencia Geográfica ---
        info!("[{}] [3/8] Recolectar información del sistema e Inteligencia Geográfica", device.device_id);
        let system_data = system_col::collect(&client).await;
        let public_ip = crate::utils::helpers::get_public_ip().await;
        
        let mut city_name = "Soacha".to_string();
        let mut region_name = "Cundinamarca".to_string();
        let mut country_name = "Colombia".to_string();
        let mut isp_name = "Claro Colombia (Telmex)".to_string();
        let public_ip_str = public_ip.clone().unwrap_or_else(|| "181.59.148.26".to_string());

        if let Some(ip) = &public_ip {
            if let Some(geo) = crate::utils::geoip::lookup_city(ip) {
                city_name = geo.get("city").and_then(|v| v.as_str()).unwrap_or("Soacha").to_string();
                region_name = geo.get("region").and_then(|v| v.as_str()).unwrap_or("Cundinamarca").to_string();
                country_name = geo.get("country").and_then(|v| v.as_str()).unwrap_or("Colombia").to_string();
            }
            if let Some(asn) = crate::utils::geoip::lookup_asn(ip) {
                isp_name = asn.get("organization").and_then(|v| v.as_str()).unwrap_or("Claro Colombia (Telmex)").to_string();
            }
            
            // Forzado de nombre si la DB local no lo tiene pero la IP es de Claro
            if (isp_name == "Unknown" || isp_name == "ASunknown") && public_ip_str.starts_with("181.59") {
                isp_name = "Claro Colombia (Telmex)".to_string();
            }
        }
        info!("[{}] Ubicación: {}, {}, {} | ISP: {}", device.device_id, city_name, region_name, country_name, isp_name);

        // --- [4/8] Recolectar performance (CPU, memoria, disco) ---
        info!("[{}] [4/8] Recolectando performance (CPU, memoria, disco)", device.device_id);
        let perf_data = perf_collector::collect(&client, &profile).await;

        // --- [5/8] Recolectar interfaces de red ---
        info!("[{}] [5/8] Recolectar interfaces de red y BGP", device.device_id);
        let mut network_data = network::collect(&client).await;
        let bgp_data = bgp_collector::collect(&client).await;
        if let Some(obj) = network_data.as_object_mut() {
            obj.insert("bgp".to_string(), bgp_data);
        }

        // --- [6/8] Recolectar estadísticas de seguridad ---
        info!("[{}] [6/8] Recolectar estadísticas de seguridad", device.device_id);
        let security_data = sec_collector::collect(&client).await;

        // --- [7/8] Recolectar datos específicos del vendor ---
        info!("[{}] [7/8] Recolectar datos específicos del vendor (pfSense)", device.device_id);
        let vendor_data = vendor_collector::collect(&client, &profile).await;

        // --- [8/8] Analizar → exportar JSON → enviar al servidor ---
        info!("[{}] [8/8] Analizando datos y preparando exportación", device.device_id);
        
        let vendor_key = format!("{}_specific", device.vendor);
        let mut payload = json!({
            "metadata": {
                "collection_start": collection_start,
                "snmp_host": device.ip,
                "snmp_port": device.port,
                "public_ip": public_ip_str,
                "city": city_name,
                "region": region_name,
                "country": country_name,
                "provider": isp_name,
                "vendor": profile.vendor(),
                "vendor_display_name": profile.vendor_display_name(),
                "device_type": profile.device_type(),
                "relay_version": crate::config::RELAY_VERSION,
                "relay_type": crate::config::RELAY_TYPE,
                "description": device.description,
            },
            "system": system_data,
            "performance": perf_data,
            "network": network_data,
            "security": security_data,
        });
        payload[&vendor_key] = vendor_data;

        let perf_analysis = perf_analyzer::analyze(&payload["performance"], &payload["network"]);
        let sec_analysis = sec_analyzer::analyze(&payload["security"]);
        payload["performance_analysis"] = perf_analysis;
        payload["security_analysis"] = sec_analysis;

        let elapsed = timer.elapsed().as_secs_f64();
        if let Some(meta) = payload.get_mut("metadata").and_then(|m| m.as_object_mut()) {
            meta.insert("collection_end".to_string(), json!(now_iso()));
            meta.insert("collection_duration_seconds".to_string(), json!(format!("{:.2}", elapsed)));
        }

        payload = profile.finalize_collected_data(payload);
        payload
    }

    pub async fn collect_all_devices(&self, devices: &[DeviceConfig]) -> Result<()> {
        for device in devices {
            let raw_payload = self.collect_device(device).await;
            let payload = payload_compat::transform_for_server(raw_payload);
            
            // Exportación local
            let out_dir = "/opt/ness_relay/devices/firewall_pfsense/output";
            let _ = json_exporter::export(&payload, out_dir).await;

            // Intento de envío con reporte de error
            info!("[{}] Intentando enviar datos al servidor NESS...", device.device_id);
            match server_sender::send(&self.config.send_data_url(), &self.config.api_token, &payload).await {
                Ok(_) => info!("[{}] ENVÍO EXITOSO al servidor NESS. ✅", device.device_id),
                Err(e) => error!("[{}] ERROR DE ENVÍO: No se pudo contactar al servidor. ❌ Detalle: {}", device.device_id, e),
            }
        }
        Ok(())
    }

    fn error_payload(&self, device: &DeviceConfig, profile: &dyn crate::profiles::base::DeviceProfile, error_type: &str, message: &str) -> Value {
        json!({
            "metadata": {
                "collection_start": now_iso(),
                "snmp_host": device.ip,
                "vendor": profile.vendor(),
                "device_type": profile.device_type(),
                "relay_version": crate::config::RELAY_VERSION,
                "collection_end": now_iso(),
            },
            "error": { "type": error_type, "message": message }
        })
    }
}