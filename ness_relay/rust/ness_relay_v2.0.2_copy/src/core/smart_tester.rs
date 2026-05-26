use anyhow::{Context, Result};
use reqwest::{Client, Url};
use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use super::config::{load_devices_from_config, DeviceConfig};
use crate::profiles::loader::ProfileLoader;
use crate::snmp::{SnmpClient, SnmpVersion};

const SYS_NAME_OID: &str = "1.3.6.1.2.1.1.5.0";
const SYS_DESCR_OID: &str = "1.3.6.1.2.1.1.1.0";
const SYS_OBJECT_ID_OID: &str = "1.3.6.1.2.1.1.2.0";
const AUTOCOMPLETE_FILE: &str = "/tmp/ness_smart_tester_autocomplete.conf";

pub async fn run_verify_setup(
    config_file: PathBuf,
    server_url: Option<String>,
    auto_fix: bool,
    assume_yes: bool,
) -> Result<()> {
    println!("\n=== NESS Relay Smart Tester ===");
    println!("Diagnóstico inteligente de entorno, red y SNMP\n");

    phase_a_system_readiness(auto_fix, assume_yes)?;

    let mut devices = load_devices_for_tester(&config_file);
    let endpoint = server_url.unwrap_or_else(|| "https://cloud.nesshq.com/api/relay/data/".to_string());

    let manual_ip = if devices.is_empty() {
        prompt_manual_target_ip(assume_yes)?
    } else {
        None
    };

    let reachable_targets = phase_b_network_health(&devices, &endpoint, manual_ip.as_deref()).await;

    if devices.is_empty() {
        if let Some(ip) = manual_ip {
            let mut allow_snmp = reachable_targets.contains(&ip);
            if !allow_snmp && !assume_yes {
                println!("[WARN] La IP {} no respondió a ping.", ip);
                allow_snmp = ask_yes_no("¿Desea intentar validación SNMP de todas formas? (Y/n): ")?;
            }

            if allow_snmp {
                if let Some(manual_device) = prompt_manual_snmp_device(&ip, assume_yes).await? {
                    devices.push(manual_device);
                }
            } else {
                println!("[INFO] Se omite Fase C manual porque no hay conectividad ICMP confirmada.");
            }
        }
    }

    let validated_ids = phase_c_deep_snmp_validation(&devices).await;
    phase_local_firewall_checker();

    // Exportar datos de dispositivos validados para autocompletado del instalador
    export_autocomplete_data(&devices, &validated_ids);

    println!("\nSmart Tester completado. Revisa advertencias y sugerencias para corregir antes de producción.\n");
    Ok(())
}

fn load_devices_for_tester(config_file: &PathBuf) -> Vec<DeviceConfig> {
    // Limpiar archivo de autocompletado anterior si existe
    let _ = std::fs::remove_file(AUTOCOMPLETE_FILE);

    if !config_file.exists() {
        println!("[INFO] No existe archivo de dispositivos en {}.", config_file.display());
        println!("[INFO] Se habilitará modo interactivo para diagnóstico manual (IP/SNMP).\n");
        return vec![];
    }

    match load_devices_from_config(config_file) {
        Ok(devices) => {
            if devices.is_empty() {
                println!("[WARN] El archivo de dispositivos no contiene equipos válidos. Se omite validación SNMP.");
            }
            devices
        }
        Err(e) => {
            println!("[WARN] No se pudo leer la configuración de dispositivos: {}", e);
            vec![]
        }
    }
}

