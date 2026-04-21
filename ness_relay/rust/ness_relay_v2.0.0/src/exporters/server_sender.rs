// ==============================================================================
// NESS Relay v2.0.0 — Enviador de datos al servidor NESS
// Equivalente Python: exporters/server_sender.py
// ==============================================================================
//
// POST a {server_url}/api/relay/receive-data/?server_id={server_id}
// Headers: Authorization: Token {api_token}
//          Content-Type: application/json
//
// Timeout: 30 segundos
// Usa rustls (TLS puro Rust, sin OpenSSL)
// ==============================================================================

use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Envía el payload JSON al servidor NESS via HTTP POST.
///
/// # Arguments
/// * `url`       — URL completa (incluyendo query param server_id)
/// * `api_token` — Token de API
/// * `data`      — Payload a enviar
pub async fn send(url: &str, api_token: &str, data: &serde_json::Value) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .use_rustls_tls()
        .build()?;

    debug!("POST {}", url);

    let resp = client
        .post(url)
        .header("Authorization", format!("Token {}", api_token))
        .header("Content-Type", "application/json")
        .json(data)
        .send()
        .await
        .map_err(|e| anyhow!("Error de red al enviar datos: {}", e))?;

    let status = resp.status();

    if status.is_success() {
        info!("Datos enviados correctamente al servidor NESS (HTTP {})", status.as_u16());
        Ok(())
    } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        let msg = format!("Autenticación rechazada por el servidor (HTTP {})", status.as_u16());
        error!("{}", msg);
        Err(anyhow!(msg))
    } else if status.is_server_error() {
        let body = resp.text().await.unwrap_or_default();
        warn!("Error del servidor NESS (HTTP {}): {}", status.as_u16(), body);
        Err(anyhow!("Error del servidor: HTTP {}", status.as_u16()))
    } else {
        let body = resp.text().await.unwrap_or_default();
        warn!("Respuesta inesperada del servidor (HTTP {}): {}", status.as_u16(), body);
        Err(anyhow!("Respuesta inesperada: HTTP {}", status.as_u16()))
    }
}

/// Verifica la conectividad con el servidor enviando un ping simple.
/// Retorna `true` si el servidor responde correctamente.
pub async fn ping(server_url: &str, api_token: &str) -> bool {
    let ping_url = format!("{}/api/relay/ping/", server_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .use_rustls_tls()
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    match client
        .get(&ping_url)
        .header("Authorization", format!("Token {}", api_token))
        .send()
        .await
    {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    }
}
