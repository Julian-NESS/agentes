//! Audit runner — Phase 9 (vulnerabilities) + Phase 10 (CIS compliance).
//!
//! These phases only run when `--audit` is passed to the agent **and**
//! `NESS_AUDIT_ENABLED=true` (the opt-in gate — see install_relay.sh).
//!
//! The SSH connection is established against the device's resolved
//! `SshCredentials` (host, port, username, password). On any error the
//! affected phase is logged and skipped; the SNMP payload from the parent
//! `collect_device()` call is **still** processed normally. This is the
//! best-effort contract: audit failures never break the regular collection
//! cycle.

use std::sync::Arc;
use std::time::Duration;
use std::io::IsTerminal;

use crate::ness_relay_core::vendor::{PluginRegistry, VendorFacts, NessVendor};
use crate::ness_relay_core::vulns::{VulnEngine, VulnEngineConfig};
use crate::ness_relay_core::cis::{CisEngine, CisEngineConfig, Check};
use crate::ness_relay_core::ssh::{SshClient, SshTarget, SshCredentials as CoreSshCredentials, AuthMethod};
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

use super::config::{DeviceConfig, SshCredentials};

/// Time budget for the entire SSH collection (connect + vulns + CIS). On
/// Debian-hosted FortiGates, observed connect+collect times are 5-25s for
/// FortiOS v7.6.6.
const SSH_PHASE_TIMEOUT: Duration = Duration::from_secs(120);