fn phase_a_system_readiness(auto_fix: bool, assume_yes: bool) -> Result<()> {
    println!();
    println!("[Fase A] System Readiness");
    println!();

    let cron_installed = is_command_available("crontab") || is_command_available("cron") || is_command_available("crond");
    let service_name = detect_cron_service_name();

    // Mostrar estado del servicio cron si systemctl disponible
    if let Some(ref svc) = service_name {
        let status_output = run_command_output("systemctl", &["status", svc, "--no-pager"]);
        if !status_output.trim().is_empty() {
            println!("{}", colorize_systemctl_output(status_output.trim()));
        }
    }

    println!();
    if cron_installed {
        println!("[OK] Cron detectado en el sistema.");
    } else {
        println!("[WARN] Cron no está instalado.");

        if auto_fix {
            let consent = if assume_yes {
                true
            } else {
                ask_yes_no("Se requiere instalar cron para programación automática. ¿Autoriza la instalación ahora? [Y/n]: ")?
            };

            if consent {
                install_cron_by_distro().context("No se pudo instalar cron automáticamente")?;
                println!("[OK] Instalación automática de cron finalizada.");
            } else {
                println!("[WARN] Usuario no autorizó instalación de cron. El agendamiento puede fallar.");
            }
        } else {
            println!("[INFO] Auto-fix deshabilitado. Recomendación: instalar cron antes de terminar.");
        }
    }

    match service_name {
        Some(name) => {
            if systemctl_is_enabled(&name) {
                println!("[OK] Servicio {} habilitado.", name);
            } else {
                println!("[WARN] Servicio {} no está habilitado.", name);
                if auto_fix {
                    let consent = if assume_yes {
                        true
                    } else {
                        ask_yes_no(&format!("¿Desea habilitar el servicio {} ahora? [Y/n]: ", name))?
                    };
                    if consent {
                        let _ = run_command_status("systemctl", &["enable", &name]);
                        let _ = run_command_status("systemctl", &["start", &name]);
                        if systemctl_is_enabled(&name) {
                            println!("[OK] Servicio {} habilitado y en ejecución.", name);
                        } else {
                            println!("[WARN] No se pudo habilitar {} automáticamente.", name);
                        }
                    }
                }
            }
        }
        None => println!("[INFO] No se detectó systemd o servicio cron explícito. Verifica el scheduler manualmente."),
    }

    println!();
    Ok(())
}

async fn phase_b_network_health(
    devices: &[DeviceConfig],
    endpoint: &str,
    manual_target_ip: Option<&str>,
) -> HashSet<String> {
    println!();
    println!("[Fase B] Network Health");
    println!();

    let gateway_ip = detect_default_gateway();
    let gateway_ok = gateway_ip
        .as_deref()
        .map_or(false, ping_host);

    if let Some(gw) = gateway_ip.as_deref() {
        if gateway_ok {
            println!("[OK] Gateway {} responde a ping.", gw);
        } else {
            println!("[WARN] Gateway {} no responde a ping.", gw);
        }
    } else {
        println!("[WARN] No se detectó gateway por defecto.");
    }

    let mut seen_targets: HashSet<String> = HashSet::new();
    let mut reachable_targets: HashSet<String> = HashSet::new();
    for device in devices {
        if seen_targets.insert(device.ip.clone()) {
            if ping_host(&device.ip) {
                println!("[OK] Ping a dispositivo {} exitoso.", device.ip);
                reachable_targets.insert(device.ip.clone());
            } else {
                if gateway_ok {
                    println!("[WARN] {} no responde a ping. Puede estar apagado o bloqueando ICMP.", device.ip);
                } else {
                    println!("[WARN] {} y el gateway no responden. Posible problema local de interfaz/ruta.", device.ip);
                }
                println!("[INFO] El dispositivo no responde a PING, intentando conexión SNMP directamente...");
            }
        }
    }

    if let Some(ip) = manual_target_ip {
        if seen_targets.insert(ip.to_string()) {
            if ping_host(ip) {
                println!("[OK] Ping manual a {} exitoso.", ip);
                reachable_targets.insert(ip.to_string());
            } else if gateway_ok {
                println!("[WARN] Ping manual a {} falló. El equipo puede bloquear ICMP o estar apagado.", ip);
                println!("[INFO] El dispositivo no responde a PING, intentando conexión SNMP directamente...");
            } else {
                println!("[WARN] Ping manual a {} falló y el gateway tampoco responde.", ip);
                println!("[INFO] Posible problema local de interfaz/ruta. Se puede intentar SNMP manualmente.");
            }
        }
    }

    if devices.is_empty() && manual_target_ip.is_none() {
        println!("[INFO] Sin dispositivos configurados ni IP manual. Fase B solo evaluará gateway y salida HTTPS.");
    }

    // Mostrar solo el hostname/IP del servidor, no la ruta completa del endpoint
    let server_display = reqwest::Url::parse(endpoint)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| endpoint.to_string());

    match test_https_connectivity(endpoint).await {
        Ok(()) => println!("[OK] Salida HTTPS hacia NESS disponible: {}", server_display),
        Err(e) => {
            println!("[WARN] Falló salida HTTPS hacia NESS ({}): {}", server_display, e);
            println!("Sugerencia: valide DNS, proxy y reglas de salida del firewall perimetral.");
        }
    }

    println!();
    reachable_targets
}

