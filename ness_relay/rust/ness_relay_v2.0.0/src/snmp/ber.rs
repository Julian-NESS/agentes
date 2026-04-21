// ==============================================================================
// NESS Relay v2.0.0 - SNMP BER Encoder/Decoder
// ==============================================================================
// Implementación propia de BER (Basic Encoding Rules) para SNMP.
// Soporta todos los tipos de valor que los dispositivos de red retornan.
// 100% Rust puro — sin dependencias C, sin OpenSSL.
// ==============================================================================

use crate::snmp::types::SnmpValue;

// ==============================================================================
// BER TAG CONSTANTS
// ==============================================================================

pub const TAG_INTEGER: u8 = 0x02;
pub const TAG_OCTET_STRING: u8 = 0x04;
pub const TAG_NULL: u8 = 0x05;
pub const TAG_OID: u8 = 0x06;
pub const TAG_SEQUENCE: u8 = 0x30;

// Application tags (SNMP-specific)
pub const TAG_IP_ADDRESS: u8 = 0x40;  // Application 0
pub const TAG_COUNTER32: u8 = 0x41;   // Application 1
pub const TAG_GAUGE32: u8 = 0x42;     // Application 2 (Unsigned32, Gauge32)
pub const TAG_TIMETICKS: u8 = 0x43;   // Application 3
pub const TAG_OPAQUE: u8 = 0x44;      // Application 4
pub const TAG_COUNTER64: u8 = 0x46;   // Application 6

// Context tags (SNMP PDU types)
pub const TAG_GET_REQUEST: u8 = 0xa0;
pub const TAG_GETNEXT_REQUEST: u8 = 0xa1;
pub const TAG_GET_RESPONSE: u8 = 0xa2;
pub const TAG_SET_REQUEST: u8 = 0xa3;
pub const TAG_GETBULK_REQUEST: u8 = 0xa5;
pub const TAG_REPORT: u8 = 0xa8;

// Exception values (SNMPv2)
pub const TAG_NO_SUCH_OBJECT: u8 = 0x80;
pub const TAG_NO_SUCH_INSTANCE: u8 = 0x81;
pub const TAG_END_OF_MIB_VIEW: u8 = 0x82;

// SNMPv3 context wrapper
pub const TAG_PLAIN_TEXT: u8 = 0xa0;  // msgData plaintext
pub const TAG_ENCRYPTED: u8 = 0xa1;   // msgData encrypted

// ==============================================================================
// BER LENGTH ENCODING
// ==============================================================================

/// Codifica la longitud en formato BER.
pub fn encode_length(len: usize) -> Vec<u8> {
    if len < 0x80 {
        vec![len as u8]
    } else if len < 0x100 {
        vec![0x81, len as u8]
    } else if len < 0x10000 {
        vec![0x82, (len >> 8) as u8, (len & 0xff) as u8]
    } else {
        vec![
            0x83,
            (len >> 16) as u8,
            ((len >> 8) & 0xff) as u8,
            (len & 0xff) as u8,
        ]
    }
}

/// Decodifica la longitud BER desde un slice, retorna (longitud, bytes_consumidos).
pub fn decode_length(data: &[u8]) -> Option<(usize, usize)> {
    if data.is_empty() {
        return None;
    }
    let first = data[0];
    if first < 0x80 {
        Some((first as usize, 1))
    } else {
        let len_bytes = (first & 0x7f) as usize;
        if data.len() < 1 + len_bytes {
            return None;
        }
        let mut len: usize = 0;
        for i in 1..=len_bytes {
            len = (len << 8) | (data[i] as usize);
        }
        Some((len, 1 + len_bytes))
    }
}

// ==============================================================================
// TLV ENCODING HELPERS
// ==============================================================================

/// Construye un TLV (Tag-Length-Value) BER.
pub fn tlv(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(1 + 4 + value.len());
    result.push(tag);
    result.extend(encode_length(value.len()));
    result.extend_from_slice(value);
    result
}

/// Construye un SEQUENCE (0x30) con los valores concatenados.
pub fn sequence(contents: &[u8]) -> Vec<u8> {
    tlv(TAG_SEQUENCE, contents)
}

// ==============================================================================
// INTEGER ENCODING
// ==============================================================================

/// Codifica un entero con signo en BER (two's complement, big-endian).
pub fn encode_integer(value: i64) -> Vec<u8> {
    let be_bytes = value.to_be_bytes(); // 8 bytes big-endian two's complement
    let mut start = 0;
    if value >= 0 {
        // Saltar bytes 0x00 redundantes, pero mantener uno si es necesario
        // para que el bit de signo del primer byte sea 0 (positivo).
        while start < 7 && be_bytes[start] == 0x00 && be_bytes[start + 1] & 0x80 == 0 {
            start += 1;
        }
    } else {
        // Saltar bytes 0xFF redundantes, pero mantener uno si es necesario
        // para que el bit de signo del primer byte sea 1 (negativo).
        while start < 7 && be_bytes[start] == 0xFF && be_bytes[start + 1] & 0x80 != 0 {
            start += 1;
        }
    }
    tlv(TAG_INTEGER, &be_bytes[start..])
}

