// ==============================================================================
// NESS Relay v2.0.0 — Punto de entrada
// Equivalente Python: main.py / __main__.py
// ==============================================================================
//
// Binary estático compilado con target musl — funciona en cualquier Linux.
//
// Uso:
//   ness_relay                          # Una sola ejecución
//   ness_relay --continuous 5           # Ciclos cada 5 minutos
//   ness_relay --update                 # Buscar e instalar actualización
//   ness_relay --version                # Mostrar versión
//   ness_relay --silent                 # Sin salida en consola
//   ness_relay --config /ruta/a.conf    # Dispositivos alternativos
// ==============================================================================

mod analyzers;
mod collectors;
mod core;
mod credential_cli;
mod exporters;
mod profiles;
mod secrets;
mod snmp;
mod utils;

// ness-relay-core (lib) — SSH audit, vendor plugins, CIS checks, vulns.
// Re-exported under `crate::ness_relay_core::*`.
extern crate ness_relay_core;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::core::config::{AppConfig, load_devices_from_config};
use crate::core::engine::CollectionEngine;
use crate::credential_cli::CredentialCmd;

// ==============================================================================
// CLI
// ==============================================================================

#[derive(Parser, Debug)]
#[command(
    name = "ness_relay",
    about = "NESS Relay Multi-Vendor v2.0.0 — Agente de monitoreo SNMP",
    version = core::config::RELAY_VERSION,
    disable_version_flag = true,
    long_about = "Agente de recolección SNMP multi-vendor para la plataforma NESS.\n\
                  Compila a un binario estático musl compatible con cualquier Linux."
)]
struct Args {
    /// Ruta al archivo de configuración de dispositivos.
    #[arg(long, short = 'c', value_name = "FILE")]
    config: Option<PathBuf>,

    /// Ejecutar en modo continuo con el intervalo especificado (en minutos).
    #[arg(long, value_name = "MINUTES")]
    continuous: Option<u64>,

    /// Buscar e instalar actualizaciones del relay.
    #[arg(long)]
    update: bool,

    /// Omite chequeo automático de actualización en modo continuo.
    #[arg(long, hide = true)]
    skip_update_check: bool,

    /// Silenciar la salida en consola (solo escribe en el archivo de log).
    #[arg(long)]
    silent: bool,

    /// Mostrar la versión y salir.
    #[arg(long, short = 'V')]
    version: bool,

    /// Ejecuta el Smart Tester interno de pre-validación de entorno/red/SNMP.
    #[arg(long, hide = true)]
    verify_setup: bool,

    /// Permite al Smart Tester aplicar correcciones automáticas (por ejemplo cron).
    #[arg(long, hide = true)]
    verify_auto_fix: bool,

    /// Ejecuta el Smart Tester en modo no interactivo (responde sí a prompts críticos).
    #[arg(long, hide = true)]
    verify_assume_yes: bool,

    /// URL de conectividad HTTPS para el Smart Tester (override temporal).
    #[arg(long, hide = true, value_name = "URL")]
    verify_server_url: Option<String>,

    /// Activar fases 9 (vulnerabilidades) y 10 (CIS) con SSH.
    ///
    /// Esta bandera es **opt-in**: el binario además consulta
    /// `NESS_AUDIT_ENABLED=true` en el entorno. Si la variable no está
    /// presente o es distinta de `true`, `--audit` se convierte en no-op
    /// silencioso (exit 0, sin tocar dispositivos) — útil para que el
    /// cron `audit_relay.sh` no falle cuando el opt-in fue retirado.
    #[arg(long)]
    audit: bool,