fn prompt_manual_target_ip(assume_yes: bool) -> Result<Option<String>> {
    if assume_yes || !io::stdin().is_terminal() {
        return Ok(None);
    }

    println!("[Asistente] No hay connection.config. Puede ejecutar diagnóstico ingresando una IP manual.");
    println!();
    let wants_manual = ask_yes_no("¿Desea ingresar una IP para prueba de PING inteligente? (Y/n): ")?;
    if !wants_manual {
        return Ok(None);
    }

    loop {
        let ip = read_input("Ingrese IP/host objetivo: ")?;
        if !ip.is_empty() {
            return Ok(Some(ip));
        }
        println!("[WARN] Debe ingresar una IP/host válido.");
    }
}

async fn prompt_manual_snmp_device(ip: &str, assume_yes: bool) -> Result<Option<DeviceConfig>> {
    if assume_yes || !io::stdin().is_terminal() {
        return Ok(None);
    }

    let wants_snmp = ask_yes_no("¿Desea ejecutar prueba SNMP manual para esta IP? (Y/n): ")?;
    if !wants_snmp {
        return Ok(None);
    }

    let version_input = read_input("Versión SNMP [1/2/3] (default 3): ")?;
    let version_choice = if version_input.trim().is_empty() {
        "3".to_string()
    } else {
        version_input.trim().to_string()
    };

    let mut device = DeviceConfig {
        device_id: "manual_1".to_string(),
        vendor: "generic".to_string(),
        ip: ip.to_string(),
        port: 161,
        description: "Diagnóstico manual Smart Tester".to_string(),
        snmp_version: "3".to_string(),
        community: "public".to_string(),
        v3_user: String::new(),
        v3_auth_protocol: "SHA".to_string(),
        v3_auth_password: String::new(),
        v3_priv_protocol: "AES128".to_string(),
        v3_priv_password: String::new(),
    };

    let port_input = read_input("Puerto SNMP (default 161): ")?;
    if let Ok(port) = port_input.trim().parse::<u16>() {
        device.port = port;
    }

    match version_choice.as_str() {
        "1" => {
            device.snmp_version = "1".to_string();
            let community = read_input("Community SNMPv1 (default public): ")?;
            if !community.trim().is_empty() {
                device.community = community.trim().to_string();
            }
        }
        "3" => {
            device.snmp_version = "3".to_string();
            device.community.clear();

            let user = read_required_input("Usuario SNMPv3: ")?;
            device.v3_user = user;

            // Menú numérico para protocolo de autenticación
            println!();
            println!("Protocolo de Autenticación:");
            println!("  1) MD5       (HMAC-MD5-96)");
            println!("  2) SHA       (HMAC-SHA1-96, recomendado)");
            println!("  3) SHA256    (HMAC-SHA2-256-128)");
            println!("  4) SHA256-192 (HMAC-SHA2-256-192)");
            println!("  5) SHA384    (HMAC-SHA2-384-192)");
            println!("  6) SHA512    (HMAC-SHA2-512-256)");
            println!("  7) NONE      (sin autenticación — no usar con privacidad)");
            let auth_choice = read_input("Selecciona 1-7 [default: 2]: ")?;
            device.v3_auth_protocol = match auth_choice.trim() {
                "1" => "MD5".to_string(),
                "2" => "SHA".to_string(),
                "3" => "SHA256".to_string(),
                "4" => "SHA256-192".to_string(),
                "5" => "SHA384".to_string(),
                "6" => "SHA512".to_string(),
                "7" => "NONE".to_string(),
                _ => "SHA".to_string(),
            };

            if device.v3_auth_protocol != "NONE" {
                // Opción de visibilidad de contraseña
                let show_auth_pass = ask_yes_no("¿Desea ver la contraseña al ingresarla? (Y/n): ")?;
                let auth_password = if show_auth_pass {
                    read_required_input("Auth password SNMPv3: ")?
                } else {
                    read_password("Auth password SNMPv3: ")?
                };
                device.v3_auth_password = auth_password;

                // Menú numérico para protocolo de privacidad
                println!();
                println!("Protocolo de Privacidad (Encriptación):");
                println!("  1) AES128  (AES-128-CFB, recomendado)");
                println!("  2) AES192  (AES-192-CFB)");
                println!("  3) AES256  (AES-256-CFB)");
                println!("  4) DES     (DES-CBC, obsoleto)");
                println!("  5) NONE    (sin encriptación)");
                let priv_choice = read_input("Selecciona 1-5 [default: 1]: ")?;
                device.v3_priv_protocol = match priv_choice.trim() {
                    "2" => "AES192".to_string(),
                    "3" => "AES256".to_string(),
                    "4" => "DES".to_string(),
                    "5" => "NONE".to_string(),
                    _ => "AES128".to_string(),
                };

                if device.v3_priv_protocol != "NONE" {
                    let show_priv_pass = ask_yes_no("¿Desea ver la contraseña al ingresarla? (Y/n): ")?;
                    let priv_password = if show_priv_pass {
                        read_required_input("Priv password SNMPv3: ")?
                    } else {
                        read_password("Priv password SNMPv3: ")?
                    };
                    device.v3_priv_password = priv_password;
                }
            } else {
                println!("[INFO] SNMPv3 sin autenticación: privacidad desactivada automáticamente (NONE).");
                device.v3_auth_password.clear();
                device.v3_priv_protocol = "NONE".to_string();
                device.v3_priv_password.clear();
            }
        }
        _ => {
            device.snmp_version = "2c".to_string();
            let community = read_input("Community SNMPv2c (default public): ")?;
            if !community.trim().is_empty() {
                device.community = community.trim().to_string();
            }
        }
    }

    let profile_loader = ProfileLoader::new();
    if let Ok(client) = SnmpClient::new(&device.to_json()).await {
        let sys_descr = client.get(SYS_DESCR_OID).await
            .value
            .as_ref()
            .map(|v| v.as_string())
            .unwrap_or_default();
        let sys_object_id = client.get(SYS_OBJECT_ID_OID).await
            .value
            .as_ref()
            .map(|v| v.as_string())
            .unwrap_or_default();

        let resolved_profile = profile_loader.resolve_profile(&device.vendor, &sys_object_id, &sys_descr);
        if resolved_profile.vendor() != device.vendor {
            println!("[OK] Vendor detectado automáticamente: {} ({})", resolved_profile.vendor_display_name(), resolved_profile.vendor());
        }
        device.vendor = resolved_profile.vendor().to_string();
    }

    Ok(Some(device))
}

