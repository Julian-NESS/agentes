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
mod config;
mod engine;
mod exporters;
mod logging;
mod profiles;
mod smart_tester;
mod snmp;
mod updater;
mod utils;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::config::{AppConfig, load_devices_from_config};
use crate::engine::CollectionEngine;

// ==============================================================================
// CLI
// ==============================================================================

#[derive(Parser, Debug)]
#[command(
    name = "ness_relay",
    about = "NESS Relay Multi-Vendor v2.0.0 — Agente de monitoreo SNMP",
    version = config::RELAY_VERSION,
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
            config::RELAY_VERSION,
            config::RELAY_TYPE
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

        smart_tester::run_verify_setup(
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
    logging::setup_logging(
        &app_config.log_dir,
        args.silent,
    );

    // Modo actualización
    if args.update {
        info!("Buscando actualizaciones…");
        match updater::run_update(
            &app_config.version_check_url,
            &app_config.api_token,
            "",     // sha256 verificación (vacío = omitir)
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
        config::RELAY_VERSION,
        app_config.server_url
    );

    // Verificar token de API
    if app_config.api_token.is_empty() {
        warn!("NESS_API_TOKEN no está configurado. Los datos no podrán enviarse al servidor.");
    }

    if args.continuous.is_some() {
        run_continuous(app_config, args.continuous.unwrap()).await
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

async fn run_continuous(config: Arc<AppConfig>, interval_minutes: u64) -> Result<()> {
    info!(
        "Modo continuo activado — ciclo cada {} minuto(s).",
        interval_minutes
    );
    let delay = Duration::from_secs(interval_minutes * 60);

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

        info!("Próximo ciclo en {} minuto(s).", interval_minutes);
        tokio::time::sleep(delay).await;
    }
}
