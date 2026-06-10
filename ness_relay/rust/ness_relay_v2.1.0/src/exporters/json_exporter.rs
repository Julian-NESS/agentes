// ==============================================================================
// NESS Relay v2.0.0 — Exportador JSON (archivo local)
// Equivalente Python: exporters/json_exporter.py
// ==============================================================================
//
// Escribe el payload de telemetría en:
//   {output_dir}/relay_data.json
//
// Si el archivo existe se sobreescribe.  Al igual que en Python, se usa
// formato pretty-print y el archivo es legible por el servidor NESS.
// ==============================================================================

use anyhow::Result;
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

/// Escribe `data` en `{output_dir}/relay_data.json` (pretty-printed).
/// Crea el directorio si no existe.
pub async fn export(data: &serde_json::Value, output_dir: &str) -> Result<()> {
    let dir = Path::new(output_dir);
    if !dir.exists() {
        fs::create_dir_all(dir).await?;
    }

    let path = dir.join("relay_data.json");
    let json_str = serde_json::to_string_pretty(data)?;

    fs::write(&path, json_str.as_bytes()).await?;
    debug!("relay_data.json escrito en {}", path.display());
    Ok(())
}

/// Retorna el contenido del archivo JSON exportado previamente, si existe.
pub async fn read_exported(output_dir: &str) -> Option<serde_json::Value> {
    let path = Path::new(output_dir).join("relay_data.json");
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
