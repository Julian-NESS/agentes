# ness-relay v2.4.0 — Bugs descubiertos en test con FortiGate real (2026-07-03)

## Bugs reportados por el operador

### Bug A — Numeración de fases incorrecta
- **Síntoma**: Los logs mostraban `[1/8] ... [8/8]` cuando el instalador ejecuta `--audit` (no era el caso, era el modo normal)
- **Causa raíz**: El log mostrado en el reporte fue de la "PRUEBA DE CONFIGURACIÓN" que NO usa `--audit`. El audit solo corre cada 6h vía `audit_relay.sh`.
- **Fix**: Numeración dinámica en `engine.rs`:
  ```rust
  let total_phases: u8 = if audit_mode { 10 } else { 8 };
  ```
- **Estado**: ✅ Aplicado

### Bug B — NESS_AUDIT_ENABLED quedaba en `false` pese a responder Y
- **Síntoma**: El operador respondió Y al opt-in pero el env file tenía `NESS_AUDIT_ENABLED="false"`
- **Causa raíz**: El orden de operaciones era:
  1. `prompt_audit_optin()` — setea `ENABLE_AUDIT` localmente dentro de la función (no en el scope global)
  2. El env file se escribía con `${ENABLE_AUDIT:-false}` → usaba el default `false`
- **Fix**:
  - Inicializar `ENABLE_AUDIT="${ENABLE_AUDIT:-false}"` como variable global antes de la función
  - Cambiar `prompt_audit_optin()` para devolver el resultado vía stdout: `echo "true"` o `echo "false"`
  - El caller captura con `ENABLE_AUDIT=$(prompt_audit_optin)`
  - Después del prompt, se hace `sed -i` sobre el env file para corregir el valor
- **Estado**: ✅ Aplicado

### Bug C — La contraseña SSH nunca se pidió al usuario
- **Síntoma**: El operador no entendía cómo se establece la conexión SSH real
- **Causa raíz**: El prompt original solo pedía el *nombre de la variable*, no la contraseña real
- **Fix**: Nuevo prompt que pregunta la contraseña directamente con `read -s`:
  ```
  🔑 Contraseña SSH para admin@192.168.10.17 (Enter para omitir):
  ```
  - Si se ingresa: se guarda en `/etc/ness_relay/secrets.env` con chmod 600
  - Si se omite: muestra el comando exacto para hacerlo manualmente después
- **Estado**: ✅ Aplicado

## Cambios complementarios

### `audit_relay.sh` carga secrets automáticamente
Antes, `audit_relay.sh` solo hacía `source /etc/profile.d/ness_relay.sh`. Ahora también carga `/etc/ness_relay/secrets.env` si existe:
```bash
SECRETS_FILE="/etc/ness_relay/secrets.env"
if [[ -f "$SECRETS_FILE" ]]; then
    source "$SECRETS_FILE"
fi
```

### Helper `escape_sed_replacement`
Para escapar caracteres especiales (`\`, `&`, `|`) en passwords antes de hacer `sed -i` sobre el env file.

## Validación E2E

```bash
NESS_AUDIT_ENABLED=true NESS_AUDIT_FAKE_DATA=true \
  ./dist/ness-relay-x86_64 --audit --silent \
  --config /tmp/test/configs/connection.config
```

Resultado:
- `/tmp/test/devices/firewall_fortinet/output/relay_data.json` (SNMP)
- `/tmp/test/devices/firewall_fortinet/output/vulnerabilities/relay_data.json` (2 CVEs)
- `/tmp/test/devices/firewall_fortinet/output/cis_compliance/relay_data.json` (16 checks)

Log muestra `[1/10]` ... `[10/10]` correctamente.

## Hash actual del binario

`ce43ab6a64693e8c21bd782da70f5522` (sin cambios desde el último build porque solo se modificó bash)

## Archivos modificados en este round

| Archivo | Cambio |
|---|---|
| `scripts/install_relay.sh` | `prompt_audit_optin()` ahora imprime `true`/`false` por stdout; orden de operaciones corregido (prompt → sed -i env file); nuevo prompt con `read -s` para password; helper `escape_sed_replacement`; `audit_relay.sh` carga `secrets.env` |

## Lo que el usuario debe probar en su VM Debian

```bash
# 1. Re-correr instalador
cd /home/nessdeployer/rust-sentinel/
sudo ./install_relay.sh
# Responde Y al prompt audit; ingresa password cuando se pida

# 2. Verificar el env file
sudo cat /etc/profile.d/ness_relay.sh | grep NESS_AUDIT_ENABLED
# Debe mostrar: export NESS_AUDIT_ENABLED="true"

# 3. Verificar secrets
sudo ls -la /etc/ness_relay/
# Debe mostrar secrets.env con permisos -rw-------

# 4. Probar audit manualmente (sin esperar 6h)
sudo NESS_AUDIT_ENABLED=true \
  /opt/ness_relay/executables/ness-relay-x86_64 \
  --audit --silent \
  --config /opt/ness_relay/configs/connection.config

# 5. Verificar subcarpetas
ls /opt/ness_relay/devices/firewall_fortinet/output/
# Debe mostrar: relay_data.json  vulnerabilities/  cis_compliance/
```

## Lecciones aprendidas

1. **Subshells con `source <(awk ...)`** son tricky: las funciones definidas dentro NO comparten scope con el shell padre. Patrón recomendado: usar stdout para devolver valores.
2. **Orden de operaciones** es crítico: si una variable se setea en un paso, los pasos siguientes que dependen de ella DEBEN ir después.
3. **UX importa**: el operador no debe adivinar cómo se carga la password SSH. Pedirla directamente + guardarla en archivo seguro es la mejor experiencia.
4. **El binario no es interactivo** por diseño — los prompts van en el instalador, no en el agente.