fn read_input(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn read_required_input(prompt: &str) -> Result<String> {
    loop {
        let value = read_input(prompt)?;
        if !value.is_empty() {
            return Ok(value);
        }
        println!("[WARN] Este campo es obligatorio.");
    }
}

/// Lee una contraseña sin mostrarla en pantalla.
fn read_password(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    // Intentar desactivar echo del terminal
    let echo_disabled = run_command_status("stty", &["-echo"]).unwrap_or(false);

    let mut password = String::new();
    io::stdin().read_line(&mut password)?;

    if echo_disabled {
        let _ = run_command_status("stty", &["echo"]);
        println!(); // Nueva línea ya que Enter no se mostró
    }

    let trimmed = password.trim().to_string();
    if trimmed.is_empty() {
        println!("[WARN] Este campo es obligatorio.");
        return read_password(prompt);
    }
    Ok(trimmed)
}

/// Returns the set of device_ids that passed SNMP validation.
async fn phase_c_deep_snmp_validation(devices: &[DeviceConfig]) -> HashSet<String> {
    println!();
    println!("[Fase C] Deep SNMP Validation");
    println!();

    let mut validated = HashSet::new();

    if devices.is_empty() {
        println!("[INFO] Sin dispositivos configurados. Se omite validación SNMP profunda.\n");
        return validated;
    }

    for device in devices {
        println!("Dispositivo {} ({})", device.device_id, device.ip);

        let cfg = device.to_json();
        let client = match SnmpClient::new(&cfg).await {
            Ok(c) => c,
            Err(e) => {
                println!("[ERROR] No se pudo construir cliente SNMP: {}", e);
                continue;
            }
        };

        if client.version == SnmpVersion::V3 && client.engine_id.is_empty() {
            println!("[WARN] EngineID Discovery falló. El equipo puede no soportar SNMPv3 o estar filtrando reportes USM.");
        }

        let mdt = client.get(SYS_NAME_OID).await;
        if mdt.is_ok() {
            if let Some(value) = mdt.value {
                let rendered = value.as_string();
                if rendered.trim().is_empty() {
                    println!("[WARN] MDT: conexión SNMP OK pero sysName vacío. Posible falta de permisos de lectura en la MIB.");
                } else {
                    println!("[OK] MDT sysName leído correctamente: {}", rendered);
                }
            }
            validated.insert(device.device_id.clone());
            continue;
        }

        let err = mdt.error.unwrap_or_else(|| "error desconocido".to_string());
        classify_snmp_error(device, &client.version, &err);
    }

    validated
}

/// Exporta datos del dispositivo validado a un archivo temporal para que
/// el instalador (install_relay.sh) pueda ofrecer autocompletado.
fn export_autocomplete_data(devices: &[DeviceConfig], validated_ids: &HashSet<String>) {
    // Solo exportar dispositivos manuales que pasaron validación SNMP
    let manual_validated: Vec<&DeviceConfig> = devices.iter()
        .filter(|d| d.device_id.starts_with("manual_") && validated_ids.contains(&d.device_id))
        .collect();

    if manual_validated.is_empty() {
        return;
    }

    let mut content = String::new();
    content.push_str("# NESS Smart Tester — Datos de autocompletado\n");
    content.push_str("# Generado automáticamente. Este archivo será consumido por install_relay.sh\n");

    for device in &manual_validated {
        content.push_str(&format!("ip={}\n", device.ip));
        content.push_str(&format!("port={}\n", device.port));
        content.push_str(&format!("snmp_version={}\n", device.snmp_version));

        if device.snmp_version == "3" {
            content.push_str(&format!("v3_user={}\n", device.v3_user));
            content.push_str(&format!("v3_auth_protocol={}\n", device.v3_auth_protocol));
            content.push_str(&format!("v3_auth_password={}\n", device.v3_auth_password));
            content.push_str(&format!("v3_priv_protocol={}\n", device.v3_priv_protocol));
            content.push_str(&format!("v3_priv_password={}\n", device.v3_priv_password));
        } else {
            content.push_str(&format!("community={}\n", device.community));
        }
    }

    if let Err(e) = std::fs::write(AUTOCOMPLETE_FILE, &content) {
        // No es un error crítico, solo no se ofrece autocompletado
        eprintln!("[WARN] No se pudo escribir archivo de autocompletado: {}", e);
    } else {
        // Establecer permisos restrictivos (solo root puede leer, contiene contraseñas)
        let _ = run_command_status("chmod", &["600", AUTOCOMPLETE_FILE]);
    }
}

fn classify_snmp_error(device: &DeviceConfig, version: &SnmpVersion, err: &str) {
    let lower = err.to_lowercase();

    match version {
        SnmpVersion::V1 | SnmpVersion::V2c => {
            if lower.contains("timeout") {
                println!("[WARN] Timeout SNMP: posible puerto UDP 161 bloqueado o equipo no alcanzable.");
                if device.vendor == "pfsense" {
                    println!("Sugerencia: Vaya a Services > SNMP en su pfSense y habilite la interfaz LAN.");
                }
                println!("Sugerencia copy-paste: sudo nmap -sU -p {} {}", device.port, device.ip);
                return;
            }
            if lower.contains("status=16") || lower.contains("authorization") || lower.contains("auth") {
                println!("[WARN] Authorization Error: Community String incorrecta o no permitida desde esta IP.");
                return;
            }
            println!("[WARN] Error SNMP v1/v2c: {}", err);
        }
        SnmpVersion::V3 => {
            if let Some(kind) = detect_v3_failure_kind(&lower) {
                print_v3_failure_message(device, kind);
                return;
            }
            if lower.contains("timeout") {
                println!("[WARN] Timeout SNMPv3: UDP 161 bloqueado o dispositivo inaccesible.");
                return;
            }
            if lower.contains("report-pdu") || lower.contains("security") {
                println!("[WARN] Error de seguridad SNMPv3: revisar EngineID, usuario, auth/priv y ventana de tiempo.");
                return;
            }
            println!("[WARN] Error SNMPv3: {}", err);
        }
    }
}

#[derive(Clone, Copy)]
enum V3FailureKind {
    UnknownUser,
    WrongDigest,
    NotInTimeWindow,
    DecryptionError,
    UnknownEngineId,
}

fn detect_v3_failure_kind(lower_error: &str) -> Option<V3FailureKind> {
    if lower_error.contains("1.3.6.1.6.3.15.1.1.3.0") || lower_error.contains("unknownusername") {
        return Some(V3FailureKind::UnknownUser);
    }
    if lower_error.contains("1.3.6.1.6.3.15.1.1.5.0") || lower_error.contains("wrongdigest") {
        return Some(V3FailureKind::WrongDigest);
    }
    if lower_error.contains("1.3.6.1.6.3.15.1.1.2.0") || lower_error.contains("notintimewindow") {
        return Some(V3FailureKind::NotInTimeWindow);
    }
    if lower_error.contains("1.3.6.1.6.3.15.1.1.6.0") || lower_error.contains("decryption") {
        return Some(V3FailureKind::DecryptionError);
    }
    if lower_error.contains("1.3.6.1.6.3.15.1.1.4.0") || lower_error.contains("unknownengineid") {
        return Some(V3FailureKind::UnknownEngineId);
    }
    None
}

fn print_v3_failure_message(device: &DeviceConfig, kind: V3FailureKind) {
    match kind {
        V3FailureKind::UnknownUser => {
            println!("[WARN] SNMPv3 UnknownUser: el usuario SNMPv3 no existe o no está habilitado para esta IP.");
        }
        V3FailureKind::WrongDigest => {
            println!("[WARN] SNMPv3 WrongDigest: usuario existe, pero auth password/protocolo (MD5/SHA/SHA2) es incorrecto.");
        }
        V3FailureKind::NotInTimeWindow => {
            println!("[WARN] SNMPv3 NotInTimeWindow: desfase de tiempo/engine boots entre relay y dispositivo.");
        }
        V3FailureKind::DecryptionError => {
            println!("[WARN] SNMPv3 DecryptionError: clave de privacidad incorrecta o cifrado no soportado (AES128/192/256/DES). ");
        }
        V3FailureKind::UnknownEngineId => {
            println!("[WARN] SNMPv3 UnknownEngineID: el equipo no reconoce el engineID usado en la sesión.");
        }
    }

    println!(
        "Sugerencia vendor {}: {}",
        device.vendor,
        vendor_specific_v3_guidance(&device.vendor, kind)
    );
}

fn vendor_specific_v3_guidance(vendor: &str, kind: V3FailureKind) -> &'static str {
    match vendor {
        "pfsense" => match kind {
            V3FailureKind::UnknownUser => "En pfSense revise Services > SNMP > SNMPv3 Users y valide username + source address permitida.",
            V3FailureKind::WrongDigest => "En pfSense valide Auth Protocol (SHA/MD5/SHA2) y vuelva a escribir Auth Passphrase sin espacios ocultos.",
            V3FailureKind::NotInTimeWindow => "Sincronice hora con NTP en Status > NTP y reinicie el servicio SNMP para resincronizar engine time.",
            V3FailureKind::DecryptionError => "Revise Priv Protocol/Passphrase en Services > SNMP. Si usa AES256 pruebe AES128 por compatibilidad.",
            V3FailureKind::UnknownEngineId => "Reinicie el servicio SNMP y vuelva a ejecutar el Smart Tester para forzar nuevo discovery de engineID.",
        },
        "fortinet" => match kind {
            V3FailureKind::UnknownUser => "En FortiGate valide el usuario SNMPv3 en config system snmp user y permisos de consulta en la interfaz.",
            V3FailureKind::WrongDigest => "Valide auth-proto y auth-pwd con show full-configuration system snmp user <user>.",
            V3FailureKind::NotInTimeWindow => "Verifique NTP en el FortiGate y en el host del relay. Un drift de tiempo rompe USM.",
            V3FailureKind::DecryptionError => "Confirme priv-proto y priv-pwd; si falla AES256, pruebe AES128 para descartar incompatibilidad.",
            V3FailureKind::UnknownEngineId => "Regenerar sesión SNMPv3 tras cambios de firmware/config puede resolver engineID stale.",
        },
        "cisco" => match kind {
            V3FailureKind::UnknownUser => "Verifique snmp-server user <user> <group> v3 y ACL permitiendo la IP del relay.",
            V3FailureKind::WrongDigest => "Confirme algoritmo auth (md5/sha/sha256/sha384/sha512) y secret exacto en snmp-server user.",
            V3FailureKind::NotInTimeWindow => "Revise clock/NTP del equipo Cisco y del relay; luego reintente discovery.",
            V3FailureKind::DecryptionError => "Valide priv aes|des y secret de privacidad. Alinee algoritmo con el configurado en el relay.",
            V3FailureKind::UnknownEngineId => "Ejecute de nuevo el tester para refrescar engineID tras recarga de SNMP en el equipo.",
        },
        "mikrotik" | "mikrotik_fw" => match kind {
            V3FailureKind::UnknownUser => "En MikroTik valide SNMP users y permisos del usuario v3 para la IP de gestión del relay.",
            V3FailureKind::WrongDigest => "Revise auth-protocol/auth-password en /snmp user print detail.",
            V3FailureKind::NotInTimeWindow => "Sincronice NTP en RouterOS y en el relay para evitar fallo de ventana temporal USM.",
            V3FailureKind::DecryptionError => "Alinee encryption-protocol y clave de privacidad. Pruebe AES128 antes de AES256.",
            V3FailureKind::UnknownEngineId => "Deshabilite/habilite SNMP en RouterOS y repita discovery para actualizar engineID.",
        },
        "ubnt" | "c_n" => match kind {
            V3FailureKind::UnknownUser => "Valide usuario v3 y permisos de lectura SNMP en el controlador/equipo.",
            V3FailureKind::WrongDigest => "Revise protocolo auth y contraseña en ambos extremos (equipo y relay).",
            V3FailureKind::NotInTimeWindow => "Configure NTP estable y reintente tras sincronización.",
            V3FailureKind::DecryptionError => "Ajuste priv protocol y passphrase; use configuración conservadora (SHA + AES128).",
            V3FailureKind::UnknownEngineId => "Reiniciar SNMP en el equipo suele regenerar contexto y resolver engineID inválido.",
        },
        _ => match kind {
            V3FailureKind::UnknownUser => "Valide que el usuario SNMPv3 exista y tenga permisos de lectura para el contexto consultado.",
            V3FailureKind::WrongDigest => "Revise auth protocol (MD5/SHA/SHA2) y contraseña de autenticación.",
            V3FailureKind::NotInTimeWindow => "Sincronice fecha/hora (NTP) en ambos extremos y repita discovery.",
            V3FailureKind::DecryptionError => "Revise contraseña y algoritmo de privacidad (AES128/192/256/DES).",
            V3FailureKind::UnknownEngineId => "Refresque engine discovery reiniciando SNMP o repitiendo la sesión desde cero.",
        },
    }
}

