# Proceso de actualizacion - NESS Relay Rust

## Objetivo
Este documento describe todo el sistema de actualizacion del agente Relay en Rust: como funciona automaticamente, como ejecutarlo manualmente y que valida en cada etapa.

## Resumen rapido
El agente soporta 2 modos de actualizacion:

1. Actualizacion automatica en modo continuo.
2. Actualizacion manual bajo demanda por CLI.

En ambos casos, la logica base es:

1. Consultar latest.json remoto.
2. Comparar version local vs version remota.
3. Validar compatibilidad de versiones (min_supported).
4. Descargar paquete ZIP.
5. Verificar SHA-256 (obligatorio).
6. Reemplazar binario con backup previo.
7. Limpiar backups antiguos.
8. Marcar reinicio graceful.

## Componentes involucrados

- Flujo principal: [main.rs](../rust/ness_relay_v2.0.0/src/main.rs)
- Logica de update: [updater.rs](../rust/ness_relay_v2.0.0/src/updater.rs)
- Estado de chequeos: [update_tracker.rs](../rust/ness_relay_v2.0.0/src/update_tracker.rs)
- Reportes al backend: [server_reporter.rs](../rust/ness_relay_v2.0.0/src/server_reporter.rs)
- Reinicio graceful: [restart_handler.rs](../rust/ness_relay_v2.0.0/src/restart_handler.rs)
- Preservacion de config: [config_backup.rs](../rust/ness_relay_v2.0.0/src/config_backup.rs)
- Configuracion de URLs/intervalos: [config.rs](../rust/ness_relay_v2.0.0/src/config.rs)
- Metadata de version remota: [latest.json](../jsons/latest.json)

## Fuente de verdad de versiones

URL oficial de metadata:

- https://storage.googleapis.com/repo.nesshq.com/utilities/relay/latest.json

Campos importantes esperados en latest.json:

1. version
2. min_supported
3. pack.url
4. pack.sha256
5. changelog

## Flujo de actualizacion automatica

El flujo automatico corre cuando el agente inicia en modo continuo.

Comando tipico:

```bash
./ness_relay --continuous 5
```

Secuencia:

1. El agente ejecuta su ciclo normal de recoleccion.
2. Revisa con update_tracker si ya toca chequeo (intervalo por defecto: 24 horas).
3. Si toca chequeo:
   - Consulta latest.json remoto.
   - Compara version local contra version remota.
   - Verifica compatibilidad con min_supported.
4. Si hay update disponible:
   - Reporta AVAILABLE al servidor.
   - Guarda configuracion critica en backup temporal.
   - Reporta STARTED.
   - Descarga ZIP del nuevo binario.
   - Verifica SHA-256 del ZIP (si no coincide, falla).
   - Extrae binario y reemplaza ejecutable actual.
   - Ajusta permisos de ejecucion.
   - Limpia backups antiguos (mantiene MAX_BACKUPS).
   - Restaura configuracion.
   - Reporta COMPLETED.
   - Marca update pending.
   - Reporta PENDING.
   - Crea flag de restart graceful.
   - Finaliza proceso actual para que supervisor lo levante de nuevo.
5. Si no hay update:
   - Continuan ciclos normales.

## Flujo de actualizacion manual

Si. Puedes ejecutar el update manualmente tu mismo.

Comando:

```bash
./ness_relay --update
```

Que hace:

1. Consulta latest.json remoto.
2. Si existe version superior y compatible:
   - Descarga paquete.
   - Verifica hash SHA-256.
   - Reemplaza binario.
   - Limpia backups.
3. Muestra en logs que debes reiniciar el agente para aplicar la nueva version.

Notas:

- Este modo no depende del scheduler de 24 horas.
- Es ideal para despliegues controlados o mantenimiento.

## Se puede desactivar temporalmente el auto-check en continuo

Existe flag oculto para operacion controlada:

```bash
./ness_relay --continuous 5 --skip-update-check
```

Uso recomendado:

- Ventanas de mantenimiento.
- Diagnostico sin cambios de version.
- Pruebas de recoleccion sin tocar update.

## Archivos temporales y artefactos del proceso

- Estado del update tracker:
  - /tmp/ness_relay_update_state.json
- Backup de configuracion preservada:
  - /tmp/ness_relay_config_backup.json
- ZIP temporal descargado:
  - /tmp/ness_relay_update.zip
- Flag de reinicio graceful:
  - /tmp/ness_relay.restart_pending
- Backups del binario:
  - misma carpeta del ejecutable, formato ness_relay.YYYYMMDD_HHMMSS.bak

## Validaciones de seguridad y consistencia

