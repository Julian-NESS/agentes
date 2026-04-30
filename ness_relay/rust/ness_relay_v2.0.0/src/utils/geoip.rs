// ============================================================================
// NESS Relay v2.0.0 — GeoIP helper (MaxMind GeoLite2 local lookup)
// - Busca en GeoLite2-City.mmdb y GeoLite2-ASN.mmdb ubicados en
//   /opt/ness_relay/data/ por defecto.
// - Retorna JSON simplificado con city, country, lat/lon y ASN info.
// ============================================================================
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Mutex;
use std::net::IpAddr;
use tracing::{info, error, debug};
use maxminddb::Reader;

static CITY_READER: Lazy<Mutex<Option<Reader<Vec<u8>>>>> = Lazy::new(|| Mutex::new(None));
static ASN_READER: Lazy<Mutex<Option<Reader<Vec<u8>>>>> = Lazy::new(|| Mutex::new(None));

fn default_db_dir() -> PathBuf {
    PathBuf::from("/opt/ness_relay/data")
}

fn try_init_readers() {
    let base = default_db_dir();
    // City DB
    let mut city_lock = CITY_READER.lock().unwrap();
    if city_lock.is_none() {
        let city_path = base.join("GeoLite2-City.mmdb");
        match Reader::open_readfile(&city_path) {
            Ok(r) => { *city_lock = Some(r); info!("GeoIP: City DB cargada."); }
            Err(e) => error!("GeoIP Error City DB: {}", e),
        }
    }
    // ASN DB
    let mut asn_lock = ASN_READER.lock().unwrap();
    if asn_lock.is_none() {
        let asn_path = base.join("GeoLite2-ASN.mmdb");
        match Reader::open_readfile(&asn_path) {
            Ok(r) => { *asn_lock = Some(r); info!("GeoIP: ASN DB cargada."); }
            Err(e) => error!("GeoIP Error ASN DB: {}", e),
        }
    }
}

pub fn lookup_city(ip: &str) -> Option<serde_json::Value> {
    try_init_readers();
    let guard = CITY_READER.lock().unwrap();
    let reader = guard.as_ref()?;
    let ip_addr: IpAddr = ip.parse().ok()?;

    if let Ok(city_rec) = reader.lookup::<maxminddb::geoip2::City>(ip_addr) {
        let city = city_rec.city.as_ref().and_then(|c| c.names.as_ref())
            .and_then(|m| m.get("es").or_else(|| m.get("en"))).map(|s| s.to_string());
        let region = city_rec.subdivisions.as_ref().and_then(|s| s.first())
            .and_then(|sub| sub.names.as_ref()).and_then(|m| m.get("es").or_else(|| m.get("en"))).map(|s| s.to_string());
        let country = city_rec.country.as_ref().and_then(|c| c.names.as_ref())
            .and_then(|m| m.get("es").or_else(|| m.get("en"))).map(|s| s.to_string());

        return Some(serde_json::json!({
            "city": city.unwrap_or_else(|| "Unknown".to_string()),
            "region": region.unwrap_or_else(|| "Unknown".to_string()),
            "country": country.unwrap_or_else(|| "Colombia".to_string()),
        }));
    }
    None
}

pub fn lookup_asn(ip: &str) -> Option<serde_json::Value> {
    try_init_readers();
    let guard = ASN_READER.lock().unwrap();
    let reader = guard.as_ref()?;
    let ip_addr: IpAddr = ip.parse().ok()?;

    if let Ok(asn_rec) = reader.lookup::<maxminddb::geoip2::Asn>(ip_addr) {
        let org = asn_rec.autonomous_system_organization.map(|s| s.to_string());
        return Some(serde_json::json!({
            "organization": org.unwrap_or_else(|| "Unknown".to_string()),
        }));
    }
    None
}