    /// Detecta el vendor real de un device vía SNMP (sysObjectID/sysDescr).
    ///
    /// Solo ejecuta las fases 1 (load profile) + 2 (SNMP connectivity +
    /// resolve_profile) y emite por stdout el slug del vendor detectado
    /// (e.g. `fortinet`, `pfsense`, `mikrotik`, etc.). Sale con código 0.
    ///
    /// Códigos de salida:
    /// - 0: vendor detectado, slug en stdout
    /// - 2: cliente SNMP no se pudo construir
    /// - 3: device inalcanzable / sin respuesta SNMP
    /// - 4: IP no encontrada en connection.config
    #[arg(long, value_name = "IP")]
    probe: Option<String>,

    /// Subcomandos de gestión de credenciales (Phase 2.5.1).
    ///
    /// Antes existía un binario separado `ness-relay-cred`. Ahora toda la
    /// lógica vive aquí como subcomandos. Ejemplos:
    ///   sudo ness-relay credential migrate-plaintext
    ///   sudo ness-relay credential set NESS_SSH_PASSWORD_fortinet_1
    ///   sudo ness-relay credential status
    #[command(subcommand)]
    credential: Option<CredentialCmd>,
}

// ==============================================================================
// ENTRY POINT
// ==============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Mostrar versión y salir
    if args.version {
        println!(
            "NESS Relay Multi-Vendor v{} ({})",
            core::config::RELAY_VERSION,
            core::config::RELAY_TYPE
        );
        return Ok(());
    }

    // Subcomando de credenciales: ness-relay credential ...
    // (Phase 2.5.1: reemplazo del binario separado ness-relay-cred.)
    // Se procesa ANTES de setup_logging y sin tocar el resto del pipeline,
    // para que install_relay.sh pueda consumir la salida limpia.
    if let Some(cmd) = args.credential {
        // `credential_cli::run` retorna ExitCode; el proceso debe
        // terminar con ese código de salida. ExitCode no tiene método
        // público para extraer el u8 interno, así que parseamos su
        // representación Debug ("ExitCode(N)" o "ExitCode") y salimos
        // con `process::exit`.
        let code = credential_cli::run(cmd);
        let dbg = format!("{:?}", code);
        let raw: i32 = dbg
            .trim_start_matches("ExitCode(")
            .trim_end_matches(")")
            .parse()
            .unwrap_or(0);
        if raw != 0 {
            std::process::exit(raw);
        }
        return Ok(());
    }

    // Determinar directorio base (donde está el ejecutable)
    let base_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // Cargar configuración principal
    let mut app_config = AppConfig::load(base_dir);

    // Override de archivo de dispositivos si se pasó por CLI
    if let Some(ref cfg_path) = args.config {
        app_config.config_file = cfg_path.clone();
    }

    // Modo Smart Tester (comando oculto para el instalador)
    if args.verify_setup {
        let endpoint = args
            .verify_server_url
            .clone()
            .or_else(|| Some(app_config.server_url.clone()));

        core::smart_tester::run_verify_setup(
            app_config.config_file.clone(),
            endpoint,
            args.verify_auto_fix,
            args.verify_assume_yes,
        )
        .await?;
        return Ok(());
    }

    // Modo --probe: detecta el vendor real de un device vía SNMP.
    // Sale con código 0 + slug en stdout, o códigos 2/3/4 según la causa.
    // Importante: corre ANTES de setup_logging y sin tocar el resto del
    // pipeline, para que `install_relay.sh` pueda consumir la salida limpia.
    if let Some(ref ip_filter) = args.probe {
        let app_config_arc = Arc::new(app_config);
        return run_probe(app_config_arc, ip_filter.clone()).await;
    }

    let app_config = Arc::new(app_config);

    // Inicializar logging
    core::logging::setup_logging(
        &app_config.log_dir,
        args.silent,
    );

    // Modo actualización
    if args.update {
        info!("Buscando actualizaciones…");
        match core::updater::run_update(
            &app_config.version_check_url,
            &app_config.api_token,
        )
        .await
        {
            Ok(true)  => info!("Actualización instalada. Reinicia el agente."),
            Ok(false) => info!("No hay actualizaciones disponibles."),
            Err(e)    => error!("Error durante la actualización: {}", e),
        }
        return Ok(());
    }

    info!(
        "NESS Relay v{} iniciando — servidor: {}",
        core::config::RELAY_VERSION,
        app_config.server_url
    );

    // Verificar token de API
    if app_config.api_token.is_empty() {
        warn!("NESS_API_TOKEN no está configurado. Los datos no podrán enviarse al servidor.");
    }

    // Resolver audit_mode: --audit solo es válido si NESS_AUDIT_ENABLED=true.
    // Si el opt-in fue retirado pero la línea de cron quedó registrada,
    // queremos que el agente salga silenciosamente (exit 0) sin tocar nada.
    let audit_mode = resolve_audit_mode(args.audit);

    if args.continuous.is_some() {
        run_continuous(
            app_config,
            args.continuous.unwrap(),
            args.skip_update_check,
            audit_mode,
        )
        .await
    } else {
        run_once(app_config, audit_mode).await
    }
}

