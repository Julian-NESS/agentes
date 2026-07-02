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

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};

use crate::analyzers::{performance as perf_analyzer, security as sec_analyzer};
use crate::collectors::{network, performance as perf_collector, security as sec_collector,
                         system_col, vendor as vendor_collector};
use super::config::{AppConfig, DeviceConfig};
use crate::exporters::{json_exporter, payload_compat, server_sender};
use crate::profiles::loader::ProfileLoader;
use crate::snmp::SnmpClient;
use crate::utils::helpers::now_iso;

const SYS_DESCR_OID: &str = "1.3.6.1.2.1.1.1.0";
const SYS_OBJECT_ID_OID: &str = "1.3.6.1.2.1.1.2.0";

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

    // -------------------------------------------------------------------------
    // Recolección de un solo dispositivo
    //
    // Retorna Result<Value>:
    //   - Ok(payload): las 8 fases se completaron con éxito. El payload contiene
    //     TODA la información recolectada y está listo para enviarse al servidor.
    //   - Err(_):    la recolección fue incompleta (fallo en fase 1, 2 o 3).
    //                NO se debe enviar al servidor para evitar falsos positivos
    //                con payloads que solo contienen metadata parcial.
    // -------------------------------------------------------------------------
    pub async fn collect_device(
        &self,
        device: &DeviceConfig,
        audit_mode: bool,
    ) -> Result<Value> {
        let device_json = device.to_json();
        let collection_start = now_iso();
        let timer = Instant::now();
        info!(
            "[{}] Iniciando recolección — vendor={} ip={} audit_mode={}",
            device.device_id, device.vendor, device.ip, audit_mode
        );

        // [1/8] Cargar perfil
        let profile = self.profile_loader.get_profile(&device.vendor);
        info!(
            "[{}] [1/8] Perfil cargado: {}",
            device.device_id,
            profile.vendor_display_name()
        );

        // [2/8] Crear cliente SNMP + probar conectividad
        let client = match SnmpClient::new(&device_json).await {
            Ok(c) => Arc::new(c),
            Err(e) => {
                error!("[{}] [2/8] No se pudo crear cliente SNMP: {}", device.device_id, e);
                if audit_mode {
                    warn!(
                        "[{}] SNMP client falló pero audit_mode=true — se construye payload SNMP-vacío y se continúa con audit (phases 9+10)",
                        device.device_id,
                    );
                    return self.audit_only_payload(device, audit_mode, timer, collection_start).await;
                }
                warn!(
                    "[{}] Recolección incompleta en fase 2/8 — NO se enviará al servidor",
                    device.device_id
                );
                return Err(anyhow!("snmp_client_error: {}", e));
            }
        };

        info!("[{}] [2/8] Probando conectividad SNMP…", device.device_id);
        let conn_result = client.test_connectivity().await;
        if !conn_result.is_ok() {
            let err_msg = conn_result.error.as_deref().unwrap_or("timeout / no response");
            warn!("[{}] [2/8] Fallo de conectividad: {}", device.device_id, err_msg);
            if audit_mode {
                warn!(
                    "[{}] SNMP caído pero audit_mode=true — se continúa con audit (phases 9+10 sobre SSH)",
                    device.device_id,
                );
                return self.audit_only_payload(device, audit_mode, timer, collection_start).await;
            }
            warn!(
                "[{}] Recolección incompleta en fase 2/8 — NO se enviará al servidor",
                device.device_id
            );
            return Err(anyhow!("connectivity_error: {}", err_msg));
        }
        info!("[{}] [2/8] Conectividad OK", device.device_id);

        let sys_descr_result = client.get(SYS_DESCR_OID).await;
        let sys_descr = sys_descr_result
            .value
            .as_ref()
            .map(|v| v.as_string())
            .unwrap_or_default();
        let sys_object_id_result = client.get(SYS_OBJECT_ID_OID).await;
        let sys_object_id = sys_object_id_result
            .value
            .as_ref()
            .map(|v| v.as_string())
            .unwrap_or_default();

        let profile = self.profile_loader.resolve_profile(
            &device.vendor,
            &sys_object_id,
            &sys_descr,
        );
        info!(
            "[{}] [2/8] Perfil resuelto: {} ({})",
            device.device_id,
            profile.vendor_display_name(),
            profile.vendor()
        );

        // [3/8] Información del sistema
        info!("[{}] [3/8] Recolectando sistema…", device.device_id);
        let system_data = system_col::collect(&client).await;

        // [4/8] Performance (CPU, memoria, disco)
        info!("[{}] [4/8] Recolectando performance…", device.device_id);
        let perf_data = perf_collector::collect(&client, &profile, &sys_object_id).await;

        // [5/8] Interfaces de red
        info!("[{}] [5/8] Recolectando interfaces…", device.device_id);
        let network_data = network::collect(&client).await;

        // [6/8] Estadísticas de seguridad
        info!("[{}] [6/8] Recolectando seguridad…", device.device_id);
        let security_data = sec_collector::collect(&client).await;

        // [7/8] Datos específicos del vendor
        info!("[{}] [7/8] Recolectando datos del vendor…", device.device_id);
        let vendor_data = vendor_collector::collect(&client, &profile, &sys_object_id).await;

        // Contar interfaces de red
        let total_interfaces = network_data.get("interfaces")
            .and_then(|v| v.as_array())
            .map_or(0, |a| a.len());

        // Construir payload con formato idéntico al Python
        // El servidor espera: metadata, system, performance, network, security
        let vendor_key = format!("{}_specific", profile.vendor());
        let mut payload = json!({
            "metadata": {
                "collection_start": collection_start,
                "snmp_host": device.ip,
                "snmp_port": device.port,
                "vendor": profile.vendor(),
                "vendor_display_name": profile.vendor_display_name(),
                "device_type": profile.device_type(),
                "relay_version": super::config::RELAY_VERSION,
                "relay_type":    super::config::RELAY_TYPE,
                "description":   device.description,
            },
            "system":      system_data,
            "performance": perf_data,
            "network":     network_data,
            "security":    security_data,
        });
        payload[&vendor_key] = vendor_data;

        // [8/8] Análisis de alertas
        info!("[{}] [8/8] Analizando alertas…", device.device_id);
        let perf_analysis = perf_analyzer::analyze(&payload["performance"], &payload["network"]);
        let sec_analysis = sec_analyzer::analyze(&payload["security"]);

        payload["performance_analysis"] = perf_analysis;
        payload["security_analysis"] = sec_analysis;

        // Finalizar metadata con tiempos de recolección
        let elapsed = timer.elapsed().as_secs_f64();
        if let Some(meta) = payload.get_mut("metadata").and_then(|m| m.as_object_mut()) {
            meta.insert("collection_end".to_string(), Value::String(now_iso()));
            meta.insert("collection_duration_seconds".to_string(),
                json!(format!("{:.2}", elapsed).parse::<f64>().unwrap_or(elapsed)));
            meta.insert("total_interfaces".to_string(), json!(total_interfaces));
        }

        payload = profile.finalize_collected_data(payload);

        let total_alerts = payload.get("security_analysis")
            .and_then(|v| v.get("total_alerts")).and_then(|v| v.as_u64()).unwrap_or(0)
            + payload.get("performance_analysis")
            .and_then(|v| v.get("total_alerts")).and_then(|v| v.as_u64()).unwrap_or(0);
        let total_warnings = payload.get("security_analysis")
            .and_then(|v| v.get("total_warnings")).and_then(|v| v.as_u64()).unwrap_or(0)
            + payload.get("performance_analysis")
            .and_then(|v| v.get("total_warnings")).and_then(|v| v.as_u64()).unwrap_or(0);

        info!(
            "[{}] Recolección completada — {} alertas, {} advertencias en {:.1}s",
            device.device_id, total_alerts, total_warnings, elapsed
        );

        // =========================================================================
        // Phase 9 + 10 — SSH-based audit (opt-in, best-effort)
        // =========================================================================
        // Only runs when:
        //   1. The caller requested audit mode (`--audit` flag from main).
        //   2. `NESS_AUDIT_ENABLED=true` is set in the environment (opt-in gate,
        //      checked once at process startup in main.rs).
        //   3. The device has valid SSH credentials configured in
        //      connection.config.
        //
        // On any failure (SSH connect timeout, plugin error, etc.) we log a
        // warning and continue with the regular SNMP payload. The contract is
        // best-effort: audit failures NEVER break the regular collection cycle.
        if audit_mode {
            use crate::ness_relay_core::vendor::PluginRegistry;
            use crate::core::audit_runner;

            let registry = std::sync::Arc::new(PluginRegistry::with_defaults());
            match audit_runner::run_audit_phases(device, registry).await {
                Ok(Some(audit_json)) => {
                    if let Some(obj) = audit_json.as_object() {
                        for (k, v) in obj {
                            payload[k] = v.clone();
                        }
                    }
                    info!(
                        target: "ness_relay::engine",
                        "[{}] Phase 9+10 (audit) anexadas al payload",
                        device.device_id,
                    );
                }
                Ok(None) => {
                    info!(
                        target: "ness_relay::engine",
                        "[{}] Phase 9+10 omitidas (sin credenciales o vendor no soportado)",
                        device.device_id,
                    );
                }
                Err(e) => {
                    warn!(
                        target: "ness_relay::engine",
                        "[{}] Phase 9+10 fallaron — {e:#} (SNMP payload ya está armado)",
                        device.device_id,
                    );
                }
            }
        }

        Ok(payload)
    }

    // -------------------------------------------------------------------------
    // Payload mínimo para `--audit` cuando SNMP no responde (Phase 2.4).
    //
    // Construye un payload con solo metadata y deja que las fases 9+10 lo
    // completen. Se usa cuando:
    //   - `--audit` está activo
    //   - El cliente SNMP no se pudo crear o no responde
    //
    // El objetivo es: si el operador está pagando el costo de correr audit
    // cada 6h, queremos ejecutarlo aunque SNMP esté caído (es más probable
    // que SSH funcione que SNMP, o viceversa — pero al menos probamos).
    // -------------------------------------------------------------------------
    async fn audit_only_payload(
        &self,
        device: &DeviceConfig,
        audit_mode: bool,
        timer: Instant,
        collection_start: String,
    ) -> Result<Value> {
        let elapsed = timer.elapsed().as_secs_f64();
        let mut payload = json!({
            "metadata": {
                "collection_start": collection_start,
                "collection_end": now_iso(),
                "collection_duration_seconds": json!(elapsed),
                "snmp_host": device.ip,
                "snmp_port": device.port,
                "vendor": device.vendor,
                "vendor_display_name": device.vendor,
                "device_type": "firewall",
                "relay_version": super::config::RELAY_VERSION,
                "relay_type": super::config::RELAY_TYPE,
                "description": device.description,
                "audit_only": true,
            },
        });

        if audit_mode {
            use crate::ness_relay_core::vendor::PluginRegistry;
            use crate::core::audit_runner;

            let registry = std::sync::Arc::new(PluginRegistry::with_defaults());
            match audit_runner::run_audit_phases(device, registry).await {
                Ok(Some(audit_json)) => {
                    if let Some(obj) = audit_json.as_object() {
                        for (k, v) in obj {
                            payload[k] = v.clone();
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    // Even when audit fails, return Ok(payload) so the caller
                    // can decide what to do with the empty/minimal payload.
                    info!(
                        target: "ness_relay::engine",
                        "[{}] audit omitido o falló — devolviendo payload mínimo",
                        device.device_id,
                    );
                }
            }
        }
        Ok(payload)
    }

    // -------------------------------------------------------------------------
    // Recolección de todos los dispositivos + exportación
    // -------------------------------------------------------------------------
    pub async fn collect_all_devices(
        &self,
        devices: &[DeviceConfig],
        audit_mode: bool,
    ) -> Result<()> {
        if devices.is_empty() {
            warn!("No hay dispositivos configurados para recolectar.");
            return Ok(());
        }

        info!("Iniciando ciclo de recolección — {} dispositivo(s)", devices.len());
        let send_url = self.config.send_data_url();
        let install_dir = &self.config.install_dir;

        let mut ok_count = 0u32;
        let mut fail_count = 0u32;
        let mut skipped_count = 0u32;

        // Enviar cada dispositivo individualmente (como hace el Python)
        for device in devices {
            // -------------------------------------------------------------------------
            // Paso clave: solo enviar al servidor si las 8 fases se completaron.
            // Si collect_device retorna Err, NO se envía nada al servidor NESS para
            // evitar falsos positivos con payloads incompletos (ej: solo metadata).
            // -------------------------------------------------------------------------
            let raw_payload = match self.collect_device(device, audit_mode).await {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        "[{}] Recolección incompleta ({}). No se enviará al servidor NESS \
                         para evitar falsos positivos. Se omite este dispositivo en este ciclo.",
                        device.device_id, e
                    );
                    skipped_count += 1;
                    continue;
                }
            };

            // Transformar payload al formato compatible con el servidor (formato Python)
            let payload = payload_compat::transform_for_server(raw_payload);

            // Determinar directorio de salida por dispositivo:
            // devices/<device_type>_<vendor>/output/
            let device_type = payload.get("metadata")
                .and_then(|m| m.get("device_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("generic");
            let vendor = payload.get("metadata")
                .and_then(|m| m.get("vendor"))
                .and_then(|v| v.as_str())
                .unwrap_or("generic");
            let device_out_dir = install_dir
                .join("devices")
                .join(format!("{}_{}", device_type, vendor))
                .join("output");
            let out_dir = device_out_dir.to_str().unwrap_or("devices/output");

            // Exportar JSON local
            if let Err(e) = json_exporter::export(&payload, out_dir).await {
                error!("[{}] Error exportando JSON local: {}", device.device_id, e);
            }

            // Enviar al servidor NESS (un POST por dispositivo)
            match server_sender::send(&send_url, &self.config.api_token, &payload).await {
                Ok(_) => {
                    info!("[{}] Datos enviados al servidor NESS correctamente.", device.device_id);
                    ok_count += 1;
                }
                Err(e) => {
                    error!("[{}] Error enviando al servidor NESS: {}", device.device_id, e);
                    fail_count += 1;
                }
            }
        }

        info!(
            "Ciclo completado — {} exitoso(s), {} fallido(s), {} omitido(s) por recolección incompleta",
            ok_count, fail_count, skipped_count
        );

        if fail_count > 0 {
            Err(anyhow::anyhow!(
                "{} dispositivo(s) fallaron al enviar datos al servidor",
                fail_count
            ))
        } else {
            Ok(())
        }
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------
    fn error_payload(
        &self,
        device: &DeviceConfig,
        profile: &dyn crate::profiles::base::DeviceProfile,
        error_type: &str,
        message: &str,
    ) -> Value {
        json!({
            "metadata": {
                "collection_start": now_iso(),
                "snmp_host": device.ip,
                "snmp_port": device.port,
                "vendor": profile.vendor(),
                "vendor_display_name": profile.vendor_display_name(),
                "device_type": profile.device_type(),
                "relay_version": super::config::RELAY_VERSION,
                "relay_type":    super::config::RELAY_TYPE,
                "description":   device.description,
                "collection_end": now_iso(),
            },
            "system":      {},
            "performance": {},
            "network":     {},
            "security":    {},
            "error": {
                "type":    error_type,
                "message": message,
            },
        })
    }
}
