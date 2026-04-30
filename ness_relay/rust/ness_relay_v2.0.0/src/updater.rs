// ==============================================================================
// NESS Relay v2.0.0 — Auto-actualizador (Refactorizado)
// Equivalente Python: updater.py
// ==============================================================================
//
// Flujo:
//   1. fetch_update_metadata()  — GET latest.json desde GCP
//   2. parse_metadata()         — Parse JSON a UpdateMetadata struct
//   3. is_compatible_upgrade()  — Valida min_supported version
//   4. save_config_before_update() — Preserva variables de entorno
//   5. download_update()        — Descarga el ZIP (streaming)
//   6. verify_hash()            — SHA-256 del ZIP descargado (OBLIGATORIO)
//   7. extract_and_replace()    — Backup + extrae el nuevo binario
//   8. restore_config_after()   — Restaura variables de entorno
//   9. cleanup_backups()        — Borra backups antiguos (mantiene N)
// ==============================================================================

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, warn};

// ==============================================================================
// ESTRUCTURAS DE DATOS
// ==============================================================================

/// Metadata de actualización parseada desde latest.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMetadata {
    pub version: String,
    pub release_date: String,
    pub arch: String,
    pub platform: String,
    pub min_supported: String,
    pub base_url: String,
    pub pack: PackInfo,
    pub changelog: Vec<String>,
}

/// Información del paquete a descargar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackInfo {
    pub url: String,
    pub fileName: String,
    pub sha256: String,
}

// ==============================================================================
// FUNCIONES DE VALIDACIÓN Y PARSEO
// ==============================================================================

/// Compara versiones semver. Retorna true si `remote` > `local`.
fn parse_semver(s: &str) -> (u32, u32, u32) {
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
}

fn is_newer(local: &str, remote: &str) -> bool {
    parse_semver(remote) > parse_semver(local)
}

/// Parsea la respuesta JSON de latest.json en una estructura UpdateMetadata.
pub fn parse_metadata(json_str: &str) -> Result<UpdateMetadata> {
    serde_json::from_str(json_str)
        .context("No se pudo parsear metadata de actualización desde JSON")
}

/// Valida que la versión remota sea compatible con la versión local.
///
/// Retorna Err si:
/// - La versión remota es <= local (no es upgrade)
/// - La versión local es < min_supported (incompatible)
pub fn is_compatible_upgrade(
    local_version: &str,
    remote_version: &str,
    min_supported: &str,
) -> Result<()> {
    let local = parse_semver(local_version);
    let remote = parse_semver(remote_version);
    let min_required = parse_semver(min_supported);

    // Validar que es versión más nueva
    if remote <= local {
        return Err(anyhow!(
            "No es upgrade: versión local {} >= versión remota {}",
            local_version,
            remote_version
        ));
    }

    // Validar compatibilidad hacia atrás
    if local < min_required {
        return Err(anyhow!(
            "Versión local {} es menor que mínima requerida {}",
            local_version,
            min_supported
        ));
    }

    info!(
        "Upgrade compatible validado: {} -> {} (mín. soportada: {})",
        local_version, remote_version, min_supported
    );
    Ok(())
}

/// Descarga y parsea la metadata desde una URL remota.
pub async fn fetch_update_metadata(metadata_url: &str, api_token: &str) -> Result<UpdateMetadata> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .use_rustls_tls()
        .build()?;

    let resp = client
        .get(metadata_url)
        .header("Authorization", format!("Token {}", api_token))
        .send()
        .await
        .context("Error fetching metadata de versión remota")?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Servidor devolvió HTTP {} al consultar metadata",
            resp.status().as_u16()
        ));
    }

    let body = resp
        .text()
        .await
        .context("No se pudo leer respuesta de metadata")?;

    parse_metadata(&body)
}