fn phase_local_firewall_checker() {
    println!();
    println!("[Fase D] Local Firewall Checker");
    println!();

    if is_command_available("ufw") {
        let out = run_command_output("ufw", &["status"]);
        if out.to_lowercase().contains("active") {
            println!("[INFO] UFW activo. Verifique reglas UDP/161 y respuestas de retorno.");
            if out.contains("161") && (out.contains("DENY") || out.contains("REJECT")) {
                println!("[WARN] UFW parece bloquear SNMP (UDP/161). Ajuste reglas locales.");
            }
        }
    }

    if is_command_available("firewall-cmd") {
        let state = run_command_output("firewall-cmd", &["--state"]);
        if state.trim() == "running" {
            println!("[INFO] firewalld activo. Revise zona activa y puertos UDP/161.");
        }
    }

    if is_command_available("iptables") {
        let input = run_command_output("iptables", &["-S", "INPUT"]);
        if contains_snmp_drop_rule(&input) {
            println!("[WARN] iptables INPUT contiene regla DROP/REJECT para tráfico SNMP.");
        }
        let output = run_command_output("iptables", &["-S", "OUTPUT"]);
        if contains_snmp_drop_rule(&output) {
            println!("[WARN] iptables OUTPUT contiene regla DROP/REJECT para tráfico SNMP.");
        }
    }

    if is_command_available("nft") {
        let nft = run_command_output("nft", &["list", "ruleset"]);
        let lower = nft.to_lowercase();
        if lower.contains("161") && (lower.contains("drop") || lower.contains("reject")) {
            println!("[WARN] nftables podría estar filtrando SNMP localmente (UDP/161).");
        }
    }

    println!("[INFO] Si el ping está deshabilitado por política, SNMP aún puede funcionar correctamente.\n");
}