/// Resolves the effective audit mode for this process.
///
/// Returns `true` only when:
///   - `--audit` was passed **and**
///   - `NESS_AUDIT_ENABLED=true` in the process environment
///
/// Otherwise returns `false`. When `--audit` was passed but the gate is
/// closed, we emit a single info log so operators can see why nothing
/// happened in cron-audit runs.
fn resolve_audit_mode(audit_flag: bool) -> bool {
    if !audit_flag {
        return false;
    }
    let enabled = std::env::var("NESS_AUDIT_ENABLED")
        .map(|v| v.eq_ignore_ascii_case("true") || v.trim() == "1")
        .unwrap_or(false);
    if !enabled {
        info!(
            target: "ness_relay::main",
            "NESS_AUDIT_ENABLED no está activado; --audit se omite silenciosamente. \
             Para habilitar, ejecute el instalador y responda 'y' al prompt de auditoría."
        );
        return false;
    }
    info!(
        target: "ness_relay::main",
        "Modo auditoría ACTIVADO (NESS_AUDIT_ENABLED=true). Phases 9+10 se ejecutarán contra dispositivos con SSH configurado."
    );
    true
}

// ==============================================================================
// Ejecución única
// ==============================================================================

async fn run_once(config: Arc<AppConfig>, audit_mode: bool) -> Result<()> {
    let devices = load_devices_from_config(&config.config_file)?;

    if devices.is_empty() {
        warn!(
            "No se encontraron dispositivos en {}",
            config.config_file.display()
        );
        info!("Crea el archivo con el formato:");
        info!("  pfsense_1_ip=192.168.1.1");
        info!("  pfsense_1_community=public");
        info!("  pfsense_1_snmp_version=2c");
        info!("  # (opcional) fortinet_1_ssh_enabled=true");
        info!("  # (opcional) fortinet_1_ssh_username=admin");
        info!("  # (opcional) fortinet_1_ssh_password_env=NESS_SSH_PASSWORD_fortigate_1");
        return Ok(());
    }

    let engine = CollectionEngine::new(Arc::clone(&config));
    engine.collect_all_devices(&devices, audit_mode).await?;
    info!("Ciclo de recolección completado.");
    Ok(())
}

// ==============================================================================
// Ejecución continua
// ==============================================================================