1. Hash SHA-256 obligatorio.
2. Validacion semver de upgrade (remote > local).
3. Validacion de compatibilidad (local >= min_supported).
4. Backup del binario antes de reemplazo.
5. Restauracion de configuracion despues del update.

## Reportes al servidor

Estados reportados durante el proceso:

1. AVAILABLE
2. STARTED
3. COMPLETED
4. PENDING
5. FAILED

URL por defecto configurable via entorno en AppConfig:

- NESS_UPDATE_REPORT_URL

## Variables de entorno relevantes

1. NESS_VERSION_CHECK_URL
2. NESS_HOSTING_URL
3. NESS_UPDATE_REPORT_URL
4. NESS_API_TOKEN
5. NESS_SERVER_ID
6. NESS_SERVER_URL
7. NESS_DEVICES_FILE
8. NESS_OUTPUT_DIR
9. NESS_LOG_DIR

## Procedimiento operativo recomendado para publicar una nueva version

1. Compilar nueva version del binario.
2. Empaquetar ZIP con el ejecutable correcto.
3. Calcular SHA-256 real del ZIP.
4. Subir ZIP al bucket GCP.
5. Actualizar latest.json con:
   - version nueva
   - pack.url correcto
   - pack.sha256 real
   - min_supported correcto
   - changelog
6. Validar manualmente:
   - ./ness_relay --update
7. Validar automaticamente:
   - ./ness_relay --continuous 5

## Importante sobre latest.json actual

El archivo [latest.json](../jsons/latest.json) tiene hashes de ejemplo (no reales). Antes de usar en produccion, reemplazar pack.sha256 y files.ness_relay.sha256 por valores SHA-256 reales del artefacto publicado.

## Troubleshooting rapido

1. Error de hash:
   - Confirmar que pack.sha256 coincide exactamente con el ZIP publicado.
2. No detecta nueva version:
   - Confirmar que version remota sea mayor a RELAY_VERSION local.
3. Update incompatible:
   - Revisar min_supported en latest.json.
4. No reporta al servidor:
   - Verificar NESS_API_TOKEN y NESS_UPDATE_REPORT_URL.
5. No reinicia tras update:
   - Verificar supervisor (systemd/docker) y flag /tmp/ness_relay.restart_pending.

## Comandos utiles

Chequeo de compilacion:

```bash
cd /home/nessuser/agentes/ness_relay/rust/ness_relay_v2.0.0 && cargo check
```

Build release:

```bash
cd /home/nessuser/agentes/ness_relay/rust/ness_relay_v2.0.0 && ./build_relay.sh --arch x86_64 --release
```

Update manual:

```bash
./ness_relay --update
```

Modo continuo con update automatico:

```bash
./ness_relay --continuous 5
```

## Guía operativa para pruebas en GCP

Para el entorno de desarrollo, la estructura recomendada del bucket debe separar dos cosas:

1. Un puntero fijo de versionado: `utilities/relay/latest.json`
2. Los artefactos por versión: `utilities/relay/2.0.0/`, `utilities/relay/2.1.0/`, etc.

Eso significa que `latest.json` no debe vivir dentro de la carpeta de la versión. Debe quedar en la raiz lógica de Relay dentro del bucket, junto a las carpetas versionadas.

Ejemplo de layout en GCS:

- `gs://agent-updates-lab/utilities/relay/latest.json`
- `gs://agent-updates-lab/utilities/relay/2.0.0/ness_relay.zip`
- `gs://agent-updates-lab/utilities/relay/2.0.0/install_relay.sh`

### Que archivo lleva el SHA-256

El comando `sha256sum` debe aplicarse sobre el archivo que realmente se descarga y valida en el flujo del agente. En este sistema, la validacion principal se hace sobre el ZIP completo, no sobre la carpeta.

Por tanto:

1. Primero creas el ZIP final.
2. Luego ejecutas `sha256sum` sobre ese ZIP.
3. El resultado se copia al campo `pack.sha256` de `latest.json`.

Si adentro del ZIP incluyes el binario `ness-relay-x86_64` y el script `install_relay.sh`, el hash de referencia sigue siendo el del ZIP completo, porque eso es lo que descarga y verifica el agente.

### Que pasa con `files.ness_relay.sha256`

Ese valor puede usarse como referencia adicional de integridad para el binario interno, pero el flujo actual del agente valida de forma obligatoria el `pack.sha256` del paquete ZIP.

En otras palabras:

1. `pack.sha256` -> hash del ZIP publicado.
2. `files.ness_relay.sha256` -> hash del binario interno, opcional para auditoria o trazabilidad.

### Procedimiento recomendado para generar el paquete de prueba