async fn test_https_connectivity(endpoint: &str) -> Result<()> {
    let client = Client::builder().timeout(Duration::from_secs(7)).build()?;

    let response = client
        .get(endpoint)
        .send()
        .await
        .with_context(|| format!("No se pudo conectar a {}", endpoint))?;

    let status = response.status();
    if status.is_success() || status.as_u16() == 401 || status.as_u16() == 403 || status.as_u16() == 405 {
        return Ok(());
    }

    let host = Url::parse(endpoint)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| "(host desconocido)".to_string());
    Err(anyhow::anyhow!(
        "Respuesta HTTP {} desde {} (host: {})",
        status,
        endpoint,
        host
    ))
}

fn contains_snmp_drop_rule(rules: &str) -> bool {
    let lower = rules.to_lowercase();
    lower.contains("161") && (lower.contains("drop") || lower.contains("reject"))
}

fn ask_yes_no(prompt: &str) -> Result<bool> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let answer = input.trim().to_lowercase();

    Ok(answer.is_empty() || answer == "y" || answer == "yes" || answer == "s" || answer == "si")
}

fn detect_default_gateway() -> Option<String> {
    let output = run_command_output("ip", &["route", "show", "default"]);
    let parts: Vec<&str> = output.split_whitespace().collect();

    for (idx, part) in parts.iter().enumerate() {
        if *part == "via" && idx + 1 < parts.len() {
            return Some(parts[idx + 1].to_string());
        }
    }
    None
}

