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
use tracing::{debug, error, info, warn};

use crate::analyzers::{performance as perf_analyzer, security as sec_analyzer};
use crate::collectors::{network, performance as perf_collector, security as sec_collector,
                         system_col, vendor as vendor_collector};
use super::config::{AppConfig, DeviceConfig};
use crate::exporters::{audit_sender, json_exporter, payload_compat, server_sender};
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

        // Numeración dinámica de fases: 8 si audit_mode=false, 10 si audit_mode=true.
        // Esto permite que los logs reflejen correctamente el flujo completo cuando
        // se ejecuta el cron de auditoría cada 6h.
        let total_phases: u8 = if audit_mode { 10 } else { 8 };

        // [1/N] Cargar perfil
        let profile = self.profile_loader.get_profile(&device.vendor);
        info!(
            "[{}] [1/{}] Perfil cargado: {}",
            device.device_id,
            total_phases,
            profile.vendor_display_name()
        );

        // [2/N] Crear cliente SNMP + probar conectividad
        let client = match SnmpClient::new(&device_json).await {
            Ok(c) => Arc::new(c),
            Err(e) => {
                error!("[{}] [2/{}] No se pudo crear cliente SNMP: {}", device.device_id, total_phases, e);
                if audit_mode {
                    warn!(
                        "[{}] SNMP client falló pero audit_mode=true — se construye payload SNMP-vacío y se continúa con audit (phases 9+10)",
                        device.device_id,
                    );
                    return self.audit_only_payload(device, audit_mode, timer, collection_start).await;
                }
                warn!(
                    "[{}] Recolección incompleta en fase 2/{} — NO se enviará al servidor",
                    device.device_id, total_phases
                );
                return Err(anyhow!("snmp_client_error: {}", e));
            }
        };

        info!("[{}] [2/{}] Probando conectividad SNMP…", device.device_id, total_phases);
        let conn_result = client.test_connectivity().await;
        if !conn_result.is_ok() {
            let err_msg = conn_result.error.as_deref().unwrap_or("timeout / no response");
            warn!("[{}] [2/{}] Fallo de conectividad: {}", device.device_id, total_phases, err_msg);
            if audit_mode {
                warn!(
                    "[{}] SNMP caído pero audit_mode=true — se continúa con audit (phases 9+10 sobre SSH)",
                    device.device_id,
                );
                return self.audit_only_payload(device, audit_mode, timer, collection_start).await;
            }
            warn!(
                "[{}] Recolección incompleta en fase 2/{} — NO se enviará al servidor",
                device.device_id, total_phases
            );
            return Err(anyhow!("connectivity_error: {}", err_msg));
        }
        info!("[{}] [2/{}] Conectividad OK", device.device_id, total_phases);

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
            "[{}] [2/{}] Perfil resuelto: {} ({})",
            device.device_id,
            total_phases,
            profile.vendor_display_name(),
            profile.vendor()
        );

        // [3/N] Información del sistema
        info!("[{}] [3/{}] Recolectando sistema…", device.device_id, total_phases);
        let system_data = system_col::collect(&client).await;

        // [4/N] Performance (CPU, memoria, disco)
        info!("[{}] [4/{}] Recolectando performance…", device.device_id, total_phases);
        let perf_data = perf_collector::collect(&client, &profile, &sys_object_id).await;

        // [5/N] Interfaces de red
        info!("[{}] [5/{}] Recolectando interfaces…", device.device_id, total_phases);
        let network_data = network::collect(&client).await;

        // [6/N] Estadísticas de seguridad
        info!("[{}] [6/{}] Recolectando seguridad…", device.device_id, total_phases);
        let security_data = sec_collector::collect(&client).await;

        // [7/N] Datos específicos del vendor
        info!("[{}] [7/{}] Recolectando datos del vendor…", device.device_id, total_phases);
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

        // [8/N] Análisis de alertas
        info!("[{}] [8/{}] Analizando alertas…", device.device_id, total_phases);
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
        // Phase 9 + 10 — SSH-based audit (opt-in, best-effort) — Phase 2.6
        // =========================================================================
        //
        // Se ejecuta AQUÍ (dentro de collect_device) para que los hallazgos
        // de vulnerabilidades/CIS se anexen al payload y se escriban
        // localmente en disco. El envío al server se hace desde
        // `collect_all_devices()` DESPUÉS del POST SNMP (porque el audit
        // endpoint requiere que el device YA exista en la BD).
        //
        // Solo se ejecuta cuando:
        //   1. `audit_mode` está activo (--audit desde main, NESS_AUDIT_ENABLED=true).
        //   2. El device tiene SSH habilitado (ssh_enabled=true en connection.config).
        if audit_mode && device.ssh_enabled {
            use crate::ness_relay_core::vendor::PluginRegistry;
            use crate::core::audit_runner;

            let registry = std::sync::Arc::new(PluginRegistry::with_defaults());
            info!(
                target: "ness_relay::engine",
                "[{}] [9/{}] Escaneando vulnerabilidades vía SSH (Fortinet FortiGate)…",
                device.device_id, total_phases,
            );
            match audit_runner::run_audit_phases(device, registry).await {
                Ok(Some(audit_json)) => {
                    // Anexar los bloques al payload para que se escriban
                    // localmente en disco. El envío al server se hace desde
                    // collect_all_devices() después del POST SNMP.
                    if let Some(vuln_b) = audit_json.get("vulnerabilities") {
                        payload["vulnerabilities"] = vuln_b.clone();
                    }
                    if let Some(cis_b) = audit_json.get("cis_compliance") {
                        payload["cis_compliance"] = cis_b.clone();
                    }

                    let vuln_count = audit_json
                        .get("vulnerabilities")
                        .and_then(|v| v.get("findings"))
                        .and_then(|f| f.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    let cis_count = audit_json
                        .get("cis_compliance")
                        .and_then(|v| v.get("findings"))
                        .and_then(|f| f.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    info!(
                        target: "ness_relay::engine",
                        "[{}] [9/{}] Vulnerabilidades completadas — {} hallazgos encontrados",
                        device.device_id, total_phases, vuln_count,
                    );
                    info!(
                        target: "ness_relay::engine",
                        "[{}] [10/{}] Controles CIS completados — {} hallazgos evaluados",
                        device.device_id, total_phases, cis_count,
                    );
                }
                Ok(None) => {
                    info!(
                        target: "ness_relay::engine",
                        "[{}] [9/{}] Phase 9+10 omitidas (sin credenciales SSH o vendor no soportado)",
                        device.device_id, total_phases,
                    );
                }
                Err(e) => {
                    warn!(
                        target: "ness_relay::engine",
                        "[{}] Phase 9+10 fallaron — {e:#} (continuando)",
                        device.device_id,
                    );
                }
            }
        }

        Ok(payload)
    }

    // -------------------------------------------------------------------------
    // run_audit_phases_post_snmp — Phase 2.5/2.6
    //
    // Wrapper público para tests/futuro. La ejecución real del audit
    // ahora ocurre dentro de `collect_device()` para que los hallazgos
    // estén en el payload cuando collect_all_devices() lo necesite.
    // El envío al server audit endpoint ocurre DESPUÉS del POST SNMP
    // desde collect_all_devices().
    // -------------------------------------------------------------------------
    pub async fn run_audit_phases_post_snmp(
        &self,
        device: &DeviceConfig,
        snmp_payload: &Value,
        audit_mode: bool,
    ) -> Option<Value> {
        // Mantenido por compatibilidad con tests anteriores.
        let _ = (device, snmp_payload, audit_mode);
        None
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
                    // Contar hallazgos
                    let vuln_count = audit_json
                        .get("vulnerabilities")
                        .and_then(|v| v.get("findings"))
                        .and_then(|f| f.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    let cis_count = audit_json
                        .get("cis_compliance")
                        .and_then(|v| v.get("findings"))
                        .and_then(|f| f.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    info!(
                        target: "ness_relay::engine",
                        "[{}] [9/10] Vulnerabilidades completadas — {} hallazgos encontrados",
                        device.device_id, vuln_count,
                    );
                    info!(
                        target: "ness_relay::engine",
                        "[{}] [10/10] Controles CIS completados — {} hallazgos evaluados",
                        device.device_id, cis_count,
                    );
                }
                Ok(None) | Err(_) => {
                    info!(
                        target: "ness_relay::engine",
                        "[{}] [9/10] Vulnerabilidades omitidas (sin credenciales o vendor no soportado)",
                        device.device_id,
                    );
                    info!(
                        target: "ness_relay::engine",
                        "[{}] [10/10] Controles CIS omitidos (sin credenciales o vendor no soportado)",
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

            // ────────────────────────────────────────────────────────
            // Subcarpetas separadas para cada fase del pipeline.
            //
            // ANTES de `transform_for_server` extraemos los bloques de audit
            // (que `transform_for_server` strippea por el gate
            // `NESS_SEND_VULNERABILITIES`). Luego los exportamos a subcarpetas
            // dedicadas ANTES de transformar el resto del payload para el
            // servidor.
            //
            // Convenciones de nombres y rutas (Phase 2.16):
            //   output/snmp/relay_snmp_data.json
            //       → telemetría SNMP (fases 1-8)
            //   output/vulnerabilities/relay_sentinel_vulnerabilities_data.json
            //       → fase 9 (vulns + NVD/KEV/EPSS)
            //   output/cis_compliance/relay_sentinel_cis_data.json
            //       → fase 10 (CIS)
            //
            // Phase 2.5 — CAMBIO DE ORDEN:
            //   1. Extraer blocks de audit del raw_payload (snapshot)
            //   2. Transformar SNMP para el server
            //   3. Escribir SNMP local
            //   4. **ENVIAR SNMP al server** (crea device en BD)
            //   5. Ejecutar Phase 9+10 (audit SSH) — ahora SÍ existe el device
            //   6. Enviar audit a endpoints dedicados + escribir local
            //
            // Bug previo: el audit se ejecutaba ANTES del POST SNMP, así que
            // el server respondía 404 ("dispositivo no encontrado") en el
            // primer ciclo tras la instalación.
            // ────────────────────────────────────────────────────────
            let mut raw_payload = raw_payload;
            let vulns_block = raw_payload.as_object_mut()
                .and_then(|m| m.remove("vulnerabilities"));
            let cis_block = raw_payload.as_object_mut()
                .and_then(|m| m.remove("cis_compliance"));

            // Transformar payload al formato compatible con el servidor (formato Python)
            let payload = payload_compat::transform_for_server(raw_payload);

            // Determinar directorio de salida por dispositivo:
            // devices/<device_type>_<vendor>_<device_idx>/output/
            //
            // Phase 2.7 FIX: incluir device_id en el path para que cada
            // dispositivo tenga su propia carpeta. Antes, los 3 FortiGates
            // escribían en la misma carpeta `firewall_fortinet/`, pisándose
            // los unos a los otros.
            //
            // device_id viene de `load_devices_from_config()` y tiene formato
            // "fortinet_1", "fortinet_2", etc. Lo usamos directamente para
            // garantizar unicidad.
            let device_type = payload.get("metadata")
                .and_then(|m| m.get("device_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("generic");
            let vendor = payload.get("metadata")
                .and_then(|m| m.get("vendor"))
                .and_then(|v| v.as_str())
                .unwrap_or("generic");
            // Phase 2.7: incluir device_id para diferenciar devices del mismo
            // vendor. Si por alguna razón device_id está vacío, caer al
            // formato legacy sin sufijo.
            //
            // Phase 2.18 FIX: `device.device_id` viene como "fortinet_1"
            // y concatenarlo tal cual producía "firewall_fortinet_fortinet_1"
            // (redundante). Ahora extraemos SOLO el índice numérico del
            // final del slug: "fortinet_1" → "1", "fortinet_2" → "2".
            // Resultado: `firewall_fortinet_1`, `firewall_fortinet_2`, etc.
            let dir_name = if !device.device_id.is_empty() {
                // Extraer el último segmento después del "_" (formato: vendor_N)
                let idx_suffix = device.device_id
                    .rsplit('_')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !idx_suffix.is_empty() && idx_suffix.chars().all(|c| c.is_ascii_digit()) {
                    format!("{}_{}_{}", device_type, vendor, idx_suffix)
                } else {
                    // Fallback: device_id no tiene el formato esperado; usar
                    // tal cual para no perder información.
                    format!("{}_{}_{}", device_type, vendor, device.device_id)
                }
            } else {
                format!("{}_{}", device_type, vendor)
            };
            let device_dir = install_dir
                .join("devices")
                .join(&dir_name);
            let out_dir = device_dir.join("output").to_str().unwrap_or("devices/output").to_string();

            debug!(
                "[{}] Directorio de salida: {}",
                device.device_id, out_dir
            );

            // ── SNMP → output/snmp/relay_snmp_data.json ────────────────────
            let snmp_dir = format!("{}/snmp", out_dir);
            match json_exporter::export_as(&payload, &snmp_dir, "relay_snmp_data.json").await {
                Ok(_) => debug!(
                    "[{}] relay_snmp_data.json (SNMP) escrito en {}",
                    device.device_id, snmp_dir
                ),
                Err(e) => error!(
                    "[{}] Error exportando relay_snmp_data.json: {}",
                    device.device_id, e
                ),
            }

            // ── Vulnerabilidades → output/vulnerabilities/relay_sentinel_vulnerabilities_data.json ──
            if let Some(vulns) = vulns_block.as_ref() {
                let vulns_dir = format!("{}/vulnerabilities", out_dir);
                let vulns_filename = "relay_sentinel_vulnerabilities_data.json";
                let count = vulns.get("findings").and_then(|f| f.as_array())
                    .map(|a| a.len()).unwrap_or(0);
                match json_exporter::export_as(vulns, &vulns_dir, vulns_filename).await {
                    Ok(_) => info!(
                        "[{}] relay_sentinel_vulnerabilities_data.json escrito en {}/{} ({} CVEs)",
                        device.device_id, vulns_dir, vulns_filename, count,
                    ),
                    Err(e) => warn!(
                        "[{}] Error exportando {}: {}",
                        device.device_id, vulns_filename, e
                    ),
                }
            }

            // ── CIS → output/cis_compliance/relay_sentinel_cis_data.json ───
            if let Some(cis) = cis_block.as_ref() {
                let cis_dir = format!("{}/cis_compliance", out_dir);
                let cis_filename = "relay_sentinel_cis_data.json";
                let count = cis.get("findings").and_then(|f| f.as_array())
                    .map(|a| a.len()).unwrap_or(0);
                match json_exporter::export_as(cis, &cis_dir, cis_filename).await {
                    Ok(_) => info!(
                        "[{}] relay_sentinel_cis_data.json escrito en {}/{} ({} hallazgos)",
                        device.device_id, cis_dir, cis_filename, count,
                    ),
                    Err(e) => warn!(
                        "[{}] Error exportando {}: {}",
                        device.device_id, cis_filename, e
                    ),
                }
            }

            // Enviar al servidor NESS (un POST por dispositivo)
            //
            // CAMBIO Phase 2.4 — envío BEST-EFFORT siempre (idéntico al flujo SNMP):
            //
            // El contrato del agente es:
            //   1. Recolectar datos (fases 1..N)
            //   2. Escribir localmente en /opt/ness_relay/devices/<vendor>/output/
            //   3. Intentar enviar al servidor NESS HQ
            //
            // Si el envío al servidor falla, se registra como WARN pero el ciclo
            // se considera EXITOSO a nivel local. El operador puede revisar los
            // hallazgos en disco aunque la nube esté caída. Este comportamiento
            // es IGUAL tanto para el modo SNMP normal (cada 5 min) como para el
            // modo auditoría (cada 6h) — no hay diferencia, así el operador
            // puede confiar en que lo escrito en disco siempre está.
            //
            // fail_count SOLO incrementa cuando algo grave pasa (p.ej. SNMP no
            // responde y por lo tanto NO se generó payload). NUNCA por un fallo
            // de red al subir al servidor.
            //
            // Phase 2.5: ESTE POST SNMP AHORA ES CRÍTICO — debe ejecutarse
            // ANTES del audit para que el device exista en BD cuando lleguen
            // los POST de audit.
            match server_sender::send(&send_url, &self.config.api_token, &payload).await {
                Ok(_) => {
                    info!("[{}] Datos SNMP enviados al servidor NESS correctamente.", device.device_id);
                    ok_count += 1;
                }
                Err(e) => {
                    warn!(
                        "[{}] No se pudo enviar SNMP al servidor NESS ({}). \
                         Los hallazgos ya están escritos localmente en {}/devices/<vendor>/output/. \
                         Si la nube NESS HQ está caída, los datos se reintentarán en el próximo ciclo.",
                        device.device_id, e, install_dir.display(),
                    );
                    // Consideramos el dispositivo "completado" localmente,
                    // idéntico al comportamiento de SNMP cuando el server falla.
                    ok_count += 1;
                }
            }

            // ────────────────────────────────────────────────────────
            // Phase 9 + 10 — Enviar audit POST-SNMP (Phase 2.5/2.6)
            // ────────────────────────────────────────────────────────
            //
            // El audit (audit_runner) YA se ejecutó dentro de collect_device()
            // y sus bloques (vulnerabilities/cis_compliance) YA están en `payload`.
            // Aquí solo extraemos esos bloques y los enviamos a los endpoints
            // audit dedicados. Ya se escribieron localmente arriba (líneas ~540-570).
            //
            // El orden es: POST SNMP primero (crea device en BD), luego POST
            // audit (device ya existe). Si el server está caído, los hallazgos
            // quedan en disco local para revisión posterior.
            //
            // Phase 2.18 BUGFIX: el `payload` ya pasó por `transform_for_server`
            // que ELIMINA los bloques `vulnerabilities` y `cis_compliance`
            // (audit data gate). Por eso `payload.get("vulnerabilities")` y
            // `payload.get("cis_compliance")` siempre devolvían `None`, y los
            // endpoints audit dedicados NUNCA recibían los hallazgos. El servidor
            // mostraba 0 vulnerabilidades / 0 controles CIS en TODOS los
            // dispositivos (incluyendo fortinet_1 que sí recolectaba datos
            // correctamente).
            //
            // Solución: usar los SNAPSHOTS `vulns_block` y `cis_block` que se
            // extrajeron ANTES de la transformación.
            if audit_mode && device.ssh_enabled {
                let server_base = self.config.server_url
                    .trim_end_matches("/api/relay/data/")
                    .trim_end_matches("/api/relay/data")
                    .trim_end_matches('/');

                // Extraer sysName del payload SNMP para que el server
                // matchee correctamente el device por hostname.
                let real_hostname = payload
                    .get("system")
                    .and_then(|s| s.get("basic_info"))
                    .and_then(|b| b.get("sys_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let real_hostname = if real_hostname.is_empty() {
                    device.device_id.clone()
                } else {
                    real_hostname
                };
                let ip_address = device.ip.clone();

                // Enviar vulnerabilities a endpoint dedicado
                // Phase 2.18: usar `vulns_block` (snapshot) en vez de
                // `payload.get("vulnerabilities")` que devuelve None porque
                // transform_for_server() strippeó la clave.
                if let Some(vuln_block) = vulns_block.as_ref() {
                    match audit_sender::send_audit_payload(
                        server_base,
                        &self.config.api_token,
                        audit_sender::AuditKind::Vulnerabilities,
                        vuln_block,
                        &device.device_id,
                        &real_hostname,
                        &ip_address,
                    ).await {
                        Ok(()) => info!(
                            "[{} → sysName='{}' ip={}] Vulnerabilidades enviadas a endpoint dedicado",
                            device.device_id, real_hostname, ip_address,
                        ),
                        Err(e) => warn!(
                            "[{} → sysName='{}' ip={}] Fallo enviando vulnerabilidades: {}",
                            device.device_id, real_hostname, ip_address, e,
                        ),
                    }
                } else {
                    warn!(
                        "[{} → sysName='{}' ip={}] No hay bloque vulnerabilities para enviar (audit no se ejecutó)",
                        device.device_id, real_hostname, ip_address,
                    );
                }

                // Enviar CIS a endpoint dedicado
                // Phase 2.18: usar `cis_block` (snapshot) en vez de
                // `payload.get("cis_compliance")` por la misma razón.
                if let Some(cis_b) = cis_block.as_ref() {
                    match audit_sender::send_audit_payload(
                        server_base,
                        &self.config.api_token,
                        audit_sender::AuditKind::Cis,
                        cis_b,
                        &device.device_id,
                        &real_hostname,
                        &ip_address,
                    ).await {
                        Ok(()) => info!(
                            "[{} → sysName='{}' ip={}] Controles CIS enviados a endpoint dedicado",
                            device.device_id, real_hostname, ip_address,
                        ),
                        Err(e) => warn!(
                            "[{} → sysName='{}' ip={}] Fallo enviando CIS: {}",
                            device.device_id, real_hostname, ip_address, e,
                        ),
                    }
                } else {
                    warn!(
                        "[{} → sysName='{}' ip={}] No hay bloque cis_compliance para enviar (audit no se ejecutó)",
                        device.device_id, real_hostname, ip_address,
                    );
                }
            }
        }

        info!(
            "Ciclo completado — {} exitoso(s) (incluye envíos al servidor y recolección local), \
             {} omitido(s) por recolección incompleta",
            ok_count, skipped_count,
        );

        // fail_count ya no existe: el envío al servidor nunca es fatal.
        // El ciclo retorna Ok SIEMPRE, salvo que haya fallos de recolección
        // (que sí se reportan arriba como skipped_count).
        if skipped_count > 0 {
            warn!(
                "{} dispositivo(s) fueron omitidos por recolección incompleta (SNMP sin respuesta). \
                 Esto SÍ requiere atención: revise conectividad SNMP.",
                skipped_count,
            );
        }
        Ok(())
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