/// Comprueba si hay una actualización disponible.
///
/// Retorna `Some(UpdateMetadata)` si existe una versión más nueva y compatible.
pub async fn check_for_updates(
    version_check_url: &str,
    api_token: &str,
) -> Result<Option<UpdateMetadata>> {
    info!("Verificando actualizaciones en: {}", version_check_url);

    match fetch_update_metadata(version_check_url, api_token).await {
        Ok(metadata) => {
            let local_version = crate::config::RELAY_VERSION;

            match is_compatible_upgrade(local_version, &metadata.version, &metadata.min_supported) {
                Ok(_) => {
                    info!(
                        "Nueva versión disponible: {} → {}",
                        local_version, metadata.version
                    );
                    Ok(Some(metadata))
                }
                Err(e) => {
                    warn!("Actualización no compatible: {}", e);
                    Ok(None)
                }
            }
        }
        Err(e) => {
            warn!("Error verificando actualizaciones: {}", e);
            Err(e)
        }
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
    let mut resp = client
        .get(url)
        .send()
        .await
        .context("Error descargando actualización")?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Error descargando actualización (HTTP {})",
            resp.status().as_u16()
        ));
    }

    let mut file = std::fs::File::create(&tmp_path)
        .context("No se pudo crear archivo temporal para descarga")?;
    while let Some(chunk) = resp
        .chunk()
        .await
        .context("Error recibiendo chunk de descarga")?
    {
        file.write_all(&chunk)
            .context("Error escribiendo chunk al archivo")?;
    }

    info!("Descarga completada: {} bytes", tmp_path.display());
    Ok(tmp_path)
}

/// Calcula el hash SHA-256 de un archivo.
async fn calculate_sha256(path: &Path) -> Result<String> {
    let data = fs::read(path)
        .await
        .context("No se pudo leer archivo para verificar hash")?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Verifica el hash SHA-256 del archivo descargado.
///
/// # Nota
/// El SHA-256 es OBLIGATORIO. Si está vacío, retorna error.
pub async fn verify_hash(path: &Path, expected_hex: &str) -> Result<()> {
    if expected_hex.is_empty() {
        return Err(anyhow!(
            "SHA-256 vacío: verificación de integridad es OBLIGATORIA"
        ));
    }

    let calculated_hash = calculate_sha256(path).await?;

    if calculated_hash.eq_ignore_ascii_case(expected_hex) {
        info!("✓ Hash SHA-256 verificado correctamente");
        Ok(())
    } else {
        Err(anyhow!(
            "✗ Hash SHA-256 inválido.\nEsperado:  {}\nCalculado: {}",
            expected_hex,
            calculated_hash
        ))
    }
}

/// Guarda la configuración actual antes de actualizar.
/// Integración con config_backup.rs.
pub async fn save_config_before_update(
    api_token: &str,
    server_id: &str,
    collection_interval_minutes: u64,
    devices_config_path: PathBuf,
    output_dir: PathBuf,
    log_dir: PathBuf,
) -> Result<PathBuf> {
    let config = crate::config_backup::PreservedConfig::from_env(
        api_token.to_string(),
        server_id.to_string(),
        collection_interval_minutes,
        devices_config_path,
        output_dir,
        log_dir,
    );

    crate::config_backup::save_config(&config, None)
        .context("No se pudo guardar configuración antes de actualizar")
}

/// Restaura la configuración después de extraer el nuevo binario.
pub async fn restore_config_after_update() -> Result<()> {
    match crate::config_backup::load_config(None) {
        Ok(config) => {
            crate::config_backup::apply_config_as_env_vars(&config)?;
            info!("Configuración restaurada después de actualización");
            Ok(())
        }
        Err(e) => {
            warn!("No se pudo restaurar configuración: {}. Continuando...", e);
            Ok(())
        }
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
/// Retorna `Ok(true)` si la actualización fue exitosa y el agente debe reiniciarse.
pub async fn apply_update(metadata: &UpdateMetadata) -> Result<()> {
    info!(
        "Descargando actualización v{} desde: {}",
        metadata.version, metadata.pack.url
    );

    let zip_path = download_update(&metadata.pack.url).await?;

    verify_hash(&zip_path, &metadata.pack.sha256).await?;

    extract_and_replace(&zip_path, "ness_relay")?;
    let _ = tokio::fs::remove_file(&zip_path).await;

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = cleanup_backups(dir, crate::config::MAX_BACKUPS).await;
        }
    }

    info!(
        "✓ Actualización completada v{} → v{}.",
        crate::config::RELAY_VERSION,
        metadata.version
    );
    Ok(())
}

pub async fn run_update(
    version_check_url: &str,
    api_token: &str,
) -> Result<bool> {
    match check_for_updates(version_check_url, api_token).await? {
        None => {
            info!("No hay actualización disponible.");
            Ok(false)
        }
        Some(metadata) => {
            apply_update(&metadata).await?;
            info!("Agente debe reiniciarse para aplicar la nueva versión.");
            Ok(true)
        }
    }
}
