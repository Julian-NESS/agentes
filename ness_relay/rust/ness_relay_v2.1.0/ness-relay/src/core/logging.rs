// ==============================================================================
// NESS Relay v2.0.0 — Configuración de logging
// Equivalente Python: core/logging_setup.py
// ==============================================================================
//
// Usa tracing + tracing-subscriber + tracing-appender para:
//   - Archivo único (ness_relay.log, sin rotación por fecha)
//   - Consola (INFO y superior cuando es interactivo, silenciable)
//   - Formato: [TIMESTAMP] LEVEL target: mensaje
// ==============================================================================

use std::path::Path;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    filter::LevelFilter,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

/// Inicializa el sistema de logging del relay.
/// log_dir — directorio donde se crearán los archivos de log.
pub fn setup_logging(log_dir: &Path, silent: bool) {
    // Crear el directorio de logs si no existe
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        eprintln!("ADVERTENCIA: No se pudo crear directorio de logs {:?}: {}", log_dir, e);
    }

    // File appender sin rotación — todo va a ness_relay.log
    let file_appender = RollingFileAppender::new(
        Rotation::NEVER,
        log_dir,
        "ness_relay.log",
    );

    // Layer de archivo: nivel INFO y superior
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_span_events(FmtSpan::NONE)
        .with_filter(LevelFilter::from_level(Level::INFO));

    if silent {
        // Modo silencioso: solo archivo, sin consola
        tracing_subscriber::registry()
            .with(file_layer)
            .init();
    } else {
        // Consola: INFO y superior (muestra progreso en tiempo real)
        let console_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .with_span_events(FmtSpan::NONE)
            .with_filter(LevelFilter::from_level(Level::INFO));

        tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer)
            .init();
    }
}
