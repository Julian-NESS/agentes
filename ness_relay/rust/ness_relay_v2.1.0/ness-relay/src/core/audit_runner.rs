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
    // Resolve SSH credentials from the env. If they are not available we
    // return Ok(None) — the regular SNMP pipeline runs unaffected.
    let creds = match device.ssh_credentials() {
        Some(c) => c,
        None => {
            info!(
                target: "ness_relay::audit",
                "[{}] SSH audit omitido (sin credenciales SSH configuradas o env var no disponible)",
                device.device_id
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

    Ok(json!({
        "vulnerabilities": vuln_block,
        "cis_compliance": cis_block,
    }))
}

/// Convert `VulnerabilityReport` → JSON. The struct has a `vendor: Vendor`
/// field that doesn't have a stable string representation in this crate; we
/// always emit `"fortinet"` since we only run audits for Fortinet in Phase 1.
fn serialize_vuln_report(report: &crate::ness_relay_core::vulns::VulnerabilityReport) -> Value {
    let mut findings = Vec::with_capacity(report.findings.len());
    for f in &report.findings {
        findings.push(json!({
            "cve_id": f.cve_id,
            "title": f.title,
            "cvss_v3": f.cvss_v3,
            "cvss_v2": f.cvss_v2,
            "severity": format!("{:?}", f.severity).to_lowercase(),
            "kev": f.kev,
            "kev_due_date": f.kev_due_date.map(|d| d.to_rfc3339()),
            "epss": f.epss,
            "epss_percentile": f.epss_percentile,
            "summary": f.summary,
            "affected": f.affected,
            "remediation": f.remediation,
            "references": f.references,
            "found_at": f.found_at.to_rfc3339(),
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
