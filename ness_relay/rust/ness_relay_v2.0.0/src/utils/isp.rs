use anyhow::{Context, Result, anyhow}; // ESTO ES LO QUE TE FALTABA
use maxminddb::Reader;
use serde::{Serialize, Deserialize};
use std::net::IpAddr;
use std::path::Path;
use once_cell::sync::OnceCell;
use tokio::task;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
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

static ASN_READER: OnceCell<Reader<Vec<u8>>> = OnceCell::new();
static CITY_READER: OnceCell<Reader<Vec<u8>>> = OnceCell::new();
static ISP_CACHE: OnceCell<Arc<RwLock<Option<IspInfo>>>> = OnceCell::new();

fn get_cache() -> Arc<RwLock<Option<IspInfo>>> {
    ISP_CACHE.get_or_init(|| Arc::new(RwLock::new(None))).clone()
}

pub async fn get_cached_isp() -> Option<IspInfo> {
    let cache = get_cache();
    let read = cache.read().await;
    read.clone()
}

pub async fn set_cached_isp(info: Option<IspInfo>) {
    let cache = get_cache();
    let mut write = cache.write().await;
    *write = info;
}

pub fn compare_snmp_and_isp(snmp_ip: &str, info: &IspInfo) {
    println!("Comparando SNMP IP: {} con ISP IP: {}", snmp_ip, info.ip);
}

pub async fn detect_isp() -> Result<IspInfo> {
    detect_isp_mmdb("8.8.8.8", "/opt/ness_relay/db").await
}

pub async fn detect_isp_mmdb(ip_str: &str, db_dir: &str) -> Result<IspInfo> {
    let ip_parsed: IpAddr = ip_str.parse().context("IP inválida")?;
    let db_dir_owned = db_dir.to_string();
    let ip_string = ip_str.to_string();

    let handle = task::spawn_blocking(move || -> Result<IspInfo> {
        let asn_path = Path::new(&db_dir_owned).join("GeoLite2-ASN.mmdb");
        let city_path = Path::new(&db_dir_owned).join("GeoLite2-City.mmdb");

        let asn_reader = ASN_READER.get_or_try_init(|| {
            Reader::open_readfile(&asn_path).map_err(|e| anyhow!("Error ASN DB: {}", e))
        })?;

        let city_reader = CITY_READER.get_or_try_init(|| {
            Reader::open_readfile(&city_path).map_err(|e| anyhow!("Error City DB: {}", e))
        })?;

        // Bloque para el ASN
        let asn_res: maxminddb::geoip2::Asn = asn_reader
            .lookup(ip_parsed)
            .map_err(|e| anyhow!("Error ASN lookup: {}", e))?;

        // Bloque para la Ciudad
        let city_res: maxminddb::geoip2::City = city_reader
            .lookup(ip_parsed)
            .map_err(|e| anyhow!("Error City lookup: {}", e))?;

       // --- REEMPLAZA DESDE AQUÍ ---
        let country = city_res.country
            .and_then(|c| c.names)
            .and_then(|n| n.get("es").or(n.get("en")).map(|s| s.to_string()));

        let city_name = city_res.city
            .and_then(|c| c.names)
            .and_then(|n| n.get("es").or(n.get("en")).map(|s| s.to_string()));

        let region = city_res.subdivisions
            .and_then(|s| s.get(0).cloned())
            .and_then(|sub| sub.names)
            .and_then(|n| n.get("es").or(n.get("en")).map(|s| s.to_string()));

        let asn_org = asn_res.autonomous_system_organization.map(|s| s.to_string());

        Ok(IspInfo {
            ip: ip_string,
            asn: asn_res.autonomous_system_number,
            asn_org: asn_org.clone(),
            country,
            region,
            city: city_name,
            isp: asn_org,
            measured_kbps: Some(0.0),
        })
    }); 

    let info = handle.await.map_err(|e| anyhow!("Join error: {}", e))??;
    Ok(info)
} 