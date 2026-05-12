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
mod exporters;
mod profiles;
mod snmp;
mod utils;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::core::config::{AppConfig, load_devices_from_config};
use crate::core::engine::CollectionEngine;

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

    if args.continuous.is_some() {
        run_continuous(
            app_config,
            args.continuous.unwrap(),
            args.skip_update_check,
        )
        .await
    } else {
        run_once(app_config).await
    }
}

// ==============================================================================
// Ejecución única
// ==============================================================================

async fn run_once(config: Arc<AppConfig>) -> Result<()> {
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
        return Ok(());
    }

    let engine = CollectionEngine::new(Arc::clone(&config));
    engine.collect_all_devices(&devices).await?;
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
) -> Result<()> {
    info!(
        "Modo continuo activado — ciclo cada {} minuto(s).",
        interval_minutes
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
        if let Err(e) = engine.collect_all_devices(&devices).await {
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
