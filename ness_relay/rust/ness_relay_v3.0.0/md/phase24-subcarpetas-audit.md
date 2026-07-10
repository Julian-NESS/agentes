# ness-relay v2.4.0 — Subcarpetas para audit data (Phase 2.4)

## Contexto
Antes, el JSON del audit (vulns + CIS) se mezclaba con el JSON SNMP en el mismo archivo `relay_data.json`. Esto dificultaba:
1. Comparar con ness-sentinel (que escribe cada reporte por separado)
2. Validar localmente solo el bloque audit
3. Limpiar el JSON para envío al servidor (requería un gate `NESS_SEND_VULNERABILITIES`)

## Solución
Subcarpetas separadas dentro de `devices/<device_type>_<vendor>/output/`:

```
output/
├── relay_data.json              ← telemetría SNMP (fases 1-8)
├── vulnerabilities/
│   └── relay_data.json          ← fase 9 (vulns + NVD/KEV/EPSS)
└── cis_compliance/
    └── relay_data.json          ← fase 10 (CIS)
```

## Schema versioning

| Subcarpeta | Schema string | Campos principales |
|---|---|---|
| `output/vulnerabilities/relay_data.json` | `ness-relay/vulnerabilities/v1` | `vendor`, `device_hostname`, `cpe`, `started_at`, `finished_at`, `duration_ms`, `counts: {total, critical, high, medium, low, info, kev_critical}`, `findings: [{cve_id, title, cvss_v3, severity, kev, kev_due_date, epss, epss_percentile, summary, affected, remediation, references, found_at}]` |
| `output/cis_compliance/relay_data.json` | `ness-relay/cis-compliance/v1` | `vendor`, `device_hostname`, `started_at`, `finished_at`, `duration_ms`, `total_checks`, `passed`, `failed`, `manual`, `errors`, `compliance_score`, `findings: [{cis_id, title, compliance_status, finding_type, severity, current_value, expected_value, remediation, cve_ids, raw_evidence, checked_at, check_duration_ms}]` |

## Implementación

### Binario (Rust)

**[engine.rs](agentes/ness_relay/rust/ness_relay_v2.1.0/ness-relay/src/core/engine.rs) — extracción ANTES de `transform_for_server`**

```rust
let mut raw_payload = raw_payload;
let vulns_block = raw_payload.as_object_mut()
    .and_then(|m| m.remove("vulnerabilities"));
let cis_block = raw_payload.as_object_mut()
    .and_then(|m| m.remove("cis_compliance"));

let payload = payload_compat::transform_for_server(raw_payload);
// ... luego exporta cada bloque a su subcarpeta
```

**Orden crítico**: extraer bloques ANTES de `transform_for_server` (que los strippea por el gate `NESS_SEND_VULNERABILITIES`).

### Numeración dinámica de fases

```rust
let total_phases: u8 = if audit_mode { 10 } else { 8 };
info!("[{}] [1/{}] Perfil cargado: ...", device.device_id, total_phases);
```

- Modo SNMP normal: `[1/8]` ... `[8/8]`
- Modo audit: `[1/10]` ... `[10/10]`

### Prompt SSH más claro (Bash)

Antes (ambiguo):
```
🔑 Nombre de la variable de entorno para la contraseña [default: NESS_SSH_PASSWORD_fortinet_1]:
```

Ahora (explícito):
```
El agente NUNCA almacena la contraseña SSH en el archivo
de configuración. Solo se guarda el NOMBRE de la variable
de entorno que el operador exporta en su shell.

🔑 Nombre de la env var (NO la contraseña) [default: NESS_SSH_PASSWORD_fortinet_1]:
...
✓ Credenciales SSH configuradas para 192.168.10.17
  Ahora exporte la contraseña en su shell:
    export NESS_SSH_PASSWORD_fortinet_1='SU_PASSWORD_AQUI'
  O añádala a ~/.bashrc para que persista entre sesiones.
```

## Variable NESS_AUDIT_FAKE_DATA

Variable de entorno para testing que omite el intento SSH y emite datos ficticios:

```bash
NESS_AUDIT_FAKE_DATA=true NESS_AUDIT_ENABLED=true ./ness-relay --audit
# Emite: 2 CVEs (CVE-2025-31514, CVE-2025-54821) + 2 hallazgos CIS
```

Útil para:
- Validar el flujo end-to-end sin necesidad de un FortiGate real
- Pruebas de integración del pipeline de subcarpetas
- Debugging del schema

## Validación E2E

```bash
NESS_AUDIT_ENABLED=true NESS_AUDIT_FAKE_DATA=true \
  ./dist/ness-relay-x86_64 --audit --silent \
  --config /tmp/test-subfolder2/configs/connection.config
```

Output:
```
/tmp/test-subfolder2/devices/firewall_fortinet/output/
├── relay_data.json              ← solo metadata (audit_only=true)
├── vulnerabilities/
│   └── relay_data.json          ← Schema: ness-relay/vulnerabilities/v1
│                                  CVE IDs: ['CVE-2025-31514', 'CVE-2025-54821']
│                                  Counts: {critical: 1, high: 1, ...}
└── cis_compliance/
    └── relay_data.json          ← Schema: ness-relay/cis-compliance/v1
                                   Total: 16, Pass: 4, Fail: 11, Manual: 1
                                   Score: 25%
```

Log:
```
[fortinet_1] vulnerabilities escritas en .../output/vulnerabilities/relay_data.json (2 CVEs)
[fortinet_1] cis_compliance escrito en .../output/cis_compliance/relay_data.json (2 hallazgos)
```

## Hashes del binario (evolución)

| Cambio | Hash |
|---|---|
| v2.4.0 inicial (bridge + --probe) | `b27c894405561b1c8cd3ff215e28494d` |
| + fake data + numeración dinámica | `4105b9f64a35880f80deeb735a675acf` |
| + subcarpetas separadas | `ce43ab6a64693e8c21bd782da70f5522` |

## Próximos pasos

1. Validar en VM Debian con FortiGate real que las 2 CVEs (CVE-2025-31514, CVE-2025-54821) y los 12 hallazgos CIS aparecen en los archivos correctos
2. Comparar los JSON generados por ness-relay --audit contra ness-sentinel scan (deben coincidir)
3. Cuando se valide, quitar `NESS_AUDIT_FAKE_DATA` del binario o dejarlo gated tras flag build-time