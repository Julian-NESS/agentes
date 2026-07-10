// ==============================================================================
// NESS Relay v2.0.0 - SNMP Value Types
// ==============================================================================
// Tipos de valor SNMP normalizados para uso interno en el relay.
// Todos los valores retornados por dispositivos SNMP se mapean aquí.
// ==============================================================================

use serde::{Deserialize, Serialize};

/// Valor SNMP unificado. Representa cualquier tipo de dato que un
/// dispositivo puede retornar via SNMP (v1, v2c, v3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnmpValue {
    /// INTEGER (-2147483648..2147483647), también usado para enumeraciones SNMP.
    Integer(i64),
    /// Counter32 (0..4294967295) — contador que rebasa a 0.
    Counter32(u32),
    /// Counter64 (0..18446744073709551615) — contador 64-bit (SNMPv2+).
    Counter64(u64),
    /// Gauge32 (0..4294967295) — gauge que no rebasa (o Unsigned32).
    Gauge32(u32),
    /// TimeTicks — centésimas de segundo desde el último reinicio.
    TimeTicks(u32),
    /// OCTET STRING — cadena de texto UTF-8.
    OctetString(String),
    /// OCTET STRING binario — bytes crudos que no son UTF-8 válido.
    OctetStringRaw(Vec<u8>),
    /// OID — identificador de objeto en notación "1.3.6.1.2.1.1.1.0".
    ObjectIdentifier(String),
    /// IpAddress (4 bytes = dirección IPv4).
    IpAddress(String),
    /// Valor nulo.
    Null,
    /// El objeto no existe en el agente.
    NoSuchObject,
    /// La instancia no existe para este índice.
    NoSuchInstance,
    /// Fin del árbol MIB.
    EndOfMibView,
}

impl SnmpValue {
    /// Retorna el valor como i64 si es posible.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            SnmpValue::Integer(v) => Some(*v),
            SnmpValue::Counter32(v) => Some(*v as i64),
            SnmpValue::Counter64(v) => Some(*v as i64),
            SnmpValue::Gauge32(v) => Some(*v as i64),
            SnmpValue::TimeTicks(v) => Some(*v as i64),
            SnmpValue::OctetString(s) => s.parse::<i64>().ok(),
            _ => None,
        }
    }

    /// Retorna el valor como u64 si es posible.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            SnmpValue::Integer(v) if *v >= 0 => Some(*v as u64),
            SnmpValue::Counter32(v) => Some(*v as u64),
            SnmpValue::Counter64(v) => Some(*v),
            SnmpValue::Gauge32(v) => Some(*v as u64),
            SnmpValue::TimeTicks(v) => Some(*v as u64),
            SnmpValue::OctetString(s) => s.parse::<u64>().ok(),
            _ => None,
        }
    }

    /// Retorna el valor como f64 si es posible.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            SnmpValue::Integer(v) => Some(*v as f64),
            SnmpValue::Counter32(v) => Some(*v as f64),
            SnmpValue::Counter64(v) => Some(*v as f64),
            SnmpValue::Gauge32(v) => Some(*v as f64),
            SnmpValue::TimeTicks(v) => Some(*v as f64),
            SnmpValue::OctetString(s) => s.parse::<f64>().ok(),
            _ => None,
        }
    }

    /// Retorna el valor como String (human-readable).
    pub fn as_string(&self) -> String {
        match self {
            SnmpValue::Integer(v) => v.to_string(),
            SnmpValue::Counter32(v) => v.to_string(),
            SnmpValue::Counter64(v) => v.to_string(),
            SnmpValue::Gauge32(v) => v.to_string(),
            SnmpValue::TimeTicks(v) => v.to_string(),
            SnmpValue::OctetString(s) => s.clone(),
            SnmpValue::OctetStringRaw(b) => {
                // Intentar decodificar como UTF-8, si no mostrar como hex
                match std::str::from_utf8(b) {
                    Ok(s) => s.trim_end_matches('\0').to_string(),
                    Err(_) => format!("0x{}", hex::encode_simple(b)),
                }
            }
            SnmpValue::ObjectIdentifier(oid) => oid.clone(),
            SnmpValue::IpAddress(ip) => ip.clone(),
            SnmpValue::Null => "NULL".to_string(),
            SnmpValue::NoSuchObject => "noSuchObject".to_string(),
            SnmpValue::NoSuchInstance => "noSuchInstance".to_string(),
            SnmpValue::EndOfMibView => "endOfMibView".to_string(),
        }
    }

    /// Retorna true si el valor es un error SNMP (no data).
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            SnmpValue::NoSuchObject | SnmpValue::NoSuchInstance | SnmpValue::EndOfMibView
        )
    }

    /// Retorna true si el valor tiene datos válidos (no es null ni error).
    pub fn has_data(&self) -> bool {
        !matches!(
            self,
            SnmpValue::Null
                | SnmpValue::NoSuchObject
                | SnmpValue::NoSuchInstance
                | SnmpValue::EndOfMibView
        )
    }

    /// Convierte a serde_json::Value para serialización JSON.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            SnmpValue::Integer(v) => serde_json::Value::Number((*v).into()),
            SnmpValue::Counter32(v) | SnmpValue::Gauge32(v) => {
                serde_json::Value::Number((*v as i64).into())
            }
            SnmpValue::Counter64(v) => serde_json::Value::Number((*v as i64).into()),
            SnmpValue::TimeTicks(v) => serde_json::Value::Number((*v as i64).into()),
            SnmpValue::OctetString(s) => serde_json::Value::String(s.clone()),
            SnmpValue::OctetStringRaw(b) => {
                serde_json::Value::String(format!("0x{}", hex::encode_simple(b)))
            }
            SnmpValue::ObjectIdentifier(oid) => serde_json::Value::String(oid.clone()),
            SnmpValue::IpAddress(ip) => serde_json::Value::String(ip.clone()),
            SnmpValue::Null => serde_json::Value::Null,
            SnmpValue::NoSuchObject => {
                serde_json::Value::String("noSuchObject".to_string())
            }
            SnmpValue::NoSuchInstance => {
                serde_json::Value::String("noSuchInstance".to_string())
            }
            SnmpValue::EndOfMibView => {
                serde_json::Value::String("endOfMibView".to_string())
            }
        }
    }
}

impl std::fmt::Display for SnmpValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

// ==============================================================================
// RESULTADO DE OPERACIÓN SNMP
// ==============================================================================

/// Resultado de una operación SNMP individual (GET, GETNEXT, etc.).
#[derive(Debug, Clone)]
pub struct SnmpResult {
    pub value: Option<SnmpValue>,
    pub error: Option<String>,
    pub oid: String,
}

impl SnmpResult {
    pub fn ok(oid: String, value: SnmpValue) -> Self {
        Self {
            value: Some(value),
            error: None,
            oid,
        }
    }

    pub fn err(oid: String, error: String) -> Self {
        Self {
            value: None,
            error: Some(error),
            oid,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none() && self.value.is_some()
    }

    pub fn has_data(&self) -> bool {
        self.is_ok() && self.value.as_ref().map(|v| v.has_data()).unwrap_or(false)
    }
}

// ==============================================================================
// HEX HELPER (interno, sin dependencia externa)
// ==============================================================================

mod hex {
    pub fn encode_simple(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