/// Default SSH connect timeout (russh handler-level).
const SSH_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Run the two SSH-audit phases for a single device. Returns a `serde_json::Value`
/// with at most two top-level keys (`vulnerabilities`, `cis_compliance`).
///
/// Returns `Ok(None)` when there is nothing to do — either the device has no
/// SSH credentials configured, or no vendor plugin supports the configured
/// vendor. The caller (engine) treats this the same as an empty Ok.
pub async fn run_audit_phases(
    device: &DeviceConfig,
    registry: Arc<PluginRegistry>,
) -> anyhow::Result<Option<Value>> {
    // ────────────────────────────────────────────────────────
    // Test mode: si NESS_AUDIT_FAKE_DATA=true, devolver un payload de
    // audit ficticio sin intentar SSH. Útil para validar el flujo end-to-end
    // de subcarpetas (vulnerabilities/, cis_compliance/) sin necesidad de
    // un FortiGate real.
    // ────────────────────────────────────────────────────────
    if std::env::var("NESS_AUDIT_FAKE_DATA")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        info!(
            target: "ness_relay::audit",
            "[{}] NESS_AUDIT_FAKE_DATA=true — emitiendo audit ficticio (sin SSH)",
            device.device_id,
        );
        return Ok(Some(json!({
            "vulnerabilities": {
                "schema": "ness-relay/vulnerabilities/v1",
                "vendor": "fortinet",
                "device_hostname": &device.description,
                "cpe": "cpe:2.3:o:fortinet:fortios:7.6.6:*:*:*:*:*:*:*",
                "started_at": chrono::Utc::now().to_rfc3339(),
                "finished_at": chrono::Utc::now().to_rfc3339(),
                "duration_ms": 123,
                "is_clean": false,
                "has_kev": true,
                "counts": { "total": 2, "critical": 1, "high": 1, "medium": 0, "low": 0, "info": 0, "kev_critical": 1 },
                "findings": [
                    {
                        "cve_id": "CVE-2025-31514",
                        "title": "FortiOS SSL-VPN buffer overflow",
                        "cvss_v3": 9.8,
                        "cvss_v2": 0.0,
                        "severity": "critical",
                        "kev": true,
                        "kev_due_date": null,
                        "epss": 0.71,
                        "epss_percentile": 95.0,
                        "summary": "Heap overflow in SSL-VPN handler.",
                        "affected": "fortios 7.6.6",
                        "remediation": "Upgrade to FortiOS 7.6.7 or later.",
                        "references": ["https://fortiguard.fortinet.com/psirt/FG-IR-25-315"],
                        "found_at": chrono::Utc::now().to_rfc3339(),
                        "false_positive_reason": "Versión del dispositivo (7.6.6) es estrictamente mayor que todas las versiones afectadas listadas (['7.6']). El CVE probablemente ya está parcheado en tu versión. Verifica con FortiGuard PSIRT: revisar el advisory en references.",
                        "psirt_confirmed": true,
                        "psirt_url": "https://fortiguard.fortinet.com/psirt/FG-IR-25-315",
                    },
                    {
                        "cve_id": "CVE-2025-54821",
                        "title": "FortiOS admin web XSS",
                        "cvss_v3": 7.5,
                        "cvss_v2": 0.0,
                        "severity": "high",
                        "kev": false,
                        "kev_due_date": null,
                        "epss": 0.18,
                        "epss_percentile": 78.0,
                        "summary": "Stored XSS in admin dashboard.",
                        "affected": "fortios 7.6.6",
                        "remediation": "Apply patch from vendor.",
                        "references": ["https://fortiguard.fortinet.com/psirt/FG-IR-25-548"],
                        "found_at": chrono::Utc::now().to_rfc3339(),
                        // Phase 2.13: incluir los nuevos campos también en fake_data
                        // para que los tests E2E verifiquen que el JSON incluye
                        // la metadata de falsos positivos y PSIRT.
                        "false_positive_reason": "Versión del dispositivo (7.6.6) es estrictamente mayor que todas las versiones afectadas listadas (['7.6']). El CVE probablemente ya está parcheado en tu versión. Verifica con FortiGuard PSIRT: revisar el advisory en references.",
                        "psirt_confirmed": true,
                        "psirt_url": "https://fortiguard.fortinet.com/psirt/FG-IR-25-548",
                    }
                ],
            },
            "cis_compliance": {
                "schema": "ness-relay/cis-compliance/v1",
                "vendor": "fortinet",
                "device_hostname": &device.description,
                "started_at": chrono::Utc::now().to_rfc3339(),
                "finished_at": chrono::Utc::now().to_rfc3339(),
                "duration_ms": 250,
                "total_checks": 16,
                "passed": 4,
                "failed": 11,
                "manual": 1,
                "errors": 0,
                "compliance_score": 25,
                "is_clean": false,
                "has_critical_failures": true,
                "findings": [
                    { "cis_id": "fortios-1.1.1", "title": "Login banner configured",
                      "compliance_status": "Pass", "finding_type": "technical",
                      "severity": "medium", "current_value": "configured",
                      "expected_value": "configured", "remediation": "—",
                      "cve_ids": [], "raw_evidence": "config system global ... set pre-login-banner enable",
                      "checked_at": chrono::Utc::now().to_rfc3339(),
                      "check_duration_ms": 5 },
                    { "cis_id": "fortios-1.2.1", "title": "HTTPS admin enabled",
                      "compliance_status": "Fail", "finding_type": "technical",
                      "severity": "high", "current_value": "disabled",
                      "expected_value": "enabled", "remediation": "config system global ... set admin-https-redirect enable",
                      "cve_ids": [], "raw_evidence": "—",
                      "checked_at": chrono::Utc::now().to_rfc3339(),
                      "check_duration_ms": 3 },
                ],
            },
        })));
    }

    // Resolve SSH credentials from the env. If they are not available we
    // return Ok(None) — the regular SNMP pipeline runs unaffected.
    let creds = match device.ssh_credentials() {
        Some(c) => c,
        None => {
            // Cambio Phase 2.4: ahora reportamos la RAZÓN EXACTA por la cual
            // las credenciales no están disponibles. Esto ayuda al operador
            // a diagnosticar si el problema es:
            //   - Falta configurar ssh_enabled=true
            //   - Falta ssh_username
            //   - Falta ssh_password_env
            //   - La variable de entorno no está seteada en el proceso
            //   - El archivo secrets.env no se cargó
            let reason = crate::core::config::ssh_unavailable_reason(device);
            let env_var_name = device.ssh_password_env.as_deref().unwrap_or("<unset>");
            info!(
                target: "ness_relay::audit",
                "[{}] SSH audit omitido — razón: {} (ssh_password_env='{}', \
                 ¿se cargó /etc/ness_relay/secrets.env?)",
                device.device_id,
                reason,
                env_var_name,
            );
            return Ok(None);
        }
    };

    // For Phase 1 we only wire Fortinet end-to-end. Other vendors go through
    // the standard plugin registry once we validate Fortinet in production.
    if !is_fortinet_vendor(&device.vendor) {
        info!(
            target: "ness_relay::audit",
            "[{}] SSH audit no soportado para vendor={} (solo Fortinet en Phase 1)",
            device.device_id,
            device.vendor
        );
        return Ok(None);
    }

    // Outer timeout — defensive, prevents hangs on unreachable firewalls.
    let audit_result = tokio::time::timeout(
        SSH_PHASE_TIMEOUT,
        run_audit_phases_inner(device, &creds, registry),
    )
    .await;

    match audit_result {
        Ok(Ok(value)) => Ok(Some(value)),
        Ok(Err(e)) => {
            warn!(
                target: "ness_relay::audit",
                "[{}] audit phases failed: {e:#} (continuando con SNMP normal)",
                device.device_id,
            );
            Ok(None)
        }
        Err(_elapsed) => {
            warn!(
                target: "ness_relay::audit",
                "[{}] audit phases timed out after {}s",
                device.device_id,
                SSH_PHASE_TIMEOUT.as_secs(),
            );
            Ok(None)
        }
    }
}

