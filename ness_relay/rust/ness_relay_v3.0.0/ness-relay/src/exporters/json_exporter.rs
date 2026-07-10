// ==============================================================================
// NESS Relay v2.4.0 — Exportador JSON (archivo local)
// Equivalente Python: exporters/json_exporter.py
// ==============================================================================
//
// Phase 2.16: convención de nombres estandarizada para que la jerarquía
// de archivos JSON se corresponda 1:1 con los tipos de datos:
//
//   {output_dir}/<archivo>.json                 ← siempre dentro de out_dir
//
// Para mantener compatibilidad con las convenciones anteriores el nombre
// por defecto sigue siendo "relay_data.json", pero ahora el nombre se
// puede especificar explícitamente — la convención nueva es:
//
//   SNMP            → <device>/output/snmp/relay_snmp_data.json
//   Vulnerabilidades → <device>/output/vulnerabilities/relay_sentinel_vulnerabilities_data.json
//   CIS             → <device>/output/cis_compliance/relay_sentinel_cis_data.json
//
// Si el archivo existe se sobreescribe. Pretty-print y formato legible
// por el servidor NESS HQ.
// ==============================================================================

use anyhow::Result;
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

/// Nombre de archivo JSON por defecto (legacy — retrocompatibilidad con
/// installers/scripts que aún esperan `relay_data.json`).
pub const DEFAULT_FILENAME: &str = "relay_data.json";

/// Escribe `data` en `{output_dir}/{filename}.json` (pretty-printed).
/// Crea el directorio si no existe.
///
/// Si no se pasa `filename`, usa [DEFAULT_FILENAME].
pub async fn export(data: &serde_json::Value, output_dir: &str) -> Result<()> {
    export_as(data, output_dir, DEFAULT_FILENAME).await
}

/// Escribe `data` en `{output_dir}/{filename}.json` (pretty-printed).
/// Crea el directorio si no existe.
///
/// Phase 2.16: nombre de archivo explícito para evitar confusión entre
/// los distintos tipos de telemetría (SNMP vs vulns vs CIS).
pub async fn export_as(
    data: &serde_json::Value,
    output_dir: &str,
    filename: &str,
) -> Result<()> {
    let dir = Path::new(output_dir);
    if !dir.exists() {
        fs::create_dir_all(dir).await?;
    }

    // Validación defensiva: impedir path traversal en filename
    if filename.is_empty() || filename.contains('/') || filename.contains("..") {
        anyhow::bail!("nombre de archivo inválido: '{filename}'");
    }

    let path = dir.join(filename);
    let json_str = serde_json::to_string_pretty(data)?;

    fs::write(&path, json_str.as_bytes()).await?;
    debug!("{} escrito en {}", filename, path.display());
    Ok(())
}

/// Retorna el contenido del archivo JSON exportado previamente, si existe.
pub async fn read_exported(output_dir: &str) -> Option<serde_json::Value> {
    read_exported_as(output_dir, DEFAULT_FILENAME).await
}

/// Lee `{output_dir}/{filename}.json`. Variante con nombre explícito.
pub async fn read_exported_as(output_dir: &str, filename: &str) -> Option<serde_json::Value> {
    let path = Path::new(output_dir).join(filename);
    match fs::read_to_string(&path).await {
        Ok(contents) => match serde_json::from_str(&contents) {
            Ok(v) => Some(v),
            Err(e) => {
                warn!("No se pudo parsear {}: {}", path.display(), e);
                None
            }
        },
        Err(_) => None,
    }
}
