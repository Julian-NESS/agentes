// ==============================================================================
// NESS Relay v2.0.0 - SNMP Client (Async, pure Rust)
// ==============================================================================
// Cliente SNMP totalmente asíncrono (tokio UDP). Soporta v1, v2c y v3.
// Implementación propia usando BER encoding y RustCrypto para v3 USM.
// Sin dependencias C, sin OpenSSL — binario estático limpio.
//
// Operaciones soportadas:
//   - GET
//   - GETNEXT
//   - GETBULK (v2c/v3)
//   - WALK (GETNEXT iterado, fallback v1)
//
// SNMPv3 USM:
//   - Autenticación: HMAC-MD5-96, HMAC-SHA-96, HMAC-SHA-256
//   - Privacidad:    DES-CBC, AES-128-CFB
//   - Engine discovery automático
// ==============================================================================

pub mod ber;
pub mod types;
pub mod v3;

use anyhow::{anyhow, Result};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use tracing::{debug, warn};

use self::ber::*;
use self::types::{SnmpResult, SnmpValue};
use self::v3::{AuthProtocol, PrivProtocol, UsmSecurityParams};

// Contador global de request IDs (thread-safe)
static REQUEST_ID_COUNTER: AtomicI32 = AtomicI32::new(1);

fn next_request_id() -> i32 {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

// ==============================================================================
// CONFIGURACIÓN DEL CLIENTE
// ==============================================================================

/// Versión SNMP a usar por el cliente.
#[derive(Debug, Clone, PartialEq)]
pub enum SnmpVersion {
    V1,
    V2c,
    V3,
}

impl SnmpVersion {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "1" | "v1" | "snmpv1" => Self::V1,
            "3" | "v3" | "snmpv3" => Self::V3,
            _ => Self::V2c, // Default: v2c
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Self::V1 => "1",
            Self::V2c => "2c",
            Self::V3 => "3",
        }
    }
}

// ==============================================================================
// SNMP CLIENT
// ==============================================================================

/// Cliente SNMP encapsulado por dispositivo.
/// Soporta SNMPv1, SNMPv2c y SNMPv3.
pub struct SnmpClient {
    pub host: String,
    pub port: u16,
    pub version: SnmpVersion,
    pub vendor: String,
    pub description: String,

    // v1/v2c
    pub community: String,

    // v3
    pub v3_user: String,
    pub v3_auth_protocol: AuthProtocol,
    pub v3_auth_password: String,
    pub v3_priv_protocol: PrivProtocol,
    pub v3_priv_password: String,

    // Estado v3 (descubierto durante la sesión)
    pub engine_id: Vec<u8>,
    pub engine_boots: u32,
    pub engine_time: u32,

    // Configuración de red
    timeout_secs: u64,
    retries: u32,
}

impl Clone for SnmpClient {
    fn clone(&self) -> Self {
        Self {
            host: self.host.clone(),
            port: self.port,
            version: self.version.clone(),
            vendor: self.vendor.clone(),
            description: self.description.clone(),
            community: self.community.clone(),
            v3_user: self.v3_user.clone(),
            v3_auth_protocol: self.v3_auth_protocol.clone(),
            v3_auth_password: self.v3_auth_password.clone(),
            v3_priv_protocol: self.v3_priv_protocol.clone(),
            v3_priv_password: self.v3_priv_password.clone(),
            engine_id: self.engine_id.clone(),
            engine_boots: self.engine_boots,
            engine_time: self.engine_time,
            timeout_secs: self.timeout_secs,
            retries: self.retries,
        }
    }
}

/// Incrementa un OID lexicográficamente (suma 1 al último componente).
/// Usado por `bulk_walk` para continuar el walk después de cada página.
fn increment_oid_lex(oid: &str) -> String {
    let parts: Vec<&str> = oid.split('.').collect();
    if parts.is_empty() {
        return oid.to_string();
    }
    let last_idx = parts.len() - 1;
    let last_num: u64 = parts[last_idx].parse().unwrap_or(0);
    let mut new_parts: Vec<String> = parts[..last_idx]
        .iter()
        .map(|s| s.to_string())
        .collect();
    new_parts.push((last_num + 1).to_string());
    new_parts.join(".")
}