/// Decodifica un entero con signo BER.
pub fn decode_integer(data: &[u8]) -> Option<i64> {
    if data.is_empty() {
        return Some(0);
    }
    let negative = data[0] & 0x80 != 0;
    let mut value: i64 = if negative { -1 } else { 0 };
    for &b in data {
        value = (value << 8) | (b as i64);
    }
    Some(value)
}

/// Codifica un entero sin signo en BER.
pub fn encode_uint(value: u64) -> Vec<u8> {
    if value == 0 {
        return tlv(TAG_INTEGER, &[0x00]);
    }
    let mut bytes = Vec::new();
    let mut n = value;
    while n > 0 {
        bytes.push((n & 0xff) as u8);
        n >>= 8;
    }
    // Añadir byte 0x00 si el bit más significativo está en 1 (evitar interpretación negativa)
    if bytes.last().map(|&b| b >= 0x80).unwrap_or(false) {
        bytes.push(0x00);
    }
    bytes.reverse();
    tlv(TAG_INTEGER, &bytes)
}

pub fn decode_uint(data: &[u8]) -> u64 {
    let mut value: u64 = 0;
    for &b in data {
        value = (value << 8) | (b as u64);
    }
    value
}

// ==============================================================================
// OID ENCODING
// ==============================================================================

/// Parsea un OID string como "1.3.6.1.2.1.1.1.0" a Vec<u64>.
pub fn oid_string_to_components(oid: &str) -> Option<Vec<u64>> {
    let parts: Result<Vec<u64>, _> = oid
        .trim_start_matches('.')
        .split('.')
        .map(|s| s.parse::<u64>())
        .collect();
    parts.ok()
}

/// Codifica un OID a bytes BER.
pub fn encode_oid(oid: &str) -> Vec<u8> {
    let components = match oid_string_to_components(oid) {
        Some(c) if c.len() >= 2 => c,
        _ => return tlv(TAG_OID, &[]),
    };

    let mut bytes = Vec::new();
    // Primeros dos componentes: 40*x + y
    bytes.push((components[0] * 40 + components[1]) as u8);

    // Componentes restantes: codificación base-128
    for &component in &components[2..] {
        encode_oid_component(component, &mut bytes);
    }
    tlv(TAG_OID, &bytes)
}

fn encode_oid_component(mut value: u64, output: &mut Vec<u8>) {
    if value == 0 {
        output.push(0x00);
        return;
    }
    let mut tmp = Vec::new();
    while value > 0 {
        tmp.push((value & 0x7f) as u8);
        value >>= 7;
    }
    tmp.reverse();
    for (i, &b) in tmp.iter().enumerate() {
        if i < tmp.len() - 1 {
            output.push(b | 0x80);
        } else {
            output.push(b);
        }
    }
}

/// Decodifica bytes BER de OID a string "1.3.6.1...".
pub fn decode_oid(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    let first = data[0] as u64;
    let mut components = vec![first / 40, first % 40];
    let mut i = 1;
    while i < data.len() {
        let mut value: u64 = 0;
        loop {
            if i >= data.len() {
                break;
            }
            let b = data[i];
            i += 1;
            value = (value << 7) | ((b & 0x7f) as u64);
            if b & 0x80 == 0 {
                break;
            }
        }
        components.push(value);
    }
    components
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(".")
}

// ==============================================================================
// OCTET STRING ENCODING
// ==============================================================================

pub fn encode_octet_string(data: &[u8]) -> Vec<u8> {
    tlv(TAG_OCTET_STRING, data)
}

pub fn encode_null() -> Vec<u8> {
    vec![TAG_NULL, 0x00]
}

// ==============================================================================
// BER PARSER — Decodifica datos BER a SnmpValue
// ==============================================================================

/// Parsea un solo TLV desde un slice de datos.
/// Retorna (valor, bytes_consumidos) o None si hay error.
pub fn parse_tlv(data: &[u8]) -> Option<(u8, &[u8], usize)> {
    if data.len() < 2 {
        return None;
    }
    let tag = data[0];
    let (len, len_bytes) = decode_length(&data[1..])?;
    let offset = 1 + len_bytes;
    if data.len() < offset + len {
        return None;
    }
    Some((tag, &data[offset..offset + len], offset + len))
}