async fn run_continuous(
    config: Arc<AppConfig>,
    interval_minutes: u64,
    skip_update_check: bool,
    audit_mode: bool,
) -> Result<()> {
    info!(
        "Modo continuo activado — ciclo cada {} minuto(s){}.",
        interval_minutes,
        if audit_mode { " (audit_mode=on)" } else { "" },
    );
    let delay = Duration::from_secs(interval_minutes * 60);
    let mut update_state = core::update_tracker::load_state(None).unwrap_or_default();

    loop {
        let devices = match load_devices_from_config(&config.config_file) {
            Ok(d) => d,
            Err(e) => {
                error!("Error cargando dispositivos: {}", e);
                tokio::time::sleep(delay).await;
                continue;
            }
        };

        let engine = CollectionEngine::new(Arc::clone(&config));
        if let Err(e) = engine.collect_all_devices(&devices, audit_mode).await {
            error!("Error en el ciclo de recolección: {}", e);
        }

        if !skip_update_check
            && core::update_tracker::should_check_now(
                &update_state,
                Some(core::config::UPDATE_CHECK_INTERVAL_HOURS),
            )
        {
            info!("Iniciando chequeo programado de actualización...");
            core::update_tracker::mark_check_completed(&mut update_state);
            if let Err(e) = core::update_tracker::save_state(&update_state, None) {
                warn!("No se pudo persistir estado de chequeo: {}", e);
            }

            match core::updater::check_for_updates(&config.version_check_url, &config.api_token).await {
                Ok(Some(metadata)) => {
                    if !config.api_token.is_empty() {
                        if let Err(e) = core::server_reporter::report_update_available(
                            &config.update_report_url,
                            &config.api_token,
                            core::config::RELAY_VERSION,
                            &metadata.version,
                        )
                        .await
                        {
                            warn!("No se pudo reportar update disponible: {}", e);
                        }
                    }

                    if let Err(e) = core::updater::save_config_before_update(
                        &config.api_token,
                        &config.server_id,
                        interval_minutes,
                        config.config_file.clone(),
                        config.output_dir.clone(),
                        config.log_dir.clone(),
                    )
                    .await
                    {
                        warn!("No se pudo guardar configuración previa a update: {}", e);
                    }

                    if !config.api_token.is_empty() {
                        if let Err(e) = core::server_reporter::report_update_started(
                            &config.update_report_url,
                            &config.api_token,
                            core::config::RELAY_VERSION,
                            &metadata.version,
                        )
                        .await
                        {
                            warn!("No se pudo reportar inicio de update: {}", e);
                        }
                    }

                    match core::updater::apply_update(&metadata).await {
                        Ok(_) => {
                            let _ = core::updater::restore_config_after_update().await;

                            if !config.api_token.is_empty() {
                                if let Err(e) = core::server_reporter::report_update_completed(
                                    &config.update_report_url,
                                    &config.api_token,
                                    core::config::RELAY_VERSION,
                                    &metadata.version,
                                )
                                .await
                                {
                                    warn!("No se pudo reportar update completado: {}", e);
                                }
                            }

                            if let Err(e) = core::update_tracker::mark_update_pending(
                                &mut update_state,
                                metadata.version.clone(),
                                None,
                            ) {
                                warn!("No se pudo marcar update pendiente: {}", e);
                            }

                            if !config.api_token.is_empty() {
                                if let Err(e) = core::server_reporter::report_update_pending(
                                    &config.update_report_url,
                                    &config.api_token,
                                    core::config::RELAY_VERSION,
                                    &metadata.version,
                                )
                                .await
                                {
                                    warn!("No se pudo reportar update pendiente: {}", e);
                                }
                            }

                            match core::restart_handler::trigger_graceful_restart(
                                &metadata.version,
                                None,
                            ) {
                                Ok(_) => {
                                    if let Err(e) = core::update_tracker::mark_update_completed(
                                        &mut update_state,
                                        None,
                                    ) {
                                        warn!("No se pudo marcar update completado: {}", e);
                                    }
                                    info!("Actualización aplicada. Cerrando proceso para reinicio controlado.");
                                    return Ok(());
                                }
                                Err(e) => {
                                    let msg = format!("No se pudo marcar restart graceful: {}", e);
                                    error!("{}", msg);
                                    let _ = core::update_tracker::mark_update_failed(
                                        &mut update_state,
                                        msg.clone(),
                                        None,
                                    );
                                    if !config.api_token.is_empty() {
                                        let _ = core::server_reporter::report_update_failed(
                                            &config.update_report_url,
                                            &config.api_token,
                                            core::config::RELAY_VERSION,
                                            &msg,
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let msg = format!("Error aplicando actualización: {}", e);
                            error!("{}", msg);
                            let _ = core::update_tracker::mark_update_failed(
                                &mut update_state,
                                msg.clone(),
                                None,
                            );
                            if !config.api_token.is_empty() {
                                let _ = core::server_reporter::report_update_failed(
                                    &config.update_report_url,
                                    &config.api_token,
                                    core::config::RELAY_VERSION,
                                    &msg,
                                )
                                .await;
                            }
                        }
                    }
                }
                Ok(None) => {
                    info!("No hay nueva versión disponible en este ciclo.");
                }
                Err(e) => {
                    warn!("Chequeo de actualización falló: {}", e);
                }
            }
        }

        info!("Próximo ciclo en {} minuto(s).", interval_minutes);
        tokio::time::sleep(delay).await;
    }
}

// ==============================================================================
// run_probe — Detecta el vendor real de un device vía SNMP
// ==============================================================================
//
// Solo ejecuta fases 1+2 del pipeline (load profile + SNMP connectivity +
// resolve_profile) y emite el slug del vendor detectado por stdout.
//
// Códigos de salida (contrato con `install_relay.sh`):
//   0 — vendor detectado, slug en stdout
//   2 — cliente SNMP no se pudo construir
//   3 — device inalcanzable / sin respuesta SNMP
//   4 — IP no encontrada en connection.config
//
// Por diseño corre ANTES de `setup_logging` para mantener stdout limpio
// (solo una línea con el slug). El instalador captura esa línea.
async fn run_probe(config: Arc<AppConfig>, ip_filter: String) -> Result<()> {
    use crate::profiles::loader::ProfileLoader;
    use crate::snmp::SnmpClient;

    let devices = match load_devices_from_config(&config.config_file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("probe_error: failed to load devices: {}", e);
            std::process::exit(2);
        }
    };

    let device = match devices.iter().find(|d| d.ip == ip_filter) {
        Some(d) => d,
        None => {
            eprintln!("probe_error: ip_not_found={}", ip_filter);
            std::process::exit(4);
        }
    };

    // Phase 1 — load profile (initial)
    let loader = ProfileLoader::new();
    let _initial = loader.get_profile(&device.vendor);

    // Phase 2 — SNMP client + connectivity + resolve
    let device_json = device.to_json();
    let client = match SnmpClient::new(&device_json).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("probe_error: snmp_client_error: {}", e);
            std::process::exit(2);
        }
    };

    if !client.test_connectivity().await.is_ok() {
        eprintln!("probe_error: unreachable={}", ip_filter);
        std::process::exit(3);
    }

    // Fetch sysDescr + sysObjectID
    let sys_descr_result = client.get("1.3.6.1.2.1.1.1.0").await;
    let sys_descr = sys_descr_result
        .value
        .as_ref()
        .map(|v| v.as_string())
        .unwrap_or_default();

    let sys_object_id_result = client.get("1.3.6.1.2.1.1.2.0").await;
    let sys_object_id = sys_object_id_result
        .value
        .as_ref()
        .map(|v| v.as_string())
        .unwrap_or_default();

    // Resolve profile
    let resolved = loader.resolve_profile(&device.vendor, &sys_object_id, &sys_descr);
    let slug = resolved.vendor().to_string();
    let label = resolved.vendor_display_name().to_string();

    // Salida: UNA sola línea con el slug del vendor detectado (stdout).
    // El label completo va por stderr para debugging sin contaminar stdout.
    // El instalador captura solo la primera línea de stdout con `head -1`.
    println!("{}", slug);

    eprintln!(
        "probe_debug: ip={} resolved_vendor={} display_name={} sys_oid={}",
        ip_filter, slug, label, sys_object_id
    );

    std::process::exit(0);
}
