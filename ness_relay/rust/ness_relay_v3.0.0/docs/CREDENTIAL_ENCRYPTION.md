# NESS Relay v2.5.0 — Cifrado de credenciales (guía del operador)

> Estado: **Activo desde v2.5.0**. Las versiones v2.4.0 son compatibles hacia
> atrás (los campos en plano siguen funcionando), pero el operador debe migrar
> para eliminar contraseñas en claro en `/opt/ness_relay/configs/connection.config`.

---

## TL;DR

| Pregunta | Respuesta |
|----------|-----------|
| ¿Dónde se guardan ahora las pass? | **Cifradas con AES-256-GCM** en `/opt/ness_relay/configs/connection.config` (campos con prefijo `$enc$2$...`) y en `/etc/ness_relay/secrets.enc` (binario). |
| ¿Qué pasa con mis instalaciones v2.4.0? | Siguen funcionando SIN cambios. El binario detecta plano y lo acepta (con un WARN). Para cifrar, ejecutar `ness-relay-cred migrate-plaintext`. |
| ¿Cómo se protege la clave maestra? | Derivada del host con HKDF-SHA256 sobre `machine-id` + sal local. **Robar el backup de otra máquina NO sirve** — la clave está atada al host. |
| ¿Y si reinstalo el sistema? | Hay que re-migrar (la sal vive en `/etc/ness_relay/.salt`; si se pierde, no se puede descifrar). Hacer backup de `/etc/ness_relay/` aparte. |
| ¿Es compatible con TPM? | **No por ahora** (elegimos portabilidad a VMs). Si el host tiene TPM2, se puede añadir soporte en una versión futura. |

---

## Cambios respecto a v2.4.0

### Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `connection.config` | Campos `v3_auth_password`, `v3_priv_password`, `community` ahora se guardan como `$enc$2$<base64>` en lugar de texto plano. |
| `/etc/ness_relay/secrets.env` | **Reemplazado** por `/etc/ness_relay/secrets.enc` (binario). El nuevo archivo ya no tiene passwords en claro. |
| `/etc/ness_relay/.salt` | **Nuevo**. Sal aleatoria de 16 bytes, `chmod 600`, `root:root`. Necesaria para derivar la clave maestra. |
| Binario `ness-relay` | Al cargar el config, descifra transparentemente los campos `$enc$2$...`. El código que consume las pass (SNMPv3, SSH) no nota la diferencia. |
| Binario `ness-relay-cred` | **Nuevo**. Subcomandos para gestión (`migrate-plaintext`, `set`, `test-cred`, `status`, etc.). |

### Lo que NO cambió

- **Endpoint del servidor**: `/api/relay/data/` sigue igual. Los JSON de SNMP + vulnerabilidades + CIS no se ven afectados.
- **Estructura del config**: la sintaxis INI sigue idéntica, solo cambian los valores de los campos sensibles.
- **Cron jobs**: la frecuencia de 5 min (recolección) y 6 h (auditoría) sigue igual.

---

## Procedimientos

### 1. Instalación nueva (operador nuevo)

El instalador (`install_relay.sh`) automáticamente:

1. Crea `/etc/ness_relay/` con permisos 700.
2. Crea `/etc/ness_relay/.salt` (16 bytes random) con permisos 600.
3. Cuando el operador ingresa una pass SNMPv3, la cifra con `ness-relay-cred encrypt-field` y la escribe como `$enc$2$...` en el config.
4. Cuando el operador ingresa una pass SSH, la cifra con `ness-relay-cred set <env_var>` y la guarda en `/etc/ness_relay/secrets.enc`.

**El operador no necesita hacer nada adicional.** Las pass se cifran en el momento.

### 2. Migración de v2.4.0 → v2.5.0 (instalación existente)

```bash
sudo /opt/ness_relay/executables/ness-relay-cred migrate-plaintext
```

El comando:

1. Lee `/opt/ness_relay/configs/connection.config`.
2. Por cada campo sensible (`v3_auth_password`, `v3_priv_password`, `community`):
   - Si ya está cifrado: lo salta.
   - Si está en plano: pide la pass por consola (con confirmación), la cifra y reescribe el campo.
3. Crea un backup `connection.config.bak.<timestamp>`.
4. **También** convierte el antiguo `/etc/ness_relay/secrets.env` (si existe) en `secrets.enc` cifrado.

Para uso no-interactivo (automatización, CI):

```bash
printf "MyPass123*\nMyPass123*\n" | \
    sudo /opt/ness_relay/executables/ness-relay-cred migrate-plaintext -y
```

### 3. Verificar que una pass descifra correctamente

```bash
sudo /opt/ness_relay/executables/ness-relay-cred test-cred fortinet_1 v3_auth_password
# → [OK]   fortinet_1_v3_auth_password: descifra correctamente (len = 16)
#         (el valor no se imprime por seguridad)
```

Si falla:

```bash
sudo /opt/ness_relay/executables/ness-relay-cred status
# → Diagnóstico del vault: .salt existe, .seed existe, machine-id OK
```

Si restauraste el config de otro host, la clave maestra será distinta y el
descifrado fallará. Soluciones:

- Re-migrar desde el host original con `migrate-plaintext`.
- O restaurar también `/etc/ness_relay/.salt` del host original.

### 4. Añadir/rotar una pass SSH (sin re-instalar)

