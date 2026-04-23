// src/utils/isp.rs
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::time::Instant;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use std::time::Duration as StdDuration;

#[derive(Clone, Serialize, Deserialize)]
pub struct IspInfo {
    pub ip: String,
    pub asn: Option<u32>,
    pub asn_org: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub isp: Option<String>,
    pub measured_kbps: Option<f64>,
}

#[derive(Deserialize)]
struct IpifyResp { ip: String }

#[derive(Deserialize)]
struct IpToAsnResp {
    as_number: Option<u32>,
    as_description: Option<String>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct IpApiResp {
    country: Option<String>,
    regionName: Option<String>,
    city: Option<String>,
    isp: Option<String>,
}

pub async fn detect_isp() -> Result<IspInfo> {
    let client = Client::builder()
        .user_agent("ness-relay/isp-detector")
        .timeout(StdDuration::from_secs(10))
        .build()?;

    // 1) IP pública
    let ip = client
        .get("https://api.ipify.org?format=json")
        .send()
        .await
        .context("falló ipify")?
        .json::<IpifyResp>()
        .await?
        .ip;

    // 2) ASN lookup
    let mut asn_num = None;
    let mut asn_o = None;
    if let Ok(resp) = client.get(&format!("https://iptoasn.com/v1/as/ip/{}", ip)).send().await {
        if let Ok(data) = resp.json::<IpToAsnResp>().await {
            asn_num = data.as_number;
            asn_o = data.as_description;
        }
    }

    // 3) Geo lookup
    let mut c = None;
    let mut r = None;
    let mut ci = None;
    let mut is = None;
    if let Ok(resp) = client.get(&format!("http://ip-api.com/json/{}", ip)).send().await {
        if let Ok(data) = resp.json::<IpApiResp>().await {
            c = data.country;
            r = data.regionName;
            ci = data.city;
            is = data.isp;
        }
    }

    // 4) Medición rápida (timeout para no bloquear mucho)
    let measured_kbps = match tokio::time::timeout(StdDuration::from_secs(4), measure_quick_download(&client)).await {
        Ok(Ok(v)) => Some(v),
        _ => None,
    };

    Ok(IspInfo {
        ip,
        asn: asn_num,
        asn_org: asn_o,
        country: c,
        region: r,
        city: ci,
        isp: is,
        measured_kbps,
    })
}

async fn measure_quick_download(client: &Client) -> Result<f64> {
    // Leer solo hasta N bytes (estimación)
    let url = "https://speed.hetzner.de/100MB.bin";
    let max_bytes: usize = 200 * 1024;
    let start = Instant::now();
    let mut resp = client.get(url).send().await?;
    let mut downloaded: usize = 0;
    while let Some(chunk) = resp.chunk().await? {
        downloaded += chunk.len();
        if downloaded >= max_bytes { break; }
    }
    let elapsed = start.elapsed().as_secs_f64();
    if elapsed <= 0.0 { return Ok(0.0); }
    Ok((downloaded as f64 * 8.0) / 1000.0 / elapsed)
}

// --------------------------
// Simple cache helpers
// --------------------------
static ISP_CACHE: OnceCell<RwLock<Option<IspInfo>>> = OnceCell::new();

pub async fn set_cached_isp(info: Option<IspInfo>) {
    let lock = ISP_CACHE.get_or_init(|| RwLock::new(None));
    let mut w = lock.write().await;
    *w = info;
}

pub async fn get_cached_isp() -> Option<IspInfo> {
    let lock = ISP_CACHE.get_or_init(|| RwLock::new(None));
    let r = lock.read().await;
    r.clone()
}

/// Compara la IP WAN reportada por SNMP con la IP pública detectada por `detect_isp()`.
/// - `snmp_wan_ip`: IP reportada por la interfaz WAN vía SNMP (pfSense OID típico).
/// - `isp`: información detectada públicamente (IP, ASN, ISP, ...).
/// Registra una advertencia si hay discrepancia (p. ej. NAT, interfaz equivocada),
/// o un info si coinciden.
pub fn compare_snmp_and_isp(snmp_wan_ip: &str, isp: &IspInfo) {
    if snmp_wan_ip != isp.ip {
        tracing::warn!(
            "Discrepancia IP WAN: SNMP reports '{}' but public IP detected '{}'. Possible NAT or incorrect interface mapping.",
            snmp_wan_ip,
            isp.ip
        );
    } else {
        tracing::info!("IP WAN validada por SNMP y detección pública: {}", snmp_wan_ip);
    }
}