/// Inner helper that actually opens the SSH session and runs the phases.
async fn run_audit_phases_inner(
    device: &DeviceConfig,
    creds: &SshCredentials,
    registry: Arc<PluginRegistry>,
) -> anyhow::Result<Value> {
    let ssh_client = SshClient::new()
        .with_timeout(SSH_CONNECT_TIMEOUT);
    let target = SshTarget::new(
        device.device_id.clone(),
        creds.host.clone(),
        creds.port,
    );
    let core_creds = CoreSshCredentials {
        username: creds.username.clone(),
        auth: AuthMethod::Password(creds.password.clone()),
    };

    info!(
        target: "ness_relay::audit",
        "[{}] Abriendo sesión SSH {} (user={}, target={}:{})",
        device.device_id,
        creds.describe(),
        creds.username,
        creds.host,
        creds.port,
    );

    let session = ssh_client
        .connect(&target, &device.device_id, &core_creds)
        .await
        .map_err(|e| anyhow::anyhow!("SSH connect failed: {e:#}"))?;

    // Resolve the Fortinet plugin via the shared registry. We use the
    // NessVendor enum for type safety; for Phase 1 we accept only Fortinet.
    let plugin = registry
        .get(NessVendor::Fortinet)
        .ok_or_else(|| anyhow::anyhow!("Fortinet plugin not registered"))?;

    // ---- Phase 7-equivalent: SSH facts collection ------------------------
    let facts: VendorFacts = plugin
        .collect(&ssh_client, &session)
        .await
        .map_err(|e| anyhow::anyhow!("Fortinet collect failed: {e:#}"))?;

    // ---- Phase 9: vulnerability scan ------------------------------------
    let vuln_engine = VulnEngine::new().with_config(VulnEngineConfig::default());
    let vuln_report = match vuln_engine.run(&facts).await {
        Ok(r) => r,
        Err(e) => {
            warn!(
                target: "ness_relay::audit",
                "[{}] vulnerability scan failed: {e:#}",
                device.device_id,
            );
            // Emit an empty report so the schema is stable.
            crate::ness_relay_core::vulns::build_report(
                NessVendor::Fortinet,
                &facts.device_hostname,
                crate::ness_relay_core::vulns::build_cpe_from_facts(&facts).unwrap_or_default(),
                chrono::Utc::now(),
                vec![],
            )
        }
    };
    let vuln_block = serialize_vuln_report(&vuln_report);

    // ---- Phase 10: CIS benchmark scan ------------------------------------
    let cis_engine = CisEngine::with_config(CisEngineConfig { concurrency: 4 });
    let cis_checks: Vec<Arc<dyn Check>> = crate::ness_relay_core::cis::all_fortios_checks();
    let cis_report = match cis_engine.run(NessVendor::Fortinet, &facts, &cis_checks).await {
        Ok(r) => r,
        Err(e) => {
            warn!(
                target: "ness_relay::audit",
                "[{}] CIS scan failed: {e:#}",
                device.device_id,
            );
            crate::ness_relay_core::cis::Report {
                vendor: NessVendor::Fortinet,
                device_hostname: facts.device_hostname.clone(),
                started_at: chrono::Utc::now(),
                finished_at: chrono::Utc::now(),
                duration_ms: 0,
                findings: vec![],
                total_checks: cis_checks.len(),
                passed: 0,
                failed: 0,
                manual: 0,
                errors: 0,
            }
        }
    };
    let cis_block = serialize_cis_report(&cis_report);

    // Best-effort SSH close (errors here don't break the audit)
    let _ = ssh_client.disconnect(session).await;

    // Cambio Phase 2.4 — audit_local_first: imprimir resumen legible en consola
    // similar al que produce ness-sentinel (`=== RESUMEN ===`). Es best-effort:
    // solo se imprime si la salida es interactiva (no cuando --silent está
    // activo o el stdout no es un TTY).
    print_audit_summary(
        &device.device_id,
        &facts.device_hostname,
        &vuln_block,
        &cis_block,
    );

    Ok(json!({
        "vulnerabilities": vuln_block,
        "cis_compliance": cis_block,
    }))
}