```bash
sudo /opt/ness_relay/executables/ness-relay-cred set NESS_SSH_PASSWORD_fortinet_1
# → pide el valor por consola con echo desactivado
# → pide confirmación
# → cifra con AES-256-GCM y guarda en /etc/ness_relay/secrets.enc
```

Para listar las pass guardadas (sin ver el valor):

```bash
sudo /opt/ness_relay/executables/ness-relay-cred list
# → Env vars en /etc/ness_relay/secrets.enc (3):
#       NESS_SSH_PASSWORD_fortinet_1  (len = 16)
#       NESS_SSH_PASSWORD_mikrotik_1  (len = 12)
#       NESS_API_TOKEN                (len = 32)
```

### 5. Ver diagnóstico del vault

```bash
sudo /opt/ness_relay/executables/ness-relay-cred status
# → NESS Relay — vault status
#       Root:                 /etc/ness_relay
#       Salt existe:           true
#       Seed existe (fb):      true
#       secrets.enc existe:   true
#       /etc/machine-id:       OK (ab3f1c2d4e5f6789)
#       Master key derivada:   OK (32 bytes zeroized)
```

---

## Preguntas frecuentes

### ¿Por qué AES-GCM y no AES-CBC o ChaCha20?

- **AES-256-GCM** ofrece **autenticación** (AEAD): si alguien modifica 1 byte del
  ciphertext, el descifrado **falla** con error claro, no devuelve basura.
- AES-CBC solo cifra, no autentica — vulnerable a padding-oracle y bit-flipping.
- ChaCha20-Poly1305 es equivalente, pero AES-GCM tiene aceleración hardware
  (AES-NI) en CPUs modernos (todos los servers x86_64 desde 2010+).

### ¿Por qué derivar la clave del `machine-id` y no pedirme un master-password?

Pedir un master-password en la instalación es un anti-patrón conocido:

- El usuario lo anota en un post-it, lo mete en el password manager, lo
  olvida a los 6 meses.
- Si cambia de operador, hay que re-instalar todo.
- Si el host muere, hay que recordar el master para restaurar backups.

Con la derivación del host, el comportamiento es **auto-contenido**:

- La clave se regenera en cada boot leyendo `machine-id` y la sal local.
- Para restaurar backups, solo hay que copiar `machine-id` + `.salt` +
  `secrets.enc` (los tres juntos son el "bundle" del host).
- Si el host se reemplaza, la sal se regenera → los backups viejos NO sirven
  (eso es **deseable**: un atacante con backups de 2024 no puede descifrar
  el host actual de 2026).

### ¿Y si mi `machine-id` cambia (ej. clonar la VM)?

El nuevo host derivará una clave **distinta** y no podrá descifrar los
hallazgos antiguos. Solución: tras clonar, copiar `/etc/ness_relay/` del
original al clon ANTES del primer arranque, o re-migrar.

### ¿Por qué AAD contextual (`<device_id>|<field>`)?

Para impedir que un atacante con acceso al config copie un token entre
dispositivos o campos. Si copia `fortinet_1|v3_auth_password` a la posición
de `mikrotik_1|v3_priv_password`, el descifrado **falla** porque el AAD
no coincide (Fortinet ≠ MikroTik).

### ¿Por qué el binario descifra on-demand y no carga todo al inicio?

Para minimizar el tiempo de residencia de las pass en memoria:

- Cifrado en disco → instantáneo (AES-GCM sin overhead).
- Descifrado → solo cuando el binario va a usar la pass (ej. para hacer
  handshake SNMPv3 o SSH).
- `Zeroizing<String>` se limpia de memoria al salir del scope.

---

## Procedimiento de recuperación ante desastre

Si `/etc/ness_relay/.salt` se perdió pero tienes el `connection.config` y
`secrets.enc`:

1. **NO** puedes descifrar. La sal es la fuente de no-arbitrariedad de la
   clave maestra (sin ella, el master key sería determinístico al 100%
   del machine-id, lo cual es OK si el machine-id no se ha cambiado).
2. Si el `machine-id` no se ha cambiado, puedes borrar `.salt` y dejar que
   `ensure_vault()` lo regenere. PERO la clave derivada será DIFERENTE a la
   anterior y no podrás descifrar.
3. Solución real: **backup periódico** de `/etc/ness_relay/` (excluyendo
   `target/`, `logs/`). Con `./.salt` + `./secrets.enc` +
   `connection.config` (cifrado), todo se restaura.

Recomendación para producción: cron diario que copia
`/etc/ness_relay/{.salt,secrets.enc}` a un directorio cifrado separado
(LUKS, EncFS, AWS KMS-encrypted S3, etc.).

---

## Roadmap (futuro, no implementado)

- **Opción B con TPM2** (v2.6.0?): si el host tiene TPM2, sellar la clave
  maestra al TPM para impedir extracción física. Requiere `tss-esapi` crate
  o `tpm2-tss`.
- **HashiCorp Vault opcional** (v3.0?): para topologías multi-tenant
  grandes donde múltiples relays comparten el mismo backend de secrets.
- **Rotación automática de claves** (v2.7.0?): comando
  `ness-relay-cred rotate` que genera una nueva sal, re-cifra todos los
  secretos, y borra el material antiguo.
- **Auto-detección de campos nuevos**: cuando el config gana un campo
  sensible nuevo (ej. `wifi_password` en un futuro perfil), el binario
  debe detectarlo y aplicar el mismo descifrado transparente.