1. Preparar una carpeta temporal con:
   - `ness-relay-x86_64`
   - `install_relay.sh`
2. Dar permisos de ejecucion al binario y al script.
3. Crear el ZIP final.
4. Calcular el SHA-256 del ZIP.
5. Subir el ZIP al folder versionado del bucket.
6. Actualizar `latest.json` con la version nueva, la URL del ZIP y el `pack.sha256` correcto.
7. Probar en desarrollo con `./ness_relay --update` o con `./ness_relay --continuous 5`.

### Ejemplo de flujo de publicacion

Si la nueva version es `2.1.0`, la publicacion quedaria asi:

1. Subir el ZIP a `utilities/relay/2.1.0/ness_relay-v2.1.0-linux-x86_64.zip`
2. Subir `latest.json` a `utilities/relay/latest.json`
3. Asegurarte de que `latest.json` apunte a esa URL exacta
4. Confirmar que `pack.sha256` corresponde al ZIP publicado
5. Ejecutar la prueba desde el agente de desarrollo

### Comandos utiles en Linux

Crear hash del ZIP:

```bash
sha256sum ness_relay-v2.1.0-linux-x86_64.zip
```

Crear ZIP:

```bash
zip -r ness_relay-v2.1.0-linux-x86_64.zip ness-relay-x86_64 install_relay.sh
```

Verificar contenido del ZIP:

```bash
unzip -l ness_relay-v2.1.0-linux-x86_64.zip
```

### Recomendacion para pruebas seguras

Antes de llevar esto a produccion, confirma estas tres cosas:

1. `latest.json` esta accesible publicamente o con la autenticacion esperada.
2. `pack.url` apunta al ZIP correcto.
3. El hash `pack.sha256` coincide exactamente con el ZIP subido.

Con eso el agente puede detectar la nueva version, descargarla y validar que el artefacto no fue alterado.

## Primera prueba en desarrollo

Con el ZIP `ness-relay-v2.0.1-linux-x86_64.zip` y el hash real ya generados, el siguiente orden recomendado para la primera prueba es:

1. Subir el ZIP al bucket de desarrollo en la carpeta de la version, por ejemplo `utilities/relay/2.0.1/`.
2. Actualizar `latest.json` en el mismo nivel que la carpeta de version.
3. Verificar que `latest.json` apunte a la URL exacta del ZIP.
4. Instalar el agente en una VM Linux de pruebas.
5. Ejecutar primero el modo manual para observar todo el flujo.
6. Si el manual funciona, ejecutar luego el modo continuo para validar el chequeo automatico.

### Que falta antes de la primera ejecucion manual

Para que la primera prueba sea correcta, deben quedar alineados estos puntos:

1. La version remota de `latest.json` debe ser mayor que la local.
2. El campo `pack.sha256` debe ser el hash del ZIP completo.
3. El campo `pack.url` debe coincidir con la ruta publica del artefacto.
4. La VM de pruebas debe tener acceso a GCP y al endpoint de reporte si se va a validar ese paso.

### Flujo manual recomendado para la prueba

1. Instalar el agente en la VM Linux.
2. Confirmar que el binario arranca y que los logs salen por consola si no se usa `--silent`.
3. Ejecutar:

```bash
./ness_relay --update
```

4. Observar la salida en consola:
   - verificacion de metadata remota,
   - comparacion de version,
   - descarga del ZIP,
   - verificacion del hash,
   - backup del binario,
   - reemplazo del ejecutable,
   - limpieza de backups,
   - mensaje de reinicio requerido.

### Sobre el sistema de salida que pides

Ya existe salida por consola en el modo manual y en el modo continuo mientras no se use `--silent`.

El comportamiento actual es:

1. `info!`, `warn!` y `error!` se imprimen en consola.
2. Lo mismo se registra en archivo de log.
3. Si ejecutas el proceso manualmente, veras el progreso paso a paso.

Si quieres una salida mas explicita todavia, la siguiente mejora natural seria agregar un modo de `verbose update` o un resumen estructurado por fases, pero para la primera prueba no es necesario porque ya tienes trazabilidad en consola.

### Comando de prueba manual recomendado

```bash
./ness_relay --update
```

### Comando de prueba automatica recomendado

```bash
./ness_relay --continuous 5
```

### Criterio de exito para la primera prueba

1. El agente detecta la version remota 2.0.1.
2. La descarga del ZIP se completa.
3. El SHA-256 coincide con `c27296b29dc3f591888e7293ee393eac96c6a7aacfd7b01126888280217ee623`.
4. El binario se reemplaza sin error.
5. El log muestra que se requiere reinicio o que el proceso se cierra para reiniciar de forma controlada.