/// Imprime un resumen legible de la auditoría en consola. Pensado para que
/// el operador vea en pantalla lo recolectado sin tener que abrir el JSON.
fn print_audit_summary(
    device_id: &str,
    hostname: &str,
    vuln_block: &Value,
    cis_block: &Value,
) {
    // Phase 2.13: SIEMPRE imprime el resumen (no solo si es TTY).
    // El operador debe ver este resumen tanto en modo interactivo como
    // en modo cron. Los códigos ANSI se omiten automáticamente si NO es TTY.
    let stdout_is_tty = std::io::stdout().is_terminal();

    let cyan = if stdout_is_tty { "\x1b[36m" } else { "" };
    let yellow = "\x1b[33m";
    let green = if stdout_is_tty { "\x1b[32m" } else { "" };
    let red = if stdout_is_tty { "\x1b[31m" } else { "" };
    let yellow = if stdout_is_tty { "\x1b[33m" } else { "" };
    let bold = if stdout_is_tty { "\x1b[1m" } else { "" };
    let dim = if stdout_is_tty { "\x1b[2m" } else { "" };
    let reset = if stdout_is_tty { "\x1b[0m" } else { "" };

    let host_display = if hostname.is_empty() {
        device_id.to_string()
    } else {
        hostname.to_string()
    };

    // Vulnerabilidades
    let vuln_findings = vuln_block
        .get("findings")
        .and_then(|f| f.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let vuln_critical = vuln_block
        .get("counts")
        .and_then(|c| c.get("critical"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let vuln_high = vuln_block
        .get("counts")
        .and_then(|c| c.get("high"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let vuln_kev = vuln_block
        .get("counts")
        .and_then(|c| c.get("kev_critical"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let has_kev = vuln_block
        .get("has_kev")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // CIS
    let cis_total = cis_block
        .get("total_checks")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cis_passed = cis_block
        .get("passed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cis_failed = cis_block
        .get("failed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cis_manual = cis_block
        .get("manual")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cis_score = cis_block
        .get("compliance_score")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cis_critical_failures = cis_block
        .get("has_critical_failures")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Listar los IDs de CVE (primeros 8 para no saturar la pantalla)
    let mut vuln_ids: Vec<String> = Vec::new();
    if let Some(findings) = vuln_block.get("findings").and_then(|f| f.as_array()) {
        for f in findings.iter().take(8) {
            if let Some(id) = f.get("cve_id").and_then(|v| v.as_str()) {
                vuln_ids.push(id.to_string());
            }
        }
    }
    let vuln_ids_str = if vuln_ids.is_empty() {
        "—".to_string()
    } else {
        vuln_ids.join(", ")
    };

    println!();
    println!("{}{}═══════════════════════════════════════════════════════════════{}", cyan, bold, reset);
    println!("{}{}  RESUMEN DE AUDITORÍA — {} ({}){}", cyan, bold, host_display, device_id, reset);
    println!("{}{}═══════════════════════════════════════════════════════════════{}", cyan, bold, reset);
    println!();

    // Bloque vulnerabilidades
    println!("{}  ▸ Vulnerabilidades (Fase 9):{}", yellow, reset);
    if vuln_findings == 0 {
        println!("{}    ✓ Sin vulnerabilidades conocidas reportadas.{}", green, reset);
    } else {
        let crit_color = if vuln_critical > 0 { red } else { yellow };
        println!(
            "{}    {} CVEs: {} críticas, {} altas, {} KEV{}",
            crit_color,
            vuln_findings,
            vuln_critical,
            vuln_high,
            vuln_kev,
            reset,
        );
        println!("{}    IDs: {}{}", dim, vuln_ids_str, reset);
        if has_kev {
            println!(
                "{}    ⚠ Hay CVEs en el catálogo CISA KEV (explotados activamente).{}",
                red, reset,
            );
        }

        // Phase 2.12: contar CVEs marcadas como potencialmente parcheadas
        // y mostrarlo destacado en el resumen. Esto le avisa al operador
        // que probablemente no debe alarmarse.
        let potentially_patched = vuln_block
            .get("findings")
            .and_then(|f| f.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|cve| {
                        cve.get("false_positive_reason")
                            .and_then(|r| r.as_str())
                            .map(|s| !s.is_empty())
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);
        if potentially_patched > 0 {
            println!(
                "{}    ⚠ {} CVE(s) marcadas como POTENCIALMENTE PARCHEADAS en tu versión.{}{}",
                yellow, potentially_patched, dim, reset
            );
            println!(
                "{}      Verificar manualmente en: /opt/ness_relay/devices/*/output/vulnerabilities/relay_data.json{}",
                dim, reset
            );
        }

        // Phase 2.12: mostrar info de PSIRT (doble validación) si está disponible
        let psirt_count = vuln_block
            .get("findings")
            .and_then(|f| f.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|cve| {
                        cve.get("psirt_confirmed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);
        if psirt_count > 0 {
            println!(
                "{}    ✓ {} CVE(s) confirmadas por FortiGuard PSIRT (doble validación){}",
                green, psirt_count, reset
            );
        }
    }

    println!();
    // Bloque CIS
    println!("{}  ▸ Compliance CIS (Fase 10):{}", yellow, reset);
    if cis_total == 0 {
        println!("{}    • Sin chequeos ejecutados.{}", dim, reset);
    } else {
        let score_color = if cis_score >= 80 {
            green
        } else if cis_score >= 50 {
            yellow
        } else {
            red
        };
        println!(
            "{}    Score: {}%{}  ({}/{} pasaron, {} fallaron, {} manuales)",
            score_color, cis_score, reset, cis_passed, cis_total, cis_failed, cis_manual,
        );
        if cis_critical_failures {
            println!(
                "{}    ⚠ Hay fallos críticos de compliance. Revisar con prioridad.{}",
                red, reset,
            );
        }
    }

    println!();
    println!("{}{}  Archivos generados:{}", dim, bold, reset);
    // Phase 2.16: nombres de archivo estandarizados por tipo de telemetría.
    // Cada archivo vive dentro de su propia subcarpeta para evitar confusión.
    println!(
        "{}    • /opt/ness_relay/devices/firewall_{}/output/snmp/relay_snmp_data.json (telemetría SNMP){}",
        dim, host_display, reset,
    );
    println!(
        "{}    • /opt/ness_relay/devices/firewall_{}/output/vulnerabilities/relay_sentinel_vulnerabilities_data.json (Fase 9){}",
        dim, host_display, reset,
    );
    println!(
        "{}    • /opt/ness_relay/devices/firewall_{}/output/cis_compliance/relay_sentinel_cis_data.json (Fase 10){}",
        dim, host_display, reset,
    );

    // Aviso de modo local
    let local_only = std::env::var("NESS_AUDIT_LOCAL_ONLY")
        .map(|v| v.eq_ignore_ascii_case("true") || v.trim() == "1")
        .unwrap_or(false);
    if local_only {
        println!();
        println!(
            "{}  ℹ NESS_AUDIT_LOCAL_ONLY=true: el envío al servidor NESS HQ fue omitido.{}",
            yellow, reset,
        );
        println!(
            "{}    Para subir los hallazgos a la nube, ejecute audit_relay.sh sin esa variable.{}",
            dim, reset,
        );
    }

    println!();
    println!("{}{}═══════════════════════════════════════════════════════════════{}", cyan, bold, reset);
    println!();
}

/// Convert `VulnerabilityReport` → JSON. The struct has a `vendor: Vendor`
/// field that doesn't have a stable string representation in this crate; we
/// always emit `"fortinet"` since we only run audits for Fortinet in Phase 1.
fn serialize_vuln_report(report: &crate::ness_relay_core::vulns::VulnerabilityReport) -> Value {
    let mut findings = Vec::with_capacity(report.findings.len());
    for f in &report.findings {
        // Phase 2.14: incluir los campos `false_positive_reason`, `psirt_confirmed`
        // y `psirt_url` al reconstruir el JSON. Si los omitimos, el operador
        // nunca ve en disco las anotaciones que `is_cve_potentially_patched()`
        // y la validación cruzada con FortiGuard PSIRT ya calcularon, y
        // `print_audit_summary()` no puede contarlas para emitir los mensajes
        // "POTENCIALMENTE PARCHEADAS" y "confirmadas por FortiGuard PSIRT".
        //
        // Phase 2.19 BUGFIX: `kev_due_date` debe serializarse como `YYYY-MM-DD`
        // (DateField en el modelo Django del servidor), NO como RFC3339.
        // El servidor rechaza con HTTP 500 si recibe `2022-09-29T00:00:00+00:00`
        // porque el modelo espera solo fecha. `chrono::NaiveDate` sin hora
        // produce el formato correcto.
        let kev_due_date_str = f.kev_due_date.map(|d| d.format("%Y-%m-%d").to_string());
        findings.push(json!({
            "cve_id": f.cve_id,
            "title": f.title,
            "cvss_v3": f.cvss_v3,
            "cvss_v2": f.cvss_v2,
            "severity": format!("{:?}", f.severity).to_lowercase(),
            "kev": f.kev,
            "kev_due_date": kev_due_date_str,
            "epss": f.epss,
            "epss_percentile": f.epss_percentile,
            "summary": f.summary,
            "affected": f.affected,
            "remediation": f.remediation,
            "references": f.references,
            "found_at": f.found_at.to_rfc3339(),
            "false_positive_reason": f.false_positive_reason,
            "psirt_confirmed": f.psirt_confirmed,
            "psirt_url": f.psirt_url,
        }));
    }
    json!({
        "schema": "ness-relay/vulnerabilities/v1",
        "vendor": report.vendor.slug(),
        "device_hostname": report.device_hostname,
        "cpe": report.cpe,
        "started_at": report.started_at.to_rfc3339(),
        "finished_at": report.finished_at.to_rfc3339(),
        "duration_ms": report.duration_ms,
        "is_clean": report.is_clean(),
        "has_kev": report.has_kev(),
        "counts": {
            "total": report.counts.total,
            "critical": report.counts.critical,
            "high": report.counts.high,
            "medium": report.counts.medium,
            "low": report.counts.low,
            "info": report.counts.info,
            "kev_critical": report.counts.kev_critical,
        },
        "findings": findings,
    })
}

/// Convert `cis::Report` → JSON.
fn serialize_cis_report(report: &crate::ness_relay_core::cis::Report) -> Value {
    let mut findings = Vec::with_capacity(report.findings.len());
    for f in &report.findings {
        findings.push(json!({
            "cis_id": f.cis_id,
            "title": f.title,
            "compliance_status": format!("{:?}", f.compliance_status),
            "finding_type": format!("{:?}", f.finding_type).to_snake_case(),
            "severity": format!("{:?}", f.severity).to_lowercase(),
            "current_value": f.current_value,
            "expected_value": f.expected_value,
            "remediation": f.remediation,
            "cve_ids": f.cve_ids,
            "raw_evidence": f.raw_evidence,
            "checked_at": f.checked_at.to_rfc3339(),
            "check_duration_ms": f.check_duration_ms,
        }));
    }
    json!({
        "schema": "ness-relay/cis-compliance/v1",
        "vendor": report.vendor.slug(),
        "device_hostname": report.device_hostname,
        "started_at": report.started_at.to_rfc3339(),
        "finished_at": report.finished_at.to_rfc3339(),
        "duration_ms": report.duration_ms,
        "total_checks": report.total_checks,
        "passed": report.passed,
        "failed": report.failed,
        "manual": report.manual,
        "errors": report.errors,
        "compliance_score": report.compliance_score(),
        "is_clean": report.is_clean(),
        "has_critical_failures": report.has_critical_failures(),
        "findings": findings,
    })
}

/// Vendor slug gate. Only Fortinet is wired end-to-end in Phase 1; others
/// return early without attempting the SSH connection.
fn is_fortinet_vendor(vendor_slug: &str) -> bool {
    matches!(vendor_slug.to_ascii_lowercase().as_str(), "fortinet" | "fortigate" | "fortios")
}

/// Tiny snake_case helper for `finding_type` enum rendering. Avoids pulling
/// in heck or similar crates.
trait SnakeCaseExt {
    fn to_snake_case(&self) -> String;
}
impl SnakeCaseExt for str {
    fn to_snake_case(&self) -> String {
        let mut out = String::with_capacity(self.len() + 2);
        let mut prev_lower = false;
        for c in self.chars() {
            if c.is_ascii_uppercase() {
                if prev_lower {
                    out.push('_');
                }
                out.push(c.to_ascii_lowercase());
                prev_lower = false;
            } else if c == ' ' || c == '-' {
                out.push('_');
                prev_lower = false;
            } else {
                out.push(c);
                prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            }
        }
        out
    }
}

// Suppress unused-import warnings for things only used behind feature gates.
#[allow(dead_code)]
const _UNUSED: fn() = || {
    let _ = Uuid::new_v4;
};
