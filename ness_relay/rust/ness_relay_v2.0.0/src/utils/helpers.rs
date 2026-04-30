// ==============================================================================
// NESS Relay v2.0.0 — Helpers generales
// Equivalente Python: utils/helpers.py
// ==============================================================================

use chrono::{Local, SecondsFormat};

/// Retorna timestamp ISO 8601 con timezone local.
/// Equivalente Python: now_iso() usando datetime.now(timezone.utc).isoformat()
pub fn now_iso() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// Retorna timestamp ISO 8601 UTC.
pub fn now_iso_utc() -> String {
    use chrono::Utc;
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Imprime un mensaje garantizando codificación UTF-8 correcta.
/// Equivalente de print_simple() en Python.
pub fn print_simple(msg: &str) {
    println!("{}", msg);
}

/// Extrae un string de un serde_json::Value, retornando "" si no es string.
pub fn json_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

/// Extrae un i64 de un serde_json::Value nested, retornando 0 si no existe.
pub fn json_i64(v: &serde_json::Value, key: &str) -> i64 {
    v.get(key)
        .and_then(|x| x.as_i64())
        .unwrap_or(0)
}

/// Extrae un f64 de un serde_json::Value nested, retornando 0.0 si no existe.
pub fn json_f64(v: &serde_json::Value, key: &str) -> f64 {
    v.get(key)
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0)
}

/// Inserta un valor en un JSON object, si el JSON no es objeto no hace nada.
pub fn json_set(obj: &mut serde_json::Value, key: &str, value: serde_json::Value) {
    if let serde_json::Value::Object(ref mut map) = obj {
        map.insert(key.to_string(), value);
    }
}

/// Combina dos JSON Objects (deep merge superficial — sobreescribe claves).
pub fn json_merge(base: &mut serde_json::Value, extra: serde_json::Value) {
    if let (serde_json::Value::Object(ref mut base_map), serde_json::Value::Object(extra_map)) =
        (base, extra)
    {
        for (k, v) in extra_map {
            base_map.insert(k, v);
        }
    }
}
pub async fn get_public_ip() -> Option<String> {
    // Definimos un cliente con un tiempo de espera de 3 segundos
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build() {
            Ok(c) => c,
            Err(_) => return None,
        };

    // Consultamos api.ipify.org que devuelve la IP en texto plano
    match client.get("https://api.ipify.org").send().await {
        Ok(resp) => {
            if let Ok(text) = resp.text().await {
                return Some(text.trim().to_string());
            }
            None
        }
        Err(_) => None,
    }
}