fn ping_host(host: &str) -> bool {
    run_command_status("ping", &["-c", "1", "-W", "2", host]).unwrap_or(false)
}

fn detect_cron_service_name() -> Option<String> {
    if !is_command_available("systemctl") {
        return None;
    }

    if run_command_status("systemctl", &["--quiet", "is-active", "cron"]).unwrap_or(false) {
        return Some("cron".to_string());
    }
    if run_command_status("systemctl", &["--quiet", "is-active", "crond"]).unwrap_or(false) {
        return Some("crond".to_string());
    }

    if is_command_available("cron") {
        return Some("cron".to_string());
    }
    if is_command_available("crond") {
        return Some("crond".to_string());
    }

    None
}

fn systemctl_is_enabled(service: &str) -> bool {
    if !is_command_available("systemctl") {
        return false;
    }

    run_command_status("systemctl", &["--quiet", "is-enabled", service]).unwrap_or(false)
}

fn install_cron_by_distro() -> Result<()> {
    if is_command_available("apt-get") {
        run_command_status("apt-get", &["update"])?;
        run_command_status("apt-get", &["install", "-y", "cron"])?;
        return Ok(());
    }

    if is_command_available("dnf") {
        run_command_status("dnf", &["install", "-y", "cronie"])?;
        return Ok(());
    }

    if is_command_available("yum") {
        run_command_status("yum", &["install", "-y", "cronie"])?;
        return Ok(());
    }

    if is_command_available("zypper") {
        run_command_status("zypper", &["-n", "install", "cron"])?;
        return Ok(());
    }

    if is_command_available("pacman") {
        run_command_status("pacman", &["-Sy", "--noconfirm", "cronie"])?;
        return Ok(());
    }

    Err(anyhow::anyhow!("No se detectó gestor de paquetes soportado para instalar cron"))
}