/// Parsea un valor BER y retorna el SnmpValue correspondiente.
pub fn parse_value(tag: u8, data: &[u8]) -> SnmpValue {
    match tag {
        TAG_INTEGER => {
            let v = decode_integer(data).unwrap_or(0);
            SnmpValue::Integer(v)
        }
        TAG_OCTET_STRING => {
            // Intentar UTF-8, si falla dejar como hex string
            match std::str::from_utf8(data) {
                Ok(s) => SnmpValue::OctetString(s.trim_end_matches('\0').to_string()),
                Err(_) => {
                    // Datos binarios: detectar IpAddress por longitud 4
                    if data.len() == 4 {
                        SnmpValue::IpAddress(format!(
                            "{}.{}.{}.{}",
                            data[0], data[1], data[2], data[3]
                        ))
                    } else {
                        SnmpValue::OctetStringRaw(data.to_vec())
                    }
                }
            }
        }
        TAG_NULL => SnmpValue::Null,
        TAG_OID => SnmpValue::ObjectIdentifier(decode_oid(data)),
        TAG_IP_ADDRESS => {
            if data.len() == 4 {
                SnmpValue::IpAddress(format!(
                    "{}.{}.{}.{}",
                    data[0], data[1], data[2], data[3]
                ))
            } else {
                SnmpValue::OctetString(format!("{:?}", data))
            }
        }
        TAG_COUNTER32 => SnmpValue::Counter32(decode_uint(data) as u32),
        TAG_GAUGE32 => SnmpValue::Gauge32(decode_uint(data) as u32),
        TAG_TIMETICKS => SnmpValue::TimeTicks(decode_uint(data) as u32),
        TAG_COUNTER64 => SnmpValue::Counter64(decode_uint(data)),
        TAG_OPAQUE => SnmpValue::OctetStringRaw(data.to_vec()),
        TAG_NO_SUCH_OBJECT => SnmpValue::NoSuchObject,
        TAG_NO_SUCH_INSTANCE => SnmpValue::NoSuchInstance,
        TAG_END_OF_MIB_VIEW => SnmpValue::EndOfMibView,
        _ => SnmpValue::OctetStringRaw(data.to_vec()),
    }
}

/// Parsea una lista de VarBinds del payload del PDU.
/// Retorna Vec<(oid_string, SnmpValue)>.
pub fn parse_varbind_list(data: &[u8]) -> Vec<(String, SnmpValue)> {
    let mut results = Vec::new();
    let mut pos = 0;

    // La lista es un SEQUENCE de SEQUENCE(VarBind)
    let inner = if let Some((TAG_SEQUENCE, inner, consumed)) = parse_tlv(data) {
        pos += consumed;
        inner
    } else {
        return results;
    };

    let mut offset = 0;
    while offset < inner.len() {
        // Cada VarBind es un SEQUENCE { OID, Value }
        if let Some((TAG_SEQUENCE, varbind_data, consumed)) = parse_tlv(&inner[offset..]) {
            offset += consumed;
            let mut vb_offset = 0;

            // Parse OID
            let oid = if let Some((TAG_OID, oid_bytes, oid_consumed)) =
                parse_tlv(&varbind_data[vb_offset..])
            {
                vb_offset += oid_consumed;
                decode_oid(oid_bytes)
            } else {
                break;
            };

            // Parse Value
            let value = if let Some((val_tag, val_data, val_consumed)) =
                parse_tlv(&varbind_data[vb_offset..])
            {
                vb_offset += val_consumed;
                parse_value(val_tag, val_data)
            } else {
                SnmpValue::Null
            };

            let _ = vb_offset; // suppress unused variable warning
            results.push((oid, value));
        } else {
            break;
        }
    }
    let _ = pos;
    results
}

// ==============================================================================
// PDU PARSING
// ==============================================================================

#[derive(Debug)]
pub struct SnmpPdu {
    pub request_id: i32,
    pub error_status: i32,
    pub error_index: i32,
    pub varbinds: Vec<(String, SnmpValue)>,
}

/// Parsea el PDU de respuesta SNMP v1/v2c.
pub fn parse_response_pdu(data: &[u8]) -> Option<SnmpPdu> {
    // data es el contenido del PDU (sin el tag del outer PDU)
    let mut offset = 0;

    // request-id (INTEGER)
    let request_id = if let Some((TAG_INTEGER, id_data, consumed)) = parse_tlv(&data[offset..]) {
        offset += consumed;
        decode_integer(id_data).unwrap_or(0) as i32
    } else {
        return None;
    };

    // error-status (INTEGER)
    let error_status =
        if let Some((TAG_INTEGER, es_data, consumed)) = parse_tlv(&data[offset..]) {
            offset += consumed;
            decode_integer(es_data).unwrap_or(0) as i32
        } else {
            return None;
        };

    // error-index (INTEGER)
    let error_index = if let Some((TAG_INTEGER, ei_data, consumed)) = parse_tlv(&data[offset..]) {
        offset += consumed;
        decode_integer(ei_data).unwrap_or(0) as i32
    } else {
        return None;
    };

    // VarBindList (SEQUENCE OF VarBind)
    let varbinds = parse_varbind_list(&data[offset..]);

    Some(SnmpPdu {
        request_id,
        error_status,
        error_index,
        varbinds,
    })
}
