# Plan de Implementación — Escaneo de Vulnerabilidades y Controles CIS en "Múltiples Dispositivos"

> **Objetivo**: Llevar la funcionalidad de auditoría (Fase 9: vulnerabilidades + Fase 10: CIS vía SSH) que ya existe en el modo "Un dispositivo" al modo "Múltiples dispositivos" del wizard de instalación guiada de NESS Relay.
>
> **Decisiones de diseño aprobadas**:
> 1. Las 4 columnas nuevas de la plantilla Excel van **siempre visibles** (vacías si no aplican).
> 2. El intervalo de auditoría es **global** (un único dropdown en el wizard, igual que en "Un dispositivo").
> 3. **Validar y rechazar** en el backend si `E-V-CIS=x` pero faltan SSH user/port/password. Razón: en el futuro el agente soportará otras marcas, no podemos asumir `admin`/`22` por defecto.
> 4. **No retrocompatible** con la plantilla vieja: el backend exige las 4 columnas. Los usuarios deben re-descargar la plantilla actualizada.

---

## Contexto — Estado actual

| Pieza | Ubicación | Estado |
|---|---|---|
| Generación de plantilla Excel (9 columnas SNMP) | [`users/views.py` L1263-1342](https://github.com/Julian-NESS/agentes) | Existe |
| Parser de plantilla Excel | [`users/views.py` L683-947](`relay_bulk_config_upload`) | Existe |
| Regen del bundle al cambiar checkboxes | [`users/views.py`](`relay_bulk_config_regenerate`) | Existe |
| Tabla de preview (`#wizard-relay-upload-preview`) | [`users/templates/users/download_utilities.html` L3715-3735](...) | Existe — 4 columnas: Monitorear / IP / SNMP / Port |
| Dropdown de intervalo de auditoría (Un dispositivo) | [`users/templates/users/download_utilities.html` L3654-3683](...) | Existe — id `wizard-relay-audit-interval` |
| Checkbox auditoría (Un dispositivo) | [`users/templates/users/download_utilities.html` L3627-3653](...) | Existe — id `wizard-relay-audit-enabled` |
| `relay_single_install` (POST que arma bundle firmado con audit) | [`users/views.py`](...) | Existe — el modo "Un dispositivo" ya manda `audit_enabled`/`audit_interval`/`ssh_*` al backend |
| `relay_bulk_config_upload` (POST que arma bundle firmado) | [`users/views.py` L683-947](...) | Existe — **NO** acepta columnas de auditoría todavía |
| Endpoint single vs bulk — `generateCommand` | [`users/templates/users/download_utilities.html` L6731-6760](...) | Existe — single llama a `/download_utilities/relay/single/`, bulk usa la URL ya guardada en `wizard-devices-config-url` |

**Gaps identificados**:
- La plantilla Excel solo trae 9 columnas (todas SNMP); no hay forma de que el operador indique "este FortiGate quiero auditarlos".
- El parser de Excel no conoce campos de auditoría: no genera `*_ssh_username`, `*_ssh_port`, `*_ssh_password` ni `audit_enabled` por fila.
- El `connection.config` generado (líneas 870-895) tampoco escribe esos campos.
- La tabla de preview UI solo tiene 4 columnas (Monitorear / IP / SNMP / Port) — no muestra el estado de auditoría.
- El wizard "Múltiples dispositivos" no expone el dropdown de intervalo de auditoría que sí existe en "Un dispositivo".

---

## Fases de implementación

> **Estados**: `🟡 Pendiente` · `🔵 En desarrollo` · `🟢 Completado`

### Fase 0 — Análisis y diseño

| # | Tarea | Estado |
|---|---|---|
| 0.1 | Identificar todas las piezas involucradas (template, parser, regen, UI preview, generateCommand) | 🟢 Completado |
| 0.2 | Aprobar decisiones de diseño con el usuario | 🟢 Completado |
| 0.3 | Documentar plan en `multiples_dispositivos.md` | 🟢 Completado |

---

### Fase 1 — Backend: extender la plantilla Excel (descargable)

**Objetivo**: El endpoint `download_relay_bulk_config_template` debe producir un `.xlsx` con **4 columnas adicionales** al final de la hoja "Dispositivos", y filas de ejemplo que ilustren el flujo (al menos una fila con E-V-CIS=x y todos los campos SSH llenos).

**Archivo a modificar**: [`nesshq/users/views.py`](nesshq/users/views.py) — vista `download_relay_bulk_config_template` (L1263-1342)

**Cambios concretos**:

1. Agregar 4 columnas al final de la lista `columns`:
   - `audit_enabled` → instrucciones: escribir `"x"` para activar, dejar vacío para omitir
   - `ssh_username` → instrucciones: usuario SSH (ej. `admin`)
   - `ssh_port` → instrucciones: puerto SSH (ej. `22`)
   - `ssh_password` → instrucciones: contraseña SSH (obligatoria si `audit_enabled=x`)

2. Renombrar la hoja "Dispositivos" a `Dispositivos` (sin cambio de nombre) y agregar una fila de ejemplo con todos los campos de auditoría llenos:
   ```python
   {
       'ip_address': '192.168.1.10',
       'port': 161,
       'snmp_version': '3',
       'community': '',
       'v3_user': 'snmp_user',
       'v3_auth_protocol': 'SHA',
       'v3_auth_password': 'snmp_pass_123',
       'v3_priv_protocol': 'AES128',
       'v3_priv_password': 'snmp_priv_123',
       'audit_enabled': 'x',
       'ssh_username': 'admin',
       'ssh_port': 22,
       'ssh_password': 'forti_password',
   },
   ```

3. En la hoja "Instrucciones" agregar 4 filas extra documentando los campos nuevos (mismo formato: Campo / Obligatorio / Descripción / Ejemplo).

**Criterios de aceptación**:
- [ ] La plantilla descargada tiene **13 columnas** en la hoja "Dispositivos" (9 SNMP + 4 auditoría).
- [ ] La hoja "Ejemplo" incluye al menos **una fila con auditoría activada** (todos los campos SSH llenos).
- [ ] La hoja "Instrucciones" tiene 13 filas, una por columna.
- [ ] La plantilla abre correctamente en Excel/LibreOffice sin warnings.

| # | Tarea | Estado |
|---|---|---|
| 1.1 | Agregar 4 columnas nuevas a la lista `columns` y a los DataFrames | � Completado |
| 1.2 | Agregar fila de ejemplo con E-V-CIS=x | 🟢 Completado |
| 1.3 | Extender hoja "Instrucciones" con 4 filas nuevas | 🟢 Completado |
| 1.4 | Test manual: descargar plantilla, abrirla, validar que las 4 columnas se ven | 🔵 En desarrollo (lo corre el usuario en Test 7) |

---

### Fase 2 — Backend: parser del Excel acepta auditoría por fila

**Objetivo**: La vista `relay_bulk_config_upload` debe:
- Leer las 4 columnas nuevas por fila.
- Validar que si `audit_enabled=x` los 3 campos SSH estén presentes.
- Generar el `connection.config` con las líneas de auditoría por dispositivo que apliquen.
- Devolver la info al frontend para que pinte la columna E-V-CIS en la tabla de preview.

**Archivo a modificar**: [`nesshq/users/views.py`](nesshq/users/views.py) — vista `relay_bulk_config_upload` (L683-947)

**Cambios concretos**:

1. En el helper `get_value(...)`, llamar igual que para SNMP — solo que los nuevos campos viven al final del Excel.

2. **Normalización de `audit_enabled`**: aceptar `x`, `X`, `true`, `TRUE`, `sí`, `si`, `1` (case-insensitive, trim). Cualquier valor vacío o desconocido → `false`. Documentar equivalencias en el código.

3. **Validación estricta** (decisión #3 aprobada):
   ```python
   if audit_enabled:
       if not ssh_username: errors.append(f'Fila {n}: E-V-CIS=x pero falta ssh_username')
       if not ssh_password: errors.append(f'Fila {n}: E-V-CIS=x pero falta ssh_password')
       if not ssh_port:     errors.append(f'Fila {n}: E-V-CIS=x pero falta ssh_port')
   ```
   Si hay errores, devolver 400 con la lista (`details: [...]`), mismo patrón que ya usa el parser para SNMPv3 incompleto.

4. En el dict `device_payload` agregar:
   ```python
   'audit_enabled': audit_enabled,
   'ssh_username': ssh_username if audit_enabled else '',
   'ssh_port': ssh_port if audit_enabled else 22,
   'ssh_password': ssh_password if audit_enabled else '',
   ```

5. En el `device_payload.update(...)` del bloque v3, NO escribir los `ssh_*` si el device no es Fortinet (consistente con la lógica de "Un dispositivo"). Por ahora la auditoría solo aplica a Fortinet, así que si el vendor detectado NO es Fortinet y `audit_enabled=true`, devolver warning pero NO generar las líneas SSH (y NO marcar `audit_enabled` en el config). Esto evita que se cree `audit_relay.sh` en dispositivos que no pueden auditarse.
   - **Decisión operacional**: mostrar warning `"Fila N: E-V-CIS=x pero el dispositivo {ip} no es Fortinet. La auditoría solo está disponible para Fortinet por ahora. Se omitirá para este dispositivo."`.

6. **Generar `connection.config`** con líneas por dispositivo (solo si `audit_enabled=True`):
   ```ini
   audit_enabled=true
   audit_interval=21600   # valor global que viene del frontend
   
   # Dispositivo N: ...
   generic_N_ssh_username=admin
   generic_N_ssh_port=22
   generic_N_ssh_password_env=NESS_SSH_PASSWORD_GENERIC_N
   generic_N_ssh_enabled=true
   ```
   Los nombres de env var siguen el patrón del instalador (Phase 2.6.4): `NESS_SSH_PASSWORD_<VENDOR>_<N>`. El backend **NO** guarda la contraseña en plano; solo el nombre del env var. La contraseña se persiste cifrada en `/etc/ness_relay/secrets.enc` (AES-256-GCM) ya en el instalador.

7. En el response JSON, agregar un campo `audit` por device en `full_devices_payload`:
   ```python
   payload['audit_enabled'] = device.get('audit_enabled', False)
   ```

8. En `preview_rows` agregar `audit_enabled: bool` para que el frontend pueda pintar la columna E-V-CIS.

**Criterios de aceptación**:
- [ ] Subir plantilla con 1 fila E-V-CIS=x y SSH completos → response 200, device aparece con `audit_enabled=true` en `full_devices_payload`.
- [ ] Subir plantilla con 1 fila E-V-CIS=x y SSH incompleto → response 400 con mensaje claro por fila.
- [ ] Subir plantilla sin las columnas nuevas → response 400 explicando que la plantilla no es la versión correcta (decisión #4: no retrocompatible).
- [ ] Subir plantilla con E-V-CIS=x en un dispositivo no-Fortinet → response 200 con warning por fila y `audit_enabled=false` para esa fila.

| # | Tarea | Estado |
|---|---|---|
| 2.1 | Extender `get_value` para las 4 columnas nuevas | � Completado |
| 2.2 | Normalizar `audit_enabled` (x/X/true/sí/1) | 🟢 Completado |
| 2.3 | Validación estricta: si audit=x, exigir los 3 campos SSH | 🟢 Completado |
| 2.4 | Detección de vendor y warning para no-Fortinet | 🟢 Completado |
| 2.5 | Generar líneas SSH_* y `audit_enabled` en `connection.config` | 🟢 Completado |
| 2.6 | Devolver `audit_enabled` por device en el response | 🟢 Completado |
| 2.7 | Tests unitarios (21/22 OK + 8/8 generación de config + 2/2 versión de plantilla) | 🟢 Completado — tests E2E en Test 1-6 los corre el usuario |

---

### Fase 3 — Backend: `relay_bulk_config_regenerate` respeta auditoría

**Objetivo**: Cuando el operador marca/desmarca checkboxes en la tabla de preview, la vista de regen ya existente debe seguir generando el bundle con las líneas de auditoría que correspondan a las filas seleccionadas.

**Archivo a modificar**: [`nesshq/users/views.py`](nesshq/users/views.py) — vista `relay_bulk_config_regenerate` (después de L960)

**Cambios concretos**:

1. El frontend ya envía en `selected_indices` la lista de filas a incluir. El backend debe **preservar** los campos `audit_enabled`, `ssh_username`, `ssh_port`, `ssh_password` de los devices seleccionados. Si la fila está en `full_devices_payload` y su `audit_enabled=true`, las líneas SSH_* se escriben en el config regenerado.
2. Validar que el intervalo de auditoría (`audit_interval`) que viene del frontend se persista en el config. Si el frontend lo manda en el payload, escribirlo como `audit_interval=N` al inicio del config.

**Criterios de aceptación**:
- [ ] Regenerar bundle con un device seleccionado que tiene `audit_enabled=true` → config resultante contiene las líneas SSH_*.
- [ ] Desmarcar ese device → la siguiente regen NO contiene sus líneas SSH_*.
- [ ] Si el frontend no manda `audit_interval`, mantener el último valor conocido (default 21600 si nunca se setó).

| # | Tarea | Estado |
|---|---|---|
| 3.1 | Preservar `audit_enabled` y `ssh_*` por device seleccionado en regen | � Completado |
| 3.2 | Aceptar y persistir `audit_interval` global en regen | 🟢 Completado |
| 3.3 | Test: regenerar con/sin dispositivos con auditoría | 🔵 En desarrollo (lo corre el usuario en Test 4) |

---

### Fase 4 — Frontend: agregar columna E-V-CIS a la tabla de preview

**Objetivo**: La tabla `#wizard-relay-upload-preview` debe mostrar una **quinta columna** "E-V-CIS" que indique visualmente si ese dispositivo va a ser auditado (icono ✅) o no (icono ❌ o "—").

**Archivo a modificar**: [`nesshq/users/templates/users/download_utilities.html`](nesshq/users/templates/users/download_utilities.html) — bloque `relay-bulk-preview` (L3715-3735 aprox.) + función `renderRelayBulkPreview(...)`.

**Cambios concretos**:

1. En el `<thead>` agregar `<th>E-V-CIS</th>` después de `<th>Port</th>`.

2. En el `<tbody>` (donde se pintan las filas), por cada device agregar:
   ```html
   <td>
     <span class="relay-bulk-audit-badge {{ 'active' if device.audit_enabled else 'inactive' }}">
       {{ '✅' if device.audit_enabled else '—' }}
     </span>
   </td>
   ```
   O mejor, replicar el estilo del checkbox "Monitorear" con un badge/icono de Bootstrap Icons (`bi-shield-check` o `bi-shield-x`).

3. En el paginador inferior (`relay-bulk-pagination`) agregar contador de dispositivos auditados:
   ```js
   const auditedCount = devices.filter(d => d.audit_enabled).length;
   // En el bloque de page-info, añadir: " · X con auditoría"
   ```

4. En `renderRelayBulkPreview(devices, vendors)`, ajustar el colspan del paginador si es necesario.

5. Estilos CSS: agregar `.relay-bulk-audit-badge` con dos variantes (`.active` verde, `.inactive` gris).

**Criterios de aceptación**:
- [ ] Tras subir una plantilla, la tabla muestra 5 columnas: Monitorear / IP / SNMP / Port / E-V-CIS.
- [ ] Los dispositivos con `audit_enabled=true` muestran un ícono verde/positivo en E-V-CIS.
- [ ] Los dispositivos sin auditoría muestran un ícono gris/negativo.
- [ ] El paginador muestra el conteo de dispositivos auditados.
- [ ] La columna se ve bien en mobile (responsive).

| # | Tarea | Estado |
|---|---|---|
| 4.1 | Agregar `<th>E-V-CIS</th>` en el `<thead>` | � Completado |
| 4.2 | Renderizar badge en cada `<tr>` del `<tbody>` | 🟢 Completado |
| 4.3 | Agregar contador "X con auditoría" en paginador | 🟢 Completado |
| 4.4 | CSS para `.relay-bulk-audit-badge.active` e `.inactive` | 🟢 Completado |
| 4.5 | Responsive: la columna no rompe en mobile (media query ≤ 700px) | 🟢 Completado |
| 4.6 | Test: subir plantilla mixta y verificar badges | 🔵 En desarrollo (lo corre el usuario en Test 7) |

---

### Fase 5 — Frontend: dropdown de intervalo de auditoría

**Objetivo**: El bloque "Múltiples dispositivos" debe tener un **dropdown de intervalo de auditoría** (6h/12h/24h), igual que el de "Un dispositivo". Debe ser visible solo cuando hay al menos un device con `audit_enabled=true` en el preview (no tiene sentido mostrarlo si nadie va a auditarse).

**Archivo a modificar**: [`nesshq/users/templates/users/download_utilities.html`](nesshq/users/templates/users/download_utilities.html) — bloque del step 3 y función `wizardTryProceedToInstallStep()`.

**Cambios concretos**:

1. En el bloque `#wizard-relay-bulk-config`, después del `<div class="relay-bulk-status">` y antes del `<div class="relay-bulk-preview">`, agregar:
   ```html
   <div class="config-field" id="wizard-relay-bulk-audit-block" style="display:none; margin-top: 1rem;">
     <label for="wizard-relay-bulk-audit-interval">
       Intervalo de auditoría (vulnerabilidades + CIS)
     </label>
     <select id="wizard-relay-bulk-audit-interval">
       <option value="21600">Cada 6 horas (recomendado)</option>
       <option value="43200">Cada 12 horas</option>
       <option value="86400">Cada 24 horas</option>
     </select>
     <p class="config-info-box" style="margin-top:0.5rem;">
       <i class="bi bi-info-circle"></i>
       <span>El escaneo se ejecuta solo en dispositivos con E-V-CIS activado en la plantilla. Requiere acceso SSH.</span>
     </p>
   </div>
   ```

2. Después de `renderRelayBulkPreview(...)`, mostrar/ocultar el bloque según si hay devices con auditoría:
   ```js
   const auditBlock = document.getElementById('wizard-relay-bulk-audit-block');
   const auditedCount = devices.filter(d => d.audit_enabled).length;
   if (auditBlock) {
       auditBlock.style.display = auditedCount > 0 ? 'block' : 'none';
   }
   ```

3. En la función `_generateCommandSync({ ... isRelayBulk, ... })` (L6899), leer el valor del nuevo dropdown y pasarlo al backend de regen si hay cambios. Si el usuario cambió el intervalo DESPUÉS de subir la plantilla, hay que **regenerar el bundle** para que el config incluya el nuevo `audit_interval`:
   ```js
   // Cuando hay audit activo, regenerar el bundle antes de generar el comando
   if (isRelayBulk && auditedCount > 0) {
       await regenerateBulkBundleUrl({ audit_interval: parseInt(...) });
   }
   ```
   Reusar `regenerateBulkBundleUrl()` que ya existe.

4. Validación: en `wizardValidateConfigStep(true)` (o función equivalente), si el modo es bulk y hay al menos un device con auditoría, **exigir** que el usuario haya seleccionado un intervalo (no puede ser null).

**Criterios de aceptación**:
- [ ] Al subir una plantilla SIN dispositivos auditados, el bloque del dropdown NO aparece.
- [ ] Al subir una plantilla CON al menos un dispositivo auditado, el bloque aparece con el default "Cada 6 horas".
- [ ] Cambiar el intervalo y generar el comando → el bundle regenerado incluye el nuevo `audit_interval`.
- [ ] El comando generado en el step 4 incluye `NESS_DEVICES_FILE_URL` con el bundle regenerado.

| # | Tarea | Estado |
|---|---|---|
| 5.1 | HTML: bloque `#wizard-relay-bulk-audit-block` con select | � Completado |
| 5.2 | JS: mostrar/ocultar bloque según `auditedCount > 0` en `renderRelayBulkTable` | 🟢 Completado |
| 5.3 | JS: al cambiar el select, regenerar bundle con nuevo `audit_interval` (`onRelayBulkAuditIntervalChange`) | 🟢 Completado |
| 5.4 | JS: en `generateCommand` para bulk, si hay auditoría, forzar regen antes de pintar el comando | 🟢 Completado |
| 5.5 | Validación: bloque solo aparece si hay al menos 1 device con E-V-CIS=x | 🟢 Completado |
| 5.6 | Test: cambiar intervalo → comando refleja el cambio | 🔵 En desarrollo (lo corre el usuario en Test 4) |

---

### Fase 6 — Frontend: integrar `audit_interval` en el comando generado

**Objetivo**: El comando `curl | sudo ... bash` que se muestra en el step 4 debe seguir funcionando igual (las env vars ya pasan el bundle firmado), pero el bundle debe contener el `audit_interval` elegido y los `ssh_*` por device.

**Archivo a modificar**: [`nesshq/users/templates/users/download_utilities.html`](nesshq/users/templates/users/download_utilities.html) — función `_generateCommandSync` y `regenerateBulkBundleUrl`.

**Cambios concretos**:

1. La función `regenerateBulkBundleUrl()` (existente) debe aceptar el nuevo payload `audit_interval` y mandarlo al backend de regen (`relay_bulk_config_regenerate`).
2. El backend de regen (Fase 3) acepta `audit_interval` y lo persiste en el config.
3. El comando generado no cambia de forma visible — sigue siendo `curl | sudo NESS_TOKEN=... NESS_TIME=... NESS_SERVER_ID=... NESS_DEVICES_FILE_URL=... bash`. El instalador se encarga del resto.

**Criterios de aceptación**:
- [ ] El comando generado es idéntico en estructura al actual, solo cambia el contenido del bundle (URL firmada).
- [ ] El instalador detecta `audit_enabled=true` en el bundle y crea `audit_relay.sh` (igual que en "Un dispositivo" — la lógica del instalador ya existe y funciona).

| # | Tarea | Estado |
|---|---|---|
| 6.1 | `relayBulkRegenerateBundle` acepta `audit_interval` y lo envía al backend | 🟢 Completado |
| 6.2 | El comando se muestra igual, sin cambios visibles | 🟢 Completado |

---

### Fase 7 — Pruebas E2E (las corre el usuario)

**Objetivo**: Validar el flujo completo de extremo a extremo, exactamente como lo hará el operador en producción.

**Criterios de aceptación globales**:

- [ ] **Test 1 (válido, mixto)**: plantilla con 3 dispositivos (1 FortiGate con auditoría + SSH completos, 1 dispositivo genérico sin auditoría, 1 FortiGate con E-V-CIS=x pero SSH incompleto) → el backend rechaza la fila 3 con mensaje claro.
- [ ] **Test 2 (válido, todo FortiGate con auditoría)**: plantilla con 2 FortiGate, ambos con E-V-CIS=x y SSH completos → respuesta 200, comando generado, instalador crea `audit_relay.sh`, primera ejecución corre auditoría en los 2 dispositivos.
  - **Test 2-bis (sub-test del bug de Fase 9)**: tras instalar con E-V-CIS=x, verificar en `connection.config` que existen las líneas `*_ssh_password_plain=...` por device. Sin estas líneas, el instalador no puede cifrar SSH y las fases 9-10 se saltan silenciosamente. Ver también que `/etc/ness_relay/secrets.enc` se crea (no solo `secrets.env`).
- [ ] **Test 3 (sin auditoría)**: plantilla con 3 dispositivos, ninguno con E-V-CIS → respuesta 200, instalador NO crea `audit_relay.sh` (igual que antes).
- [ ] **Test 4 (intervalo)**: cambiar el dropdown a "Cada 12 horas" y regenerar → el bundle contiene `audit_interval=43200` y el cron del instalador queda `0 */12 * * *`.
- [ ] **Test 5 (No-Fortinet con E-V-CIS=x)**: plantilla con 1 dispositivo genérico y E-V-CIS=x → backend acepta, devuelve warning, ese dispositivo NO se audita.
- [ ] **Test 6 (plantilla vieja)**: subir plantilla con solo 9 columnas → backend rechaza con error 400 explicando que es la versión anterior.
- [ ] **Test 7 (UI responsive)**: en viewport mobile, la columna E-V-CIS no rompe el layout.

| # | Tarea | Estado |
|---|---|---|
| 7.1 | Test 1 — plantilla mixta con SSH incompleto | � En desarrollo (lo corre el usuario) |
| 7.2 | Test 2 — todo FortiGate con auditoría | � Completado (2026-07-13: 3 FortiGate con E-V-CIS=x corrieron fases 9-10, JSON + envío al servidor OK) |
| 7.2b | Test 2-bis — verificar `_ssh_password_plain=...` en el config y `secrets.enc` creado | 🟢 Completado (2026-07-13: `secrets.enc` creado con 354 bytes; `_ssh_password_plain=...` procesado por `apply_audit_to_bundle`; config final con `*_ssh_password_env=NESS_SSH_PASSWORD_FORTINET_N`) |
| 7.3 | Test 3 — sin auditoría | 🔵 En desarrollo (lo corre el usuario) |
| 7.4 | Test 4 — cambiar intervalo | 🔵 En desarrollo (lo corre el usuario) |
| 7.5 | Test 5 — no-Fortinet con E-V-CIS | 🔵 No ejecutado (el usuario probó con MikroTik SIN E-V-CIS; el flujo cubre el caso "vendor con E-V-CIS en no-Fortinet" que el código ya maneja, pero falta la prueba explícita) |
| 7.6 | Test 6 — plantilla vieja | 🔵 En desarrollo (lo corre el usuario) |
| 7.7 | Test 7 — UI responsive | 🔵 En desarrollo (lo corre el usuario) |

---

### Fase 8 — Despliegue y documentación

**Objetivo**: Subir los cambios al bucket GCP y actualizar la documentación para el usuario final.

| # | Tarea | Estado |
|---|---|---|
| 8.1 | Subir nueva versión del instalador a GCP (`install_relay.sh` + `latest.json`) | 🟡 Pendiente |
| 8.2 | Reiniciar `gunicorn` del backend nesshq para que tome los cambios | 🟡 Pendiente |
| 8.3 | Verificar que el wizard en producción muestra los cambios | 🟡 Pendiente |
| 8.4 | Documentar en la guía del usuario cómo usar E-V-CIS en la plantilla | 🟡 Pendiente |

---

### Fase 9 — Bug: bulk escribía `_ssh_password_env=...` en vez de `_ssh_password_plain=...`

**Reportado por el usuario el 2026-07-12 tras la prueba E2E de Test 2 (3 FortiGate con E-V-CIS=x).**

**Síntoma**:
- Instalación del agente completada "exitosamente".
- `audit_relay.sh` creado y cron `0 */6 * * *` configurado.
- Primera ejecución reportó `Modo auditoría ACTIVADO (NESS_AUDIT_ENABLED=true)`.
- **PERO**: el agente recorrió solo fases 1-8, no ejecutó las fases 9-10 (vulnerabilidades + CIS). Reportó `Ciclo completado — N exitoso(s)` sin auditoría real.

**Config final observado**:
- `audit_enabled=true` ✓ (top-level)
- `audit_interval=21600` ✓ (top-level)
- Comentarios `# SSH audit (Phase 2.4)` por device ✓
- **`generic_N_ssh_username`/`_port`/`_enabled` AUSENTES** ✗
- `generic_N_ssh_password_env=NESS_SSH_PASSWORD_GENERIC_N` AUSENTE ✗
- `generic_N_ssh_password_plain=...` AUSENTE ✗
- `/etc/ness_relay/secrets.enc` NO creado ✗
- `/etc/ness_relay/` solo contiene `.salt` y `.seed`

**Logs del instalador (las pistas)**:
```
─── Vendor compatible con auditoría detectada: fortinet ───
    Dispositivo: 192.168.10.17
[2026-07-12 17:12:50] Silent mode + sin bundle_pw para fortinet_1: saltando prompt SSH (configure manualmente después)
[2026-07-12 17:12:51] apply_audit_to_bundle: no hay *_ssh_password_plain en el config, saltando
```

**Root cause** (3 problemas en cascada, todos introducidos en Fases 2+3 del plan):

1. **Backend escribía `_ssh_password_env=...` directamente** en vez de `_ssh_password_plain=...`. El instalador (Phase 2.6.3) espera el `_plain` para poder cifrar la password con `ness-relay set` y persistirla en `secrets.enc`. Sin `_plain`, no había manera de obtener la password en runtime.

2. **Backend no enviaba `ssh_password` al frontend** en `full_devices_payload` (decisión inicial de "seguridad"). Esto significa que cuando el usuario marca/desmarca checkboxes y se llama `relay_bulk_config_regenerate`, el frontend no tenía cómo reenviar la password al backend. El regen producía un config sin `_ssh_password_plain` (campo vacío).

3. **El instalador en modo silent no puede pedir la password** (sería un `read` que se cuelga en `curl | bash`). El instalador busca `_plain` en el bundle firmado; si no está, salta el prompt con `WARN: Silent mode + sin bundle_pw ...`. La auditoría queda sin credenciales → fases 9-10 se saltan sin error visible.

**Por qué el modo "Un dispositivo" SÍ funcionaba**: `relay_single_install` siempre escribió `_ssh_password_plain=...` (línea 1409 de `views.py`). Esta era la convención esperada por el instalador. El plan inicial copió la idea del env var directamente sin verificar el contrato real del instalador.

**Fix (Phase 2.6.4-fix aplicado el 2026-07-12)**:

1. En `relay_bulk_config_upload` (línea ~1014) y `relay_bulk_config_regenerate` (línea ~1215): cambiar `_ssh_password_env=NESS_SSH_PASSWORD_GENERIC_N` → `_ssh_password_plain=mi_password`. El instalador se encarga del cifrado con `apply_audit_to_bundle`.

2. En `relay_bulk_config_upload` (`full_devices_payload`): el backend ahora SÍ envía `ssh_password` al frontend (campo en `relayBulkPreviewState.rows[i].ssh_password`). El frontend la retiene en memoria y la reenvía al backend SOLO en el POST de regen. No se renderiza en el HTML.

3. Seguridad: la password en el bundle está protegida por `chmod 600` + nombre UUID aleatorio + URL firmada con expiración de 15 min. Mismo nivel de protección que `relay_single_install`.

**Criterios de aceptación del fix**:
- [x] `relay_bulk_config_upload` escribe `_ssh_password_plain=...` en el config cuando `audit_enabled=true`.
- [x] `relay_bulk_config_regenerate` escribe `_ssh_password_plain=...` usando la password que viene en el body del POST.
- [x] `full_devices_payload` incluye `ssh_password` por device con `audit_enabled=true`.
- [ ] (Test E2E) Instalar con E-V-CIS=x y verificar que el instalador crea `/etc/ness_relay/secrets.enc` (no solo `.salt`/`.seed`).
- [ ] (Test E2E) Primera ejecución corre fases 9-10 y produce `relay_sentinel_vulnerabilities_data.json` y `relay_sentinel_cis_data.json`.

| # | Tarea | Estado |
|---|---|---|
| 9.1 | Backend: escribir `_ssh_password_plain` (upload) | 🟢 Completado |
| 9.2 | Backend: escribir `_ssh_password_plain` (regen) | 🟢 Completado |
| 9.3 | Backend: enviar `ssh_password` en `full_devices_payload` | 🟢 Completado |
| 9.4 | Test E2E: verificar que `secrets.enc` se crea y fases 9-10 corren | � Completado (2026-07-13: `secrets.enc` creado 354 bytes con 3 credenciales Fortinet; fases 9-10 corrieron para los 3 FortiGate y se saltaron limpiamente para el MikroTik) |

---

## Resumen de entregables

| Archivo | Cambios |
|---|---|
| [`nesshq/users/views.py`](nesshq/users/views.py) | `download_relay_bulk_config_template`: +4 columnas + hoja Instrucciones. `relay_bulk_config_upload`: parser + validación + config con `_ssh_password_plain` (Phase 2.6.4-fix). `relay_bulk_config_regenerate`: preservar audit + password por device seleccionado. |
| [`nesshq/users/templates/users/download_utilities.html`](nesshq/users/templates/users/download_utilities.html) | `<th>E-V-CIS</th>`, badges, dropdown de intervalo, integración con `regenerateBulkBundleUrl`, `generateCommand` async. |
| Instalador (`agentes/ness_relay/...`) | **Sin cambios** — el instalador ya soporta `audit_enabled` y `_ssh_password_plain` (mismo esquema que `relay_single_install`). |

## Riesgos identificados

| Riesgo | Mitigación |
|---|---|
| Plantillas viejas en circulación | Decisión #4: rechazar plantilla vieja con error 400. Comunicar a usuarios que re-descarguen. |
| Dispositivos no-Fortinet con E-V-CIS=x | Decisión #3 + warning explícito. Cuando se sume otra marca al instalador, la lógica del backend solo necesita cambiar el `vendor == 'fortinet'` por el conjunto de vendors soportados. |
| Contraseñas SSH en el bundle | El bundle SÍ trae la password en plano (`*_ssh_password_plain=...`) — esto es necesario para que el instalador (en modo silent) pueda cifrarla con `ness-relay set` y persistirla en `/etc/ness_relay/secrets.enc`. El bundle está protegido por `chmod 600`, UUID aleatorio y URL firmada con expiración de 15 min. Mismo esquema que `relay_single_install`. |
| Dropdown de intervalo no visible si no hay audit | UX intencional: no se muestra hasta que hay al menos un device con E-V-CIS=x. |

---

## Orden de ejecución recomendado

1. **Fase 1** (plantilla) → **Fase 2** (parser) → **Fase 4** (UI preview) → **Fase 5** (dropdown) → **Fase 3** (regen) → **Fase 6** (integración) → **Fase 7** (pruebas) → **Fase 8** (deploy) → **Fase 9** (bug de `_ssh_password_plain` ya corregido).
2. Las Fases 1-2 se pueden hacer juntas (un solo commit) porque ambas son backend.
3. Las Fases 4-5 son UI y se pueden hacer juntas (un solo commit).
4. La Fase 3 puede ir al final o entrelazada, ya que el flujo principal funciona sin ella (solo el regen al desmarcar checkboxes).
5. **Fase 9 (fix de auditoría) es bloqueante para Test 2-bis**: sin este fix, ningún Test 2 puede pasar completamente. Las fases 1-8 sin Fase 9 dan la falsa sensación de que la auditoría funciona (cron creado, banner mostrado) pero las fases 9-10 nunca corren en bulk.

---

**Cambios totales del fix de Fase 9**: 3 hunks en [`nesshq/users/views.py`](nesshq/users/views.py) — `relay_bulk_config_upload` línea ~1014 (escribir `_plain`), `full_devices_payload` (enviar `ssh_password` al frontend), `relay_bulk_config_regenerate` línea ~1215 (escribir `_plain` desde el regen). No se tocó el instalador.
