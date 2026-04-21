// ==============================================================================
// NESS Relay v2.0.0 — Auto-actualizador
// Equivalente Python: updater.py
// ==============================================================================
//
// Flujo:
//   1. check_for_updates()  — GET {version_check_url} → compara semver
//   2. download_update()    — descarga el ZIP del nuevo binario (streaming)
//   3. verify_hash()        — SHA-256 del ZIP descargado
//   4. extract_and_replace()— backup del binario actual + extrae el nuevo
//   5. cleanup_backups()    — borra backups antiguos (mantiene N)
// ==============================================================================

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{error, info, warn};

/// Compara versiones semver.  Retorna true si `remote` > `local`.
fn is_newer(local: &str, remote: &str) -> bool {
    let parse = |s: &str| -> (u32, u32, u32) {
        let p: Vec<u32> = s
            .trim_start_matches('v')
            .split('.')
            .filter_map(|x| x.parse().ok())
            .collect();
        (
            *p.first().unwrap_or(&0),
            *p.get(1).unwrap_or(&0),
            *p.get(2).unwrap_or(&0),
        )
    };
    parse(remote) > parse(local)
}

/// Consulta el servidor para ver si hay una versión más reciente.
/// Retorna `Some(download_url)` si existe una actualización disponible.
pub async fn check_for_updates(version_check_url: &str, api_token: &str) -> Option<String> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .use_rustls_tls()
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    let resp = match client
        .get(version_check_url)
        .header("Authorization", format!("Token {}", api_token))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("Error consultando versión remota: {}", e);
            return None;
        }
    };

    if !resp.status().is_success() {
        warn!(
            "Servidor devolvió HTTP {} al consultar versión",
            resp.status().as_u16()
        );
        return None;
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            warn!("Respuesta de versión no es JSON válido: {}", e);
            return None;
        }
    };

    let remote_version = body.get("version").and_then(|v| v.as_str())?;
    let download_url   = body.get("download_url").and_then(|v| v.as_str())?;
    let local_version  = crate::config::RELAY_VERSION;

    if is_newer(local_version, remote_version) {
        info!(
            "Nueva versión disponible: {} → {}",
            local_version, remote_version
        );
        Some(download_url.to_string())
    } else {
        info!("Versión actual ({}) es la más reciente.", local_version);
        None
    }
}

/// Descarga el archivo de actualización (ZIP) de forma streaming.
/// Retorna la ruta al archivo descargado en `/tmp`.
pub async fn download_update(url: &str) -> Result<PathBuf> {
    let tmp_path = PathBuf::from("/tmp/ness_relay_update.zip");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .use_rustls_tls()
        .build()?;

    info!("Descargando actualización desde {}", url);
    let mut resp = client.get(url).send().await?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Error descargando actualización (HTTP {})",
            resp.status().as_u16()
        ));
    }

    let mut file = std::fs::File::create(&tmp_path)?;
    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk)?;
    }

    info!("Descarga completada: {}", tmp_path.display());
    Ok(tmp_path)
}

/// Verifica el hash SHA-256 del archivo descargado.
/// `expected_hex` — SHA-256 en hexadecimal (puede ser vacío para omitir).
pub async fn verify_hash(path: &Path, expected_hex: &str) -> Result<()> {
    if expected_hex.is_empty() {
        return Ok(());
    }

    let data = fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = format!("{:x}", hasher.finalize());

    if hash.eq_ignore_ascii_case(expected_hex) {
        info!("Hash SHA-256 verificado correctamente.");
        Ok(())
    } else {
        Err(anyhow!(
            "Hash SHA-256 inválido. Esperado: {}, Calculado: {}",
            expected_hex,
            hash
        ))
    }
}

/// Extrae el binario del ZIP y reemplaza el ejecutable actual.
/// Hace un backup del binario anterior antes de reemplazar.
///
/// # Arguments
/// * `zip_path`    — Ruta al ZIP descargado
/// * `binary_name` — Nombre del binario dentro del ZIP (ej. "ness_relay")
pub fn extract_and_replace(zip_path: &Path, binary_name: &str) -> Result<PathBuf> {
    // Ruta del ejecutable actual (el propio proceso)
    let current_exe = std::env::current_exe()?;
    let install_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow!("No se pudo determinar el directorio de instalación"))?;

    // Backup del binario actual
    let backup_name = format!(
        "ness_relay.{}.bak",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );
    let backup_path = install_dir.join(&backup_name);
    std::fs::copy(&current_exe, &backup_path)?;
    info!("Backup del binario actual: {}", backup_path.display());

    // Extraer nuevo binario del ZIP
    let zip_file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file)?;

    let new_binary_path = install_dir.join(binary_name);

    let mut found = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.name() == binary_name || entry.name().ends_with(&format!("/{}", binary_name)) {
            let mut out = std::fs::File::create(&new_binary_path)?;
            std::io::copy(&mut entry, &mut out)?;
            found = true;
            break;
        }
    }

    if !found {
        // Restaurar backup
        std::fs::copy(&backup_path, &current_exe)?;
        return Err(anyhow!(
            "No se encontró '{}' dentro del ZIP de actualización",
            binary_name
        ));
    }

    // Permisos de ejecución en Linux
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&new_binary_path, perms)?;
    }

    info!("Binario actualizado: {}", new_binary_path.display());
    Ok(new_binary_path)
}

/// Elimina backups antiguos del directorio de instalación.
/// Mantiene los `max_count` más recientes.
pub async fn cleanup_backups(install_dir: &Path, max_count: usize) -> Result<()> {
    let mut backups: Vec<PathBuf> = Vec::new();
    let mut entries = fs::read_dir(install_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("ness_relay.") && name_str.ends_with(".bak") {
            backups.push(entry.path());
        }
    }

    // Ordenar por nombre (el timestamp en el nombre garantiza orden cronológico)
    backups.sort();

    if backups.len() > max_count {
        let to_remove = backups.len() - max_count;
        for path in backups.iter().take(to_remove) {
            if let Err(e) = fs::remove_file(path).await {
                warn!("No se pudo eliminar backup {}: {}", path.display(), e);
            } else {
                info!("Backup eliminado: {}", path.display());
            }
        }
    }

    Ok(())
}

/// Punto de entrada del proceso de actualización completo.
pub async fn run_update(
    version_check_url: &str,
    api_token: &str,
    expected_sha256: &str,
) -> Result<bool> {
    match check_for_updates(version_check_url, api_token).await {
        None => return Ok(false),
        Some(download_url) => {
            let zip_path = download_update(&download_url).await?;

            if let Err(e) = verify_hash(&zip_path, expected_sha256).await {
                error!("Verificación de hash fallida: {}", e);
                let _ = tokio::fs::remove_file(&zip_path).await;
                return Err(e);
            }

            extract_and_replace(&zip_path, "ness_relay")?;
            let _ = tokio::fs::remove_file(&zip_path).await;

            // Limpiar backups viejos
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    let _ = cleanup_backups(dir, 3).await;
                }
            }

            info!("Actualización completada. Reinicia el agente para aplicar la nueva versión.");
            Ok(true)
        }
    }
}