fn is_command_available(cmd: &str) -> bool {
    run_command_status("sh", &["-c", &format!("command -v {} >/dev/null 2>&1", cmd)]).unwrap_or(false)
}

fn run_command_status(program: &str, args: &[&str]) -> Result<bool> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("No se pudo ejecutar {}", program))?;
    Ok(status.success())
}

fn run_command_output(program: &str, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(out) => {
            if out.status.success() {
                String::from_utf8_lossy(&out.stdout).to_string()
            } else {
                String::from_utf8_lossy(&out.stderr).to_string()
            }
        }
        Err(_) => String::new(),
    }
}

/// Coloriza la salida de `systemctl status` para resaltar el indicador de estado
/// y la línea "Active:" con colores según el estado del servicio.
fn colorize_systemctl_output(output: &str) -> String {
    // ANSI color codes
    const GREEN: &str = "\x1b[32m";
    const RED: &str = "\x1b[31m";
    const YELLOW: &str = "\x1b[33m";
    const RESET: &str = "\x1b[0m";

    let mut result = String::with_capacity(output.len() + 128);

    for line in output.lines() {
        if line.starts_with('●') || line.starts_with("●") {
            // Línea del indicador: ● cron.service - ...
            let color = if output.contains("active (running)") {
                GREEN
            } else if output.contains("inactive") || output.contains("dead") {
                RED
            } else {
                YELLOW
            };
            result.push_str(&format!("{}{}{}", color, "●", RESET));
            if let Some(rest) = line.strip_prefix('●') {
                result.push_str(rest);
            }
            result.push('\n');
        } else if line.trim_start().starts_with("Active:") {
            // Línea Active: colorizar el estado
            if line.contains("active (running)") {
                let colored = line.replace(
                    "active (running)",
                    &format!("{}active (running){}", GREEN, RESET),
                );
                result.push_str(&colored);
                result.push('\n');
            } else if line.contains("inactive") {
                let colored = line.replace(
                    "inactive",
                    &format!("{}inactive{}", RED, RESET),
                );
                result.push_str(&colored);
                result.push('\n');
            } else if line.contains("failed") {
                let colored = line.replace(
                    "failed",
                    &format!("{}failed{}", RED, RESET),
                );
                result.push_str(&colored);
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Remover último salto de línea extra
    if result.ends_with('\n') {
        result.pop();
    }
    result
}