impl SnmpClient {
    /// Crea un nuevo SnmpClient desde la configuración del dispositivo.
    pub async fn new(device_config: &serde_json::Value) -> Result<Self> {
        let host = device_config["ip"]
            .as_str()
            .unwrap_or("127.0.0.1")
            .to_string();
        let port = device_config["port"].as_u64().unwrap_or(161) as u16;
        let version_str = device_config["snmp_version"].as_str().unwrap_or("2c");
        let version = SnmpVersion::from_str(version_str);
        let community = device_config["community"]
            .as_str()
            .unwrap_or("public")
            .to_string();
        let vendor = device_config["vendor"].as_str().unwrap_or("generic").to_string();
        let description = device_config["description"]
            .as_str()
            .unwrap_or(&host)
            .to_string();

        let v3_user = device_config["v3_user"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let v3_auth_protocol = AuthProtocol::from_str(
            device_config["v3_auth_protocol"].as_str().unwrap_or("SHA"),
        );
        let v3_auth_password = device_config["v3_auth_password"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let v3_priv_protocol = PrivProtocol::from_str(
            device_config["v3_priv_protocol"].as_str().unwrap_or("AES128"),
        );
        let v3_priv_password = device_config["v3_priv_password"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let mut client = Self {
            host,
            port,
            version,
            vendor,
            description,
            community,
            v3_user,
            v3_auth_protocol,
            v3_auth_password,
            v3_priv_protocol,
            v3_priv_password,
            engine_id: Vec::new(),
            engine_boots: 0,
            engine_time: 0,
            timeout_secs: 5,
            retries: 2,
        };

        // Para SNMPv3, hacer el discovery del engine automáticamente
        if client.version == SnmpVersion::V3 {
            if let Err(e) = client.discover_engine().await {
                warn!("SNMPv3 engine discovery falló: {}. Se usará engine ID vacío.", e);
            }
        }

        Ok(client)
    }

    /// Retorna info de conexión para logging.
    pub fn connection_info(&self) -> serde_json::Value {
        let mut info = serde_json::json!({
            "host": self.host,
            "port": self.port,
            "snmp_version": self.version.as_str(),
            "vendor": self.vendor,
        });
        if self.version == SnmpVersion::V3 {
            info["v3_user"] = serde_json::Value::String(self.v3_user.clone());
            info["v3_auth"] = serde_json::Value::String(format!("{:?}", self.v3_auth_protocol));
            info["v3_priv"] = serde_json::Value::String(format!("{:?}", self.v3_priv_protocol));
        } else {
            info["community"] = serde_json::Value::String(self.community.clone());
        }
        info
    }

    // ===========================================================================
    // ENGINE DISCOVERY (SNMPv3)
    // ===========================================================================

    /// Descubre el engineID, engineBoots y engineTime del agente SNMPv3.
    /// RFC 3414 §4 — Discovery: enviar request vacío, parsear el Report.
    async fn discover_engine(&mut self) -> Result<()> {
        debug!("SNMPv3 engine discovery para {}:{}", self.host, self.port);

        // Construir mensaje de discovery (sin auth/priv, engineID vacío)
        let request_id = next_request_id();
        let discovery_msg = self.build_v3_discovery_message(request_id);

        let response = self.send_udp(&discovery_msg).await?;
        let (engine_id, boots, time) = self.parse_v3_engine_from_report(&response)?;

        self.engine_id = engine_id;
        self.engine_boots = boots;
        self.engine_time = time;

        debug!(
            "Engine descubierto: ID={}, boots={}, time={}",
            hex_string(&self.engine_id),
            self.engine_boots,
            self.engine_time
        );
        Ok(())
    }

    /// Construye mensaje de descubrimiento SNMPv3 (sin seguridad).
    fn build_v3_discovery_message(&self, request_id: i32) -> Vec<u8> {
        // GetRequest vacío sin autenticación para discovery
        let varbind_null = sequence(&[
            encode_oid("1.3.6.1.2.1.1.1.0").as_slice(), // sysDescr
            encode_null().as_slice(),
        ].concat());
        let varbind_list = sequence(&varbind_null);

        let pdu_content = [
            encode_integer(request_id as i64).as_slice(),
            encode_integer(0).as_slice(), // error-status
            encode_integer(0).as_slice(), // error-index
            varbind_list.as_slice(),
        ]
        .concat();
        let pdu = tlv(TAG_GET_REQUEST, &pdu_content);

        // USM Security Parameters vacíos (para discovery: username vacío)
        let usm_params = self.encode_usm_params(&[], 0, 0, &[], &[], &[]);
        let usm_encoded = encode_octet_string(&usm_params);

        // msgGlobalData
        let msg_id = next_request_id();
        // RFC 3414 §4: discovery usa reportable flag (bit 2 = 0x04), noAuth noPriv
        let msg_flags = encode_octet_string(&[0x04]); // reportable, noAuth, noPriv
        let msg_security_model = encode_integer(3); // USM
        let msg_max_size = encode_integer(65535);
        let global_data = sequence(&[
            encode_integer(msg_id as i64).as_slice(),
            msg_max_size.as_slice(),
            msg_flags.as_slice(),
            msg_security_model.as_slice(),
        ]
        .concat());

        // Scoped PDU (context engine ID vacío, context name vacío)
        let scoped_pdu = sequence(&[
            encode_octet_string(&[]).as_slice(), // contextEngineID
            encode_octet_string(b"").as_slice(), // contextName
            pdu.as_slice(),
        ]
        .concat());

        // Mensaje SNMPv3 completo
        sequence(&[
            encode_integer(3).as_slice(),      // msgVersion
            global_data.as_slice(),
            usm_encoded.as_slice(),
            scoped_pdu.as_slice(),
        ]
        .concat())
    }

    /// Parsea el Report del agente para extraer engineID, boots, time.
    fn parse_v3_engine_from_report(
        &self,
        data: &[u8],
    ) -> Result<(Vec<u8>, u32, u32)> {
        // Parsear el mensaje SNMPv3 completo
        let (tag, msg_data, _) = parse_tlv(data)
            .ok_or_else(|| anyhow!("Report inválido: no es un TLV válido"))?;
        if tag != TAG_SEQUENCE {
            return Err(anyhow!("Report no es un SEQUENCE"));
        }

        // Encontrar los parámetros USM (3er elemento del SEQUENCE msgV3)
        let mut offset = 0;
        // Saltar msgVersion
        if let Some((_, _, consumed)) = parse_tlv(&msg_data[offset..]) {
            offset += consumed;
        }
        // Saltar msgGlobalData
        if let Some((_, _, consumed)) = parse_tlv(&msg_data[offset..]) {
            offset += consumed;
        }
        // msgSecurityParameters (OCTET STRING con los params USM encodados)
        let usm_bytes = if let Some((TAG_OCTET_STRING, usm_data, consumed)) =
            parse_tlv(&msg_data[offset..])
        {
            offset += consumed;
            usm_data.to_vec()
        } else {
            return Err(anyhow!("No se encontraron params USM en el Report"));
        };

        // Decodificar el SEQUENCE interno de USM params
        let usm_seq_data = if let Some((TAG_SEQUENCE, inner, _)) = parse_tlv(&usm_bytes) {
            inner.to_vec()
        } else {
            usm_bytes.clone()
        };

        let mut usm_offset = 0;
        // msgAuthoritativeEngineID (OCTET STRING)
        let engine_id = if let Some((TAG_OCTET_STRING, eid, consumed)) =
            parse_tlv(&usm_seq_data[usm_offset..])
        {
            usm_offset += consumed;
            eid.to_vec()
        } else {
            return Err(anyhow!("No se encontró engineID en USM params"));
        };

        // msgAuthoritativeEngineBoots (INTEGER)
        let boots = if let Some((TAG_INTEGER, boots_data, consumed)) =
            parse_tlv(&usm_seq_data[usm_offset..])
        {
            usm_offset += consumed;
            decode_integer(boots_data).unwrap_or(0) as u32
        } else {
            0
        };

        // msgAuthoritativeEngineTime (INTEGER)
        let time = if let Some((TAG_INTEGER, time_data, consumed)) =
            parse_tlv(&usm_seq_data[usm_offset..])
        {
            decode_integer(time_data).unwrap_or(0) as u32
        } else {
            0
        };

        Ok((engine_id, boots, time))
    }

    // ===========================================================================
    // CONSTRUCCIÓN DE MENSAJES SNMP
    // ===========================================================================

    /// Construye un mensaje SNMP v1/v2c GET o GETNEXT.
    fn build_v1v2c_get(&self, oid: &str, pdu_tag: u8) -> Vec<u8> {
        let request_id = next_request_id();
        let varbind = sequence(&[encode_oid(oid).as_slice(), encode_null().as_slice()].concat());
        let varbind_list = sequence(&varbind);

        let pdu_content = [
            encode_integer(request_id as i64).as_slice(),
            encode_integer(0).as_slice(), // error-status: noError
            encode_integer(0).as_slice(), // error-index: 0
            varbind_list.as_slice(),
        ]
        .concat();
        let pdu = tlv(pdu_tag, &pdu_content);

        let version = match self.version {
            SnmpVersion::V1 => 0i64,
            _ => 1i64,
        };
        sequence(&[
            encode_integer(version).as_slice(),
            encode_octet_string(self.community.as_bytes()).as_slice(),
            pdu.as_slice(),
        ]
        .concat())
    }

    /// Construye un mensaje SNMP v2c/v3 GETBULK.
    fn build_v2c_getbulk(&self, oid: &str, non_repeaters: u32, max_repetitions: u32) -> Vec<u8> {
        let request_id = next_request_id();
        let varbind = sequence(&[encode_oid(oid).as_slice(), encode_null().as_slice()].concat());
        let varbind_list = sequence(&varbind);

        let pdu_content = [
            encode_integer(request_id as i64).as_slice(),
            encode_integer(non_repeaters as i64).as_slice(), // non-repeaters
            encode_integer(max_repetitions as i64).as_slice(), // max-repetitions
            varbind_list.as_slice(),
        ]
        .concat();
        let pdu = tlv(TAG_GETBULK_REQUEST, &pdu_content);

        sequence(&[
            encode_integer(1).as_slice(), // version = v2c
            encode_octet_string(self.community.as_bytes()).as_slice(),
            pdu.as_slice(),
        ]
        .concat())
    }

    /// Construye un mensaje SNMPv3 authenticado y/o cifrado.
    fn build_v3_message(
        &self,
        oid: &str,
        pdu_tag: u8,
        non_repeaters: u32,
        max_repetitions: u32,
    ) -> Vec<u8> {
        let request_id = next_request_id();
        let varbind = sequence(&[encode_oid(oid).as_slice(), encode_null().as_slice()].concat());
        let varbind_list = sequence(&varbind);

        let pdu_content = if pdu_tag == TAG_GETBULK_REQUEST {
            [
                encode_integer(request_id as i64).as_slice(),
                encode_integer(non_repeaters as i64).as_slice(),
                encode_integer(max_repetitions as i64).as_slice(),
                varbind_list.as_slice(),
            ]
            .concat()
        } else {
            [
                encode_integer(request_id as i64).as_slice(),
                encode_integer(0).as_slice(),
                encode_integer(0).as_slice(),
                varbind_list.as_slice(),
            ]
            .concat()
        };
        let pdu = tlv(pdu_tag, &pdu_content);

        // Flags de seguridad (bit 0 = auth, bit 1 = priv, bit 2 = reportable).
        // Importante: auth/priv solo se activan cuando hay credenciales efectivas.
        let has_auth = self.v3_auth_protocol != AuthProtocol::None
            && !self.v3_user.is_empty()
            && !self.v3_auth_password.is_empty();
        let has_priv = has_auth
            && self.v3_priv_protocol != PrivProtocol::None
            && !self.v3_priv_password.is_empty();

        let auth_flag = if has_auth { 0x01 } else { 0x00 };
        let priv_flag = if has_priv { 0x02 } else { 0x00 };
        let reportable_flag = 0x04u8; // Siempre reportable en requests
        let flags = [auth_flag | priv_flag | reportable_flag];

        let msg_id = next_request_id();
        let global_data = sequence(&[
            encode_integer(msg_id as i64).as_slice(),
            encode_integer(65535).as_slice(), // msgMaxSize
            encode_octet_string(&flags).as_slice(),
            encode_integer(3).as_slice(), // USM
        ]
        .concat());

        // Derivar claves si hay autenticación
        let usm_sec = if has_auth {
            Some(UsmSecurityParams::new(
                &self.v3_user,
                self.v3_auth_protocol.clone(),
                &self.v3_auth_password,
                self.v3_priv_protocol.clone(),
                &self.v3_priv_password,
                &self.engine_id,
                self.engine_boots,
                self.engine_time,
            ))
        } else {
            None
        };

        // Scoped PDU (plaintext)
        let scoped_pdu = sequence(&[
            encode_octet_string(&self.engine_id).as_slice(),
            encode_octet_string(b"").as_slice(),
            pdu.as_slice(),
        ]
        .concat());

        // Si hay privacidad, cifrar el scoped PDU
        let (msg_data_bytes, priv_params_bytes) = if has_priv {
            if let Some(ref sec) = usm_sec {
                if !sec.priv_key_localized.is_empty() {
                    match self.v3_priv_protocol {
                        PrivProtocol::Aes128 => {
                            match v3::encrypt_aes128(
                                &sec.priv_key_localized,
                                self.engine_boots,
                                self.engine_time,
                                &scoped_pdu,
                            ) {
                                Ok((ciphertext, salt)) => {
                                    (encode_octet_string(&ciphertext), salt)
                                }
                                Err(e) => {
                                    debug!("AES encrypt error: {}, enviando sin cifrar", e);
                                    (scoped_pdu.clone(), vec![])
                                }
                            }
                        }
                        PrivProtocol::Aes192 => {
                            match v3::encrypt_aes192(
                                &sec.priv_key_localized,
                                self.engine_boots,
                                self.engine_time,
                                &scoped_pdu,
                            ) {
                                Ok((ciphertext, salt)) => {
                                    (encode_octet_string(&ciphertext), salt)
                                }
                                Err(e) => {
                                    debug!("AES-192 encrypt error: {}, enviando sin cifrar", e);
                                    (scoped_pdu.clone(), vec![])
                                }
                            }
                        }
                        PrivProtocol::Aes256 => {
                            match v3::encrypt_aes256(
                                &sec.priv_key_localized,
                                self.engine_boots,
                                self.engine_time,
                                &scoped_pdu,
                            ) {
                                Ok((ciphertext, salt)) => {
                                    (encode_octet_string(&ciphertext), salt)
                                }
                                Err(e) => {
                                    debug!("AES-256 encrypt error: {}, enviando sin cifrar", e);
                                    (scoped_pdu.clone(), vec![])
                                }
                            }
                        }
                        PrivProtocol::Des => {
                            match v3::encrypt_des(
                                &sec.priv_key_localized,
                                self.engine_boots,
                                &scoped_pdu,
                            ) {
                                Ok((ciphertext, salt)) => {
                                    (encode_octet_string(&ciphertext), salt)
                                }
                                Err(e) => {
                                    debug!("DES encrypt error: {}, enviando sin cifrar", e);
                                    (scoped_pdu.clone(), vec![])
                                }
                            }
                        }
                        PrivProtocol::None => (scoped_pdu.clone(), vec![]),
                    }
                } else {
                    (scoped_pdu.clone(), vec![])
                }
            } else {
                (scoped_pdu.clone(), vec![])
            }
        } else {
            // Sin privacidad: scoped PDU va como SEQUENCE (plaintext)
            (scoped_pdu.clone(), vec![])
        };

        // Auth placeholder (12 bytes ceros para SHA/MD5, 24 para SHA256)
        let auth_placeholder = if has_auth {
            vec![0u8; self.v3_auth_protocol.digest_length()]
        } else {
            vec![]
        };

        let usm_params = self.encode_usm_params(
            &self.engine_id,
            self.engine_boots,
            self.engine_time,
            self.v3_user.as_bytes(),
            &auth_placeholder,
            &priv_params_bytes,
        );
        let usm_encoded = encode_octet_string(&usm_params);

        // Construir el mensaje completo
        let msg = sequence(&[
            encode_integer(3).as_slice(),
            global_data.as_slice(),
            usm_encoded.as_slice(),
            msg_data_bytes.as_slice(),
        ]
        .concat());

        // Si hay autenticación, calcular y aplicar HMAC
        if let Some(ref sec) = usm_sec {
            self.apply_authentication(msg, sec)
                .unwrap_or_else(|e| { debug!("Auth error: {}", e); Vec::new() })
        } else {
            msg
        }
    }

    /// Aplica HMAC de autenticación al mensaje SNMPv3.
    fn apply_authentication(&self, mut msg: Vec<u8>, params: &UsmSecurityParams) -> Result<Vec<u8>> {
        if params.auth_key_localized.is_empty() {
            return Ok(msg);
        }
        // Calcular HMAC sobre el mensaje completo (con auth tag = ceros)
        let tag = v3::authenticate_message(&msg, &params.auth_key_localized, &params.auth_protocol)?;
        // Encontrar y reemplazar los ceros de auth tag en el mensaje
        // (búsqueda del marcador de auth placeholder)
        let placeholder = vec![0u8; params.auth_protocol.digest_length()];
        if let Some(pos) = find_subslice(&msg, &placeholder) {
            msg[pos..pos + tag.len()].copy_from_slice(&tag);
        }
        Ok(msg)
    }

    /// Codifica los parámetros USM como un SEQUENCE.
    fn encode_usm_params(
        &self,
        engine_id: &[u8],
        boots: u32,
        time: u32,
        username: &[u8],
        auth_params: &[u8],
        priv_params: &[u8],
    ) -> Vec<u8> {
        sequence(&[
            encode_octet_string(engine_id).as_slice(),
            encode_integer(boots as i64).as_slice(),
            encode_integer(time as i64).as_slice(),
            encode_octet_string(username).as_slice(),
            encode_octet_string(auth_params).as_slice(),
            encode_octet_string(priv_params).as_slice(),
        ]
        .concat())
    }

    // ===========================================================================
    // PARSEO DE RESPUESTA SNMP
    // ===========================================================================

    /// Parsea una respuesta SNMP v1/v2c y retorna los VarBinds.
    fn parse_v1v2c_response(
        &self,
        data: &[u8],
    ) -> Result<Vec<(String, SnmpValue)>> {
        let (tag, msg_data, _) = parse_tlv(data)
            .ok_or_else(|| anyhow!("Respuesta no es un TLV válido"))?;
        if tag != TAG_SEQUENCE {
            return Err(anyhow!("Respuesta no es un SEQUENCE"));
        }

        let mut offset = 0;
        // Skip version
        if let Some((_, _, c)) = parse_tlv(&msg_data[offset..]) { offset += c; }
        // Skip community
        if let Some((_, _, c)) = parse_tlv(&msg_data[offset..]) { offset += c; }

        // El PDU es el tercer elemento
        if let Some((pdu_tag, pdu_data, _)) = parse_tlv(&msg_data[offset..]) {
            if pdu_tag != TAG_GET_RESPONSE {
                return Err(anyhow!("No es un GetResponse PDU (tag=0x{:02x})", pdu_tag));
            }
            if let Some(pdu) = parse_response_pdu(pdu_data) {
                if pdu.error_status != 0 {
                    return Err(anyhow!(
                        "SNMP error: status={}, index={}",
                        pdu.error_status,
                        pdu.error_index
                    ));
                }
                return Ok(pdu.varbinds);
            }
        }

        Err(anyhow!("No se pudo parsear la respuesta SNMP"))
    }

    /// Parsea una respuesta SNMPv3 y retorna los VarBinds.
    /// Soporta descifrado AES-128-CFB / DES-CBC cuando privacidad está habilitada.
    fn parse_v3_response(&self, data: &[u8]) -> Result<Vec<(String, SnmpValue)>> {
        let (tag, msg_data, _) = parse_tlv(data)
            .ok_or_else(|| anyhow!("Respuesta v3 inválida"))?;
        if tag != TAG_SEQUENCE {
            return Err(anyhow!("Respuesta v3 no es SEQUENCE"));
        }

        let mut offset = 0;
        // Skip msgVersion
        if let Some((_, _, c)) = parse_tlv(&msg_data[offset..]) { offset += c; }
        // Skip msgGlobalData
        if let Some((_, _, c)) = parse_tlv(&msg_data[offset..]) { offset += c; }

        // Extraer USM security parameters (necesarios para descifrado)
        let (_resp_engine_id, resp_engine_boots, resp_engine_time, resp_priv_params) =
            if let Some((TAG_OCTET_STRING, usm_raw, c)) = parse_tlv(&msg_data[offset..]) {
                offset += c;
                self.extract_usm_params(usm_raw)
            } else {
                offset += if let Some((_, _, c)) = parse_tlv(&msg_data[offset..]) { c } else { 0 };
                (
                    self.engine_id.clone(),
                    self.engine_boots,
                    self.engine_time,
                    vec![],
                )
            };

        // msgData = scoped PDU (SEQUENCE) o cifrado (OCTET STRING)
        let scoped_data = if let Some((data_tag, data_content, _)) = parse_tlv(&msg_data[offset..]) {
            if data_tag == TAG_SEQUENCE {
                // Plaintext — usar directamente
                data_content.to_vec()
            } else if data_tag == TAG_OCTET_STRING {
                // Cifrado — descifrar
                let plaintext = self.decrypt_scoped_pdu(
                    data_content,
                    &resp_priv_params,
                    resp_engine_boots,
                    resp_engine_time,
                )?;
                // El plaintext es un SEQUENCE, parsear el contenido
                if let Some((TAG_SEQUENCE, inner, _)) = parse_tlv(&plaintext) {
                    inner.to_vec()
                } else {
                    return Err(anyhow!("ScopedPDU descifrado no es un SEQUENCE válido"));
                }
            } else {
                return Err(anyhow!("Formato de msgData inesperado: tag=0x{:02x}", data_tag));
            }
        } else {
            return Err(anyhow!("No se encontró msgData en respuesta v3"));
        };

        if scoped_data.is_empty() {
            return Err(anyhow!("ScopedPDU vacío"));
        }

        let mut s_offset = 0;
        // Skip contextEngineID
        if let Some((_, _, c)) = parse_tlv(&scoped_data[s_offset..]) { s_offset += c; }
        // Skip contextName
        if let Some((_, _, c)) = parse_tlv(&scoped_data[s_offset..]) { s_offset += c; }

        // PDU (GetResponse o Report)
        if let Some((pdu_tag, pdu_data, _)) = parse_tlv(&scoped_data[s_offset..]) {
            if pdu_tag == TAG_REPORT {
                // Report PDU indica condición de error SNMPv3 (credenciales, engine, time-window).
                if let Some(pdu) = parse_response_pdu(pdu_data) {
                    if let Some((report_oid, report_value)) = pdu.varbinds.first() {
                        return Err(anyhow!(
                            "SNMPv3 Report-PDU recibido: oid={}, value={}. Revisa usuario/auth/priv y sincronización de engineBoots/engineTime",
                            report_oid,
                            report_value.as_string()
                        ));
                    }
                }
                return Err(anyhow!(
                    "SNMPv3 Report-PDU recibido del agente. Revisa credenciales y parámetros de seguridad v3"
                ));
            }

            if pdu_tag != TAG_GET_RESPONSE {
                return Err(anyhow!(
                    "PDU de respuesta v3 inesperado (tag=0x{:02x})",
                    pdu_tag
                ));
            }

            if let Some(pdu) = parse_response_pdu(pdu_data) {
                if pdu.error_status != 0 {
                    return Err(anyhow!(
                        "SNMP v3 error: status={}, index={}",
                        pdu.error_status,
                        pdu.error_index
                    ));
                }
                return Ok(pdu.varbinds);
            }
        }
        Err(anyhow!("No se pudo parsear el PDU de respuesta v3"))
    }

    /// Extrae engineID, engineBoots, engineTime y privParameters desde USM params.
    fn extract_usm_params(&self, usm_raw: &[u8]) -> (Vec<u8>, u32, u32, Vec<u8>) {
        let usm_inner = if let Some((TAG_SEQUENCE, inner, _)) = parse_tlv(usm_raw) {
            inner
        } else {
            return (self.engine_id.clone(), self.engine_boots, self.engine_time, vec![]);
        };

        let mut off = 0;
        // engineID
        let engine_id = if let Some((TAG_OCTET_STRING, d, c)) = parse_tlv(&usm_inner[off..]) {
            off += c;
            d.to_vec()
        } else {
            self.engine_id.clone()
        };
        // engineBoots
        let boots = if let Some((TAG_INTEGER, d, c)) = parse_tlv(&usm_inner[off..]) {
            off += c;
            decode_integer(d).unwrap_or(0) as u32
        } else { self.engine_boots };
        // engineTime
        let time = if let Some((TAG_INTEGER, d, c)) = parse_tlv(&usm_inner[off..]) {
            off += c;
            decode_integer(d).unwrap_or(0) as u32
        } else { self.engine_time };
        // username
        if let Some((_, _, c)) = parse_tlv(&usm_inner[off..]) { off += c; }
        // authParams
        if let Some((_, _, c)) = parse_tlv(&usm_inner[off..]) { off += c; }
        // privParams
        let priv_params = if let Some((TAG_OCTET_STRING, d, _)) = parse_tlv(&usm_inner[off..]) {
            d.to_vec()
        } else {
            vec![]
        };

        (engine_id, boots, time, priv_params)
    }

    /// Intenta extraer (engineID, boots, time) desde un Report-PDU de sincronización v3.
    fn parse_v3_report_sync_hint(&self, data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
        let (tag, msg_data, _) = parse_tlv(data)?;
        if tag != TAG_SEQUENCE {
            return None;
        }

        let mut offset = 0;
        if let Some((_, _, c)) = parse_tlv(&msg_data[offset..]) { offset += c; } else { return None; }
        if let Some((_, _, c)) = parse_tlv(&msg_data[offset..]) { offset += c; } else { return None; }

        let (engine_id, boots, time, priv_params) =
            if let Some((TAG_OCTET_STRING, usm_raw, c)) = parse_tlv(&msg_data[offset..]) {
                offset += c;
                self.extract_usm_params(usm_raw)
            } else {
                return None;
            };

        let scoped_data = if let Some((data_tag, data_content, _)) = parse_tlv(&msg_data[offset..]) {
            if data_tag == TAG_SEQUENCE {
                data_content.to_vec()
            } else if data_tag == TAG_OCTET_STRING {
                let plaintext = self
                    .decrypt_scoped_pdu(data_content, &priv_params, boots, time)
                    .ok()?;
                if let Some((TAG_SEQUENCE, inner, _)) = parse_tlv(&plaintext) {
                    inner.to_vec()
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        };

        let mut s_offset = 0;
        if let Some((_, _, c)) = parse_tlv(&scoped_data[s_offset..]) { s_offset += c; } else { return None; }
        if let Some((_, _, c)) = parse_tlv(&scoped_data[s_offset..]) { s_offset += c; } else { return None; }

        if let Some((pdu_tag, pdu_data, _)) = parse_tlv(&scoped_data[s_offset..]) {
            if pdu_tag != TAG_REPORT {
                return None;
            }
            if let Some(pdu) = parse_response_pdu(pdu_data) {
                if let Some((report_oid, _)) = pdu.varbinds.first() {
                    if report_oid == "1.3.6.1.6.3.15.1.1.2.0" || report_oid == "1.3.6.1.6.3.15.1.1.4.0" {
                        return Some((engine_id, boots, time));
                    }
                }
            }
        }
        None
    }

    async fn execute_v3_request_with_recovery(
        &self,
        oid: &str,
        pdu_tag: u8,
        non_repeaters: u32,
        max_repetitions: u32,
    ) -> Result<Vec<(String, SnmpValue)>> {
        let mut working = self.clone();

        for attempt in 0..2 {
            let msg = working.build_v3_message(oid, pdu_tag, non_repeaters, max_repetitions);
            let response = working.send_udp(&msg).await?;

            match working.parse_v3_response(&response) {
                Ok(vb) => return Ok(vb),
                Err(e) => {
                    let err_msg = e.to_string();
                    if attempt == 1 || !working.is_v3_report_sync_error(&err_msg) {
                        return Err(anyhow!(err_msg));
                    }

                    if let Some((engine_id, boots, time)) = working.parse_v3_report_sync_hint(&response) {
                        working.engine_id = engine_id;
                        working.engine_boots = boots;
                        working.engine_time = time;
                        continue;
                    }

                    working.discover_engine().await?;
                }
            }
        }

        Err(anyhow!("No se pudo completar request SNMPv3 tras re-sync"))
    }

    /// Descifra el scoped PDU cifrado usando AES-128-CFB o DES-CBC.
    fn decrypt_scoped_pdu(
        &self,
        ciphertext: &[u8],
        priv_params: &[u8],
        engine_boots: u32,
        engine_time: u32,
    ) -> Result<Vec<u8>> {
        if self.v3_priv_protocol == PrivProtocol::None || self.v3_priv_password.is_empty() {
            return Err(anyhow!("Datos cifrados recibidos pero sin protocolo de privacidad configurado"));
        }

        // Derivar clave de privacidad
        let priv_key = v3::derive_priv_key(
            &self.v3_priv_password,
            &self.engine_id,
            &self.v3_auth_protocol,
            &self.v3_priv_protocol,
        );

        match self.v3_priv_protocol {
            PrivProtocol::Aes128 => {
                v3::decrypt_aes128(&priv_key, engine_boots, engine_time, priv_params, ciphertext)
            }
            PrivProtocol::Aes192 => {
                v3::decrypt_aes192(&priv_key, engine_boots, engine_time, priv_params, ciphertext)
            }
            PrivProtocol::Aes256 => {
                v3::decrypt_aes256(&priv_key, engine_boots, engine_time, priv_params, ciphertext)
            }
            PrivProtocol::Des => {
                v3::decrypt_des(&priv_key, priv_params, ciphertext)
            }
            PrivProtocol::None => Err(anyhow!("Sin protocolo de privacidad")),
        }
    }

    // ===========================================================================
    // TRANSPORTE UDP
    // ===========================================================================

    /// Envía un paquete UDP y espera la respuesta con timeout y reintentos.
    async fn send_udp(&self, data: &[u8]) -> Result<Vec<u8>> {
        let addr_str = format!("{}:{}", self.host, self.port);
        let addr: SocketAddr = addr_str
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("No se pudo resolver {}", addr_str))?;

        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        for attempt in 0..=self.retries {
            match timeout(
                Duration::from_secs(self.timeout_secs),
                self.send_recv_once(&socket, addr, data),
            )
            .await
            {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(e)) => {
                    if attempt == self.retries {
                        return Err(anyhow!("SNMP UDP error: {}", e));
                    }
                    debug!("Reintento {}/{}: {}", attempt + 1, self.retries, e);
                }
                Err(_) => {
                    if attempt == self.retries {
                        return Err(anyhow!(
                            "Timeout SNMP ({} s) a {}:{}",
                            self.timeout_secs,
                            self.host,
                            self.port
                        ));
                    }
                }
            }
        }
        Err(anyhow!("Todos los reintentos SNMP fallaron"))
    }

    async fn send_recv_once(
        &self,
        socket: &UdpSocket,
        addr: SocketAddr,
        data: &[u8],
    ) -> Result<Vec<u8>> {
        socket.send_to(data, addr).await?;
        let mut buf = vec![0u8; 65535];
        let (len, _) = socket.recv_from(&mut buf).await?;
        buf.truncate(len);
        Ok(buf)
    }

    // ===========================================================================
    // OPERACIONES SNMP PÚBLICAS
    // ===========================================================================

    /// Verifica la conectividad SNMP consultando sysDescr (1.3.6.1.2.1.1.1.0).
    pub async fn test_connectivity(&self) -> SnmpResult {
        let sys_descr = "1.3.6.1.2.1.1.1.0";
        match self.get(sys_descr).await {
            result if result.is_ok() => result,
            result => result,
        }
    }

    fn is_v3_report_sync_error(&self, err: &str) -> bool {
        self.version == SnmpVersion::V3
            && (err.contains("SNMPv3 Report-PDU")
                || err.contains("1.3.6.1.6.3.15.1.1.2.0")
                || err.contains("1.3.6.1.6.3.15.1.1.4.0"))
    }

    /// Realiza una operación SNMP GET para un OID específico.
    pub async fn get(&self, oid: &str) -> SnmpResult {
        let varbinds = match self.version {
            SnmpVersion::V3 => match self
                .execute_v3_request_with_recovery(oid, TAG_GET_REQUEST, 0, 0)
                .await
            {
                Ok(vb) => vb,
                Err(e) => return SnmpResult::err(oid.to_string(), e.to_string()),
            },
            _ => {
                let msg = self.build_v1v2c_get(oid, TAG_GET_REQUEST);
                let response = match self.send_udp(&msg).await {
                    Ok(r) => r,
                    Err(e) => return SnmpResult::err(oid.to_string(), e.to_string()),
                };
                match self.parse_v1v2c_response(&response) {
                    Ok(vb) => vb,
                    Err(e) => return SnmpResult::err(oid.to_string(), e.to_string()),
                }
            }
        };

        if let Some((resp_oid, value)) = varbinds.into_iter().next() {
            if value.is_error() {
                SnmpResult::err(
                    oid.to_string(),
                    format!("OID no soportado: {}", value.as_string()),
                )
            } else {
                SnmpResult::ok(resp_oid, value)
            }
        } else {
            SnmpResult::err(oid.to_string(), "Sin datos en respuesta".to_string())
        }
    }

    /// Realiza una operación SNMP GETNEXT.
    pub async fn get_next(&self, oid: &str) -> SnmpResult {
        let varbinds = match self.version {
            SnmpVersion::V3 => match self
                .execute_v3_request_with_recovery(oid, TAG_GETNEXT_REQUEST, 0, 0)
                .await
            {
                Ok(vb) => vb,
                Err(e) => return SnmpResult::err(oid.to_string(), e.to_string()),
            },
            _ => {
                let msg = self.build_v1v2c_get(oid, TAG_GETNEXT_REQUEST);
                let response = match self.send_udp(&msg).await {
                    Ok(r) => r,
                    Err(e) => return SnmpResult::err(oid.to_string(), e.to_string()),
                };
                match self.parse_v1v2c_response(&response) {
                    Ok(vb) => vb,
                    Err(e) => return SnmpResult::err(oid.to_string(), e.to_string()),
                }
            }
        };

        if let Some((resp_oid, value)) = varbinds.into_iter().next() {
            if value.is_error() {
                SnmpResult::err(oid.to_string(), value.as_string())
            } else {
                SnmpResult::ok(resp_oid, value)
            }
        } else {
            SnmpResult::err(oid.to_string(), "Sin datos".to_string())
        }
    }

    /// Realiza operación SNMP GETBULK para obtener tablas completas.
    /// Para SNMPv1, usa GETNEXT walk como fallback automático.
    pub async fn bulk(
        &self,
        oid: &str,
        max_repetitions: u32,
    ) -> (Vec<(String, SnmpValue)>, Option<String>) {
        if self.version == SnmpVersion::V1 {
            return self.walk_via_getnext(oid, max_repetitions as usize).await;
        }

        let varbinds = match self.version {
            SnmpVersion::V3 => match self
                .execute_v3_request_with_recovery(oid, TAG_GETBULK_REQUEST, 0, max_repetitions)
                .await
            {
                Ok(vb) => vb,
                Err(e) => return (vec![], Some(e.to_string())),
            },
            _ => {
                let msg = self.build_v2c_getbulk(oid, 0, max_repetitions);
                let response = match self.send_udp(&msg).await {
                    Ok(r) => r,
                    Err(e) => return (vec![], Some(e.to_string())),
                };
                match self.parse_v1v2c_response(&response) {
                    Ok(vb) => vb,
                    Err(e) => return (vec![], Some(e.to_string())),
                }
            }
        };

        // Filtrar solo OIDs que son sub-árbol del OID base
        let base_oid = oid.trim_end_matches('.');
        let results: Vec<(String, SnmpValue)> = varbinds
            .into_iter()
            .filter(|(resp_oid, value)| {
                resp_oid.starts_with(base_oid) && !value.is_error()
            })
            .collect();

        (results, None)
    }

    /// Walk completo de una tabla SNMP: itera con GETBULK hasta agotar la tabla.
    ///
    /// A diferencia de `bulk(OID, n)` que solo hace UNA llamada GETBULK y trae
    /// como mucho `n` entradas, `bulk_walk(OID)` continúa iterando mientras
    /// el agente devuelva entradas dentro del sub-árbol del OID base.
    ///
    /// Esto es necesario para switches con muchas interfaces (ej: Huawei S5731
    /// con 48 GE + 4 10GE + interfaces virtuales = ~60 entradas, que exceden
    /// el max_repetitions=50 de una sola llamada).
    ///
    /// Parámetros:
    /// - oid: OID base de la tabla (ej: "1.3.6.1.2.1.2.2.1.1" para ifIndex)
    /// - page_size: entradas por iteración (default 50 = max_repetitions seguro)
    /// - max_total: tope duro para evitar loops infinitos (default 500)
    pub async fn bulk_walk(
        &self,
        oid: &str,
    ) -> (Vec<(String, SnmpValue)>, Option<String>) {
        let page_size: u32 = 50;
        let max_total: usize = 500;
        let mut all_results: Vec<(String, SnmpValue)> = Vec::new();
        let mut last_oid: Option<String> = None;
        let base_oid = oid.trim_end_matches('.');

        for _iteration in 0..20 {  // hasta 20 páginas × 50 = 1000 entradas máx
            // OID de inicio: la primera vez es el base; las siguientes son
            // el último OID recibido + 1 lexicográficamente.
            let start_oid = match &last_oid {
                Some(prev) => increment_oid_lex(prev),
                None => oid.to_string(),
            };

            let (page_results, error) = self.bulk(&start_oid, page_size).await;
            if let Some(err) = error {
                if all_results.is_empty() {
                    return (vec![], Some(err));
                }
                break;
            }

            // Filtrar entradas que aún estén dentro del sub-árbol del OID base
            // (bulk() ya hace este filtro, pero al iterar necesitamos evitar
            // que el OID incrementado caiga fuera del árbol).
            let valid_entries: Vec<(String, SnmpValue)> = page_results
                .into_iter()
                .filter(|(resp_oid, _)| resp_oid.starts_with(base_oid))
                .collect();

            let prev_count = all_results.len();
            for entry in valid_entries {
                if all_results.len() >= max_total {
                    break;
                }
                // Evitar duplicados: el primer OID de la nueva página puede
                // coincidir con el último de la página anterior.
                if Some(&entry.0) != last_oid.as_ref() {
                    all_results.push(entry);
                }
            }

            // Si no obtuvimos entradas nuevas, terminamos.
            if all_results.len() == prev_count {
                break;
            }
            // Guardar el último OID para la siguiente iteración.
            if let Some(last) = all_results.last() {
                last_oid = Some(last.0.clone());
            }
            // Si obtuvimos menos del page_size, ya no hay más entradas.
            if all_results.len() - prev_count < page_size as usize {
                break;
            }
        }

        (all_results, None)
    }

    /// Walk iterativo via GETNEXT (fallback para SNMPv1 o cuando GETBULK falla).
    async fn walk_via_getnext(
        &self,
        base_oid: &str,
        max_results: usize,
    ) -> (Vec<(String, SnmpValue)>, Option<String>) {
        let mut results = Vec::new();
        let mut current_oid = base_oid.to_string();
        let base = base_oid.trim_end_matches('.');

        for _ in 0..max_results {
            let result = self.get_next(&current_oid).await;
            if let Some(error) = &result.error {
                return (results, Some(error.clone()));
            }
            let resp_oid = result.oid.clone();
            if !resp_oid.starts_with(base) {
                // Salimos del árbol
                break;
            }
            if let Some(value) = result.value {
                if value.is_error() {
                    break;
                }
                results.push((resp_oid.clone(), value));
                current_oid = resp_oid;
            } else {
                break;
            }
        }
        (results, None)
    }
}

// ==============================================================================
// UTILIDADES
// ==============================================================================

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Busca un sub-slice dentro de un slice.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
