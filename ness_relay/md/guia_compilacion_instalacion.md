# NESS Relay v2.0.0 (Rust) — Guía completa de compilación, instalación y pruebas

> **Audiencia:** Esta guía está escrita para que cualquier persona con conocimientos básicos de terminal pueda seguirla paso a paso, sin necesidad de experiencia previa en Rust.

---

## Tabla de contenido

1. [Conceptos previos importantes](#1-conceptos-previos-importantes)
2. [Cómo compilar el agente en Ubuntu/Linux](#2-cómo-compilar-el-agente-en-ubuntulinux)
3. [Cómo compilar el agente en Windows](#3-cómo-compilar-el-agente-en-windows)
4. [¿Dónde queda el ejecutable compilado?](#4-dónde-queda-el-ejecutable-compilado)
5. [Cómo verificar que el agente se compiló correctamente](#5-cómo-verificar-que-el-agente-se-compiló-correctamente)
6. [Cómo ejecutar el agente dentro de la VM de Ubuntu](#6-cómo-ejecutar-el-agente-dentro-de-la-vm-de-ubuntu)
7. [Qué hacen build_relay.sh e install_relay.sh](#7-qué-hacen-build_relaysh-e-install_relaysh)
8. [Configuración del archivo devices.conf](#8-configuración-del-archivo-devicesconf)
9. [Prueba de fuego: pfSense con SNMPv3](#9-prueba-de-fuego-pfsense-con-snmpv3)
10. [Variables de entorno disponibles](#10-variables-de-entorno-disponibles)
11. [Cómo leer los logs del agente](#11-cómo-leer-los-logs-del-agente)
12. [Cómo verificar que el reporte se generó correctamente](#12-cómo-verificar-que-el-reporte-se-generó-correctamente)
13. [Cómo verificar que el reporte se envió al servidor](#13-cómo-verificar-que-el-reporte-se-envió-al-servidor)
14. [Prueba en múltiples distribuciones Linux](#14-prueba-en-múltiples-distribuciones-linux)
15. [Tabla de comandos de referencia rápida](#15-tabla-de-comandos-de-referencia-rápida)
16. [Solución de problemas comunes](#16-solución-de-problemas-comunes)

---

## 1. Conceptos previos importantes

### ¿Por qué el agente en Rust es diferente al Python?

El agente en Python necesita instalado Python, librerías, OpenSSL y muchas otras dependencias en cada máquina donde se ejecuta. Si la versión de Python o de OpenSSL no coincide, el agente falla.

El agente en Rust se compila una sola vez como un **binario estático** (`musl`). Esto significa que **todo lo que necesita para funcionar está adentro del ejecutable**: el runtime, las librerías criptográficas, el cliente SNMP, todo. El resultado es un único archivo que puedes copiar a cualquier Linux y ejecutar directamente, sin instalar nada más.

### ¿Qué es un binario musl estático?

Normalmente los programas en Linux usan `glibc`, que es la biblioteca del sistema. El problema es que la versión de `glibc` varía entre distribuciones (Ubuntu 20 tiene una versión, Ubuntu 22 otra, CentOS 7 otra completamente diferente). Si compilas en Ubuntu 22 y copias el binario a CentOS 7, falla con un error como `GLIBC_2.29 not found`.

Con `musl`, el binario lleva su propia copia de las bibliotecas básicas adentro. No necesita la del sistema. Por eso funciona en cualquier Linux: Ubuntu 18/20/22/24, Debian 10/11/12, CentOS 7/8, Fedora, RHEL, Alpine, etc. Solo requiere que el kernel de Linux sea versión 3.x o superior (cualquier distribución de los últimos 10 años cumple esto).

### Árbol de carpetas del proyecto

```
agentes/
├── GUIA_PRUEBAS_Y_COMPILACION.md    ← este archivo
├── python/
│   └── ness_relay_v2.0.0/           ← versión Python original
└── rust/
    └── ness_relay_v2.0.0/           ← versión Rust (nueva)
        ├── Cargo.toml               ← manifiesto del proyecto (equivale a requirements.txt)
        ├── build_relay.sh           ← script de compilación
        ├── install_relay.sh         ← script de instalación
        └── src/                     ← código fuente Rust
            ├── main.rs              ← punto de entrada, CLI
            ├── engine.rs            ← motor de recolección
            ├── config.rs            ← configuración y lectura de devices.conf
            ├── logging.rs           ← sistema de logs rotativos
            ├── updater.rs           ← auto-actualización
            ├── snmp/                ← stack SNMP puro Rust (v1/v2c/v3)
            ├── profiles/vendors/    ← perfiles por vendor (pfsense, fortinet, etc.)
            ├── collectors/          ← recolectores (sistema, red, perf, seguridad)
            ├── analyzers/           ← analizadores de alertas
            └── exporters/           ← exportadores (JSON local + envío HTTP)
```

---

## 2. Cómo compilar el agente en Ubuntu/Linux

### Paso 1: Instalar Rust (si no lo tienes)

Abre una terminal y ejecuta:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

El instalador te pregunta cómo instalar. Elige la opción `1` (instalación por defecto). Cuando termine:

```bash
# Cargar Rust en la sesión actual (solo la primera vez por sesión)
source "$HOME/.cargo/env"

# Verificar que quedó instalado
rustc --version
cargo --version
```

Deberías ver algo como:
```
rustc 1.77.0 (stable)
cargo 1.77.0 (stable)
```

### Paso 2: Instalar el target musl

Rust necesita saber que quieres compilar para Linux estático. Este comando descarga el compilador para ese target:

```bash
rustup target add x86_64-unknown-linux-musl
```

### Paso 3: Instalar las herramientas musl del sistema

```bash
# En Ubuntu / Debian
sudo apt update && sudo apt install -y musl-tools

# En Fedora / RHEL 8+ / CentOS 8+
sudo dnf install -y musl-gcc musl-libc-static

# En CentOS 7 / RHEL 7
sudo yum install -y musl-gcc

# En openSUSE
sudo zypper install -y musl-tools
```

### Paso 4: Ir a la carpeta del proyecto

```bash
cd /home/nessuser/agentes/rust/ness_relay_v2.0.0
```

### Paso 5: Compilar

**Opción A — Usar el script automático (recomendado):**

```bash
./build_relay.sh
```

El script hace todo automáticamente: instala dependencias si faltan, compila y pone el binario en `dist/`.

**Opción B — Compilar manualmente:**

```bash
RUSTFLAGS="-C target-feature=+crt-static" \
  cargo build --release --target x86_64-unknown-linux-musl
```

> **¿Qué significa cada parte?**
> - `RUSTFLAGS="-C target-feature=+crt-static"` — fuerza el enlace estático de la libc
> - `cargo build` — ordena a Cargo (el gestor de paquetes de Rust) que compile
> - `--release` — modo optimizado (sin este flag, el binario es lento y grande)
> - `--target x86_64-unknown-linux-musl` — especifica que queremos el target musl estático

La primera compilación tarda entre **2 y 8 minutos** porque descarga y compila todas las dependencias. Las siguientes compilaciones son mucho más rápidas porque usa una caché.

---

## 3. Cómo compilar el agente en Windows

Compilar en Windows para obtener un binario que se ejecute en Linux se llama **cross-compilation** (compilación cruzada). Hay dos formas:

### Opción A — Usar WSL2 (Windows Subsystem for Linux) — Recomendada

Esta es la forma más sencilla. Si tienes Windows 10 u 11, activa WSL2:

```powershell
# En PowerShell como Administrador:
wsl --install
```

Reinicia el PC, abre Ubuntu desde el menú de inicio y sigue exactamente los mismos pasos de la **Sección 2** de esta guía. El binario que obtengas funcionará en cualquier Linux.

### Opción B — Compilar directamente en Windows con cross-compilation

> ⚠️ Esta opción es más compleja. Solo la necesitas si no puedes usar WSL2.

**Paso 1:** Instala Rust para Windows desde https://rustup.rs (descarga el `.exe`)

**Paso 2:** Instala el target Linux musl desde PowerShell:

```powershell
rustup target add x86_64-unknown-linux-musl
```

**Paso 3:** Instala el linker cruzado. La forma más sencilla es usar la herramienta `cross`:

```powershell
cargo install cross
```

**Paso 4:** Compila con `cross` (necesita Docker Desktop instalado):

```powershell
cd C:\ruta\a\agentes\rust\ness_relay_v2.0.0
cross build --release --target x86_64-unknown-linux-musl
```

> **Recomendación:** Usa siempre **WSL2** en Windows. Es más rápido, más fácil y el resultado es idéntico.

---

## 4. ¿Dónde queda el ejecutable compilado?

Dependiendo de cómo compilaste:

| Método | Ruta del ejecutable |
|--------|-------------------|
| `./build_relay.sh` (script del proyecto) | `agentes/rust/ness_relay_v2.0.0/dist/ness-relay` |
| `cargo build --release --target ...` (manual) | `agentes/rust/ness_relay_v2.0.0/target/x86_64-unknown-linux-musl/release/ness-relay` |

El archivo se llama `ness-relay` (sin extensión, como todos los ejecutables en Linux).

Para copiarlo a un lugar más cómodo:

```bash
# Crear una carpeta de distribución limpia
mkdir -p /tmp/ness_relay_dist

# Copiar el binario
cp target/x86_64-unknown-linux-musl/release/ness-relay /tmp/ness_relay_dist/

# Ver el tamaño del binario
ls -lh /tmp/ness_relay_dist/ness-relay
```

El binario suele medir entre **5 y 12 MB** (es grande porque lleva todo adentro, pero mucho más pequeño que un ejecutable Python con todas sus dependencias).

---

## 5. Cómo verificar que el agente se compiló correctamente

### Verificación 1: Ver la versión

```bash
./target/x86_64-unknown-linux-musl/release/ness-relay --version
```

Salida esperada:
```
NESS Relay Multi-Vendor v2.0.0 (ness-relay)
```

### Verificación 2: Ver la ayuda

```bash
./target/x86_64-unknown-linux-musl/release/ness-relay --help
```

Salida esperada:
```
NESS Relay Multi-Vendor v2.0.0 — Agente de monitoreo SNMP

Usage: ness-relay [OPTIONS]

Options:
  -c, --config <FILE>       Ruta al archivo de configuración de dispositivos
      --continuous <MINUTES> Ejecutar en modo continuo con el intervalo especificado (en minutos)
      --update              Buscar e instalar actualizaciones del relay
      --silent              Silenciar la salida en consola
  -V, --version             Mostrar la versión y salir
  -h, --help                Print help
```

### Verificación 3: Confirmar que es un binario estático (la prueba más importante)

```bash
ldd ./target/x86_64-unknown-linux-musl/release/ness-relay
```

**Salida esperada (binario estático correcto):**
```
        not a dynamic executable
```
o en algunas distribuciones:
```
        statically linked
```

**Salida que indica un problema (binario dinámico, NO lo que queremos):**
```
        linux-vdso.so.1 => ...
        libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 ...
```

Si la salida dice `not a dynamic executable` ✓, el binario funcionará en cualquier Linux.

### Verificación 4: Comprobar el tipo de archivo

```bash
file ./target/x86_64-unknown-linux-musl/release/ness-relay
```

Salida esperada:
```
ness-relay: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), statically linked, stripped
```

La palabra clave es **`statically linked`** y **`stripped`** (sin símbolos de debug, más compacto).

---

## 6. Cómo ejecutar el agente dentro de la VM de Ubuntu

### ¿El agente se ejecuta por terminal o de forma gráfica?

El agente NESS Relay es una **herramienta de línea de comandos (CLI)**. No tiene interfaz gráfica. Esto es intencional: los agentes de monitoreo deben poder ejecutarse en servidores que no tienen pantalla ni entorno gráfico (el 99% de los servidores en producción son así).

Sin embargo, se puede ejecutar de ambas formas en la VM:
- **Por terminal:** directamente (para pruebas y debugging)
- **Por cron (automático):** para ejecución programada en producción, sin necesidad de que alguien esté presente

### Paso 1: Transferir el binario a la VM de Ubuntu

Desde la máquina donde compilaste, transfiere el binario a la VM:

```bash
# Usando SCP (reemplaza usuario@ip_de_la_vm con los datos reales)
scp target/x86_64-unknown-linux-musl/release/ness-relay usuario@192.168.x.x:/home/usuario/

# También puedes usar rsync
rsync -avz target/x86_64-unknown-linux-musl/release/ness-relay usuario@192.168.x.x:/home/usuario/
```

Si la VM está en la misma máquina (virtualización local), puedes copiar con:

```bash
# Si usas VirtualBox/VMware con carpetas compartidas
cp target/x86_64-unknown-linux-musl/release/ness-relay /media/sf_Compartida/

# Si usas VS Code Remote SSH, simplemente arrastra el archivo
```

### Paso 2: Entrar a la VM y dar permisos de ejecución

```bash
# Conectarse a la VM por SSH
ssh usuario@192.168.x.x

# En la VM: dar permisos de ejecución al binario
chmod +x /home/usuario/ness-relay

# Verificar que funciona
./ness-relay --version
```

### Paso 3: Crear la carpeta de trabajo y el archivo de configuración

El agente necesita saber qué dispositivos monitorear. Lee esta información del archivo `devices.conf`. Crea una carpeta de trabajo:

```bash
# Crear carpeta de trabajo del agente
mkdir -p /home/usuario/ness_relay_test
cd /home/usuario/ness_relay_test

# Mover el binario aquí
mv /home/usuario/ness-relay .

# Crear el archivo de configuración de dispositivos
nano devices.conf
```

Ejemplo de `devices.conf` para la prueba con pfSense SNMPv3 (más detalles en la Sección 8 y 9):

```ini
# pfSense con SNMPv3
pfsense_1_ip=192.168.x.x
pfsense_1_port=161
pfsense_1_snmp_version=3
pfsense_1_v3_user=ness_monitor
pfsense_1_v3_auth_protocol=SHA
pfsense_1_v3_auth_password=tu_password_de_auth
pfsense_1_v3_priv_protocol=AES128
pfsense_1_v3_priv_password=tu_password_de_priv
pfsense_1_description=Firewall pfSense Test
```

### Paso 4: Configurar las variables de entorno

El agente necesita saber a qué servidor NESS enviar los datos y con qué credenciales. Las URLs del servidor están **hardcodeadas dentro del ejecutable** por seguridad — solo se necesita indicar el **SERVER_ID** (1, 2 o 3):

- `1` = On-premise: `172.206.0.217:8080`
- `2` = Testing: `testing.nesshq.com`
- `3` = Public Cloud: `cloud.nesshq.com` (por defecto)

```bash
# Para pruebas manuales, exportar las variables directamente:
export NESS_SERVER_ID=3
export NESS_API_TOKEN=tu_token_de_api_aqui

# Opcionalmente, indicar dónde está el archivo de dispositivos:
export NESS_DEVICES_FILE=/home/usuario/ness_relay_test/devices.conf
```

> **Nota:** No es necesario definir `NESS_SERVER_URL` a menos que quieras forzar una URL personalizada. El ejecutable resuelve la URL automáticamente usando `NESS_SERVER_ID`.

### Paso 5: Ejecutar el agente (modo de prueba)

**Ejecución única (para ver qué pasa):**

```bash
cd /home/usuario/ness_relay_test
./ness-relay
```

El agente va a:
1. Leer `devices.conf` del mismo directorio
2. Conectarse al pfSense por SNMP
3. Recolectar datos del sistema, performance, interfaces, seguridad
4. Guardar el resultado en `relay_output/relay_data.json`
5. Enviar los datos al servidor NESS

Verás en la terminal una salida como esta:

```
[2026-03-10T10:00:00Z INFO  ness_relay] NESS Relay v2.0.0 iniciando — servidor: https://cloud.nesshq.com/api/relay/data/
[2026-03-10T10:00:00Z INFO  ness_relay::engine] [pfsense_1] Iniciando recolección — vendor=pfsense ip=192.168.x.x
[2026-03-10T10:00:00Z INFO  ness_relay::engine] [pfsense_1] [1/8] Perfil cargado: pfSense
[2026-03-10T10:00:00Z INFO  ness_relay::engine] [pfsense_1] [2/8] Probando conectividad SNMP…
[2026-03-10T10:00:00Z INFO  ness_relay::engine] [pfsense_1] [2/8] Conectividad OK
...
[2026-03-10T10:00:02Z INFO  ness_relay::engine] [pfsense_1] [8/8] Analizando alertas…
[2026-03-10T10:00:02Z INFO  ness_relay::engine] [pfsense_1] Recolección completada — 0 alertas
[2026-03-10T10:00:02Z INFO  ness_relay] Datos enviados al servidor NESS correctamente.
```

**Ejecución con una ruta de configuración explícita:**

```bash
./ness-relay --config /ruta/absoluta/a/devices.conf
```

**Ejecución continua (cada 5 minutos):**

```bash
./ness-relay --continuous 5
```

**Ejecución silenciosa (sin salida en terminal, solo logs en archivo):**

```bash
./ness-relay --silent
```

---

## 7. Qué hacen `build_relay.sh` e `install_relay.sh`

### `build_relay.sh` — El script de compilación

**Ruta:** `agentes/rust/ness_relay_v2.0.0/build_relay.sh`

Este script automatiza todos los pasos de compilación. Lo que hace internamente:

```
1. Verifica si Rust está instalado; si no, lo instala automáticamente
2. Añade el target x86_64-unknown-linux-musl
3. Detecta el sistema operativo (apt, dnf, yum, zypper) e instala musl-tools
4. Compila con RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target ...
5. Copia el binario a la carpeta dist/
6. Verifica que sea estático con ldd
7. Muestra el tamaño del binario
```

**Cómo usarlo:**

```bash
cd agentes/rust/ness_relay_v2.0.0

# Build estándar para x86_64 (64-bit normal)
./build_relay.sh

# Build para ARM64 (servidores ARM, Raspberry Pi, etc.)
./build_relay.sh --arch aarch64

# Build de debug (más lento, pero con información de error detallada)
./build_relay.sh --debug
```

**Dónde queda el resultado:**

```
agentes/rust/ness_relay_v2.0.0/
└── dist/
    └── ness-relay          ← el binario listo para distribuir
```

---

### `install_relay.sh` — El script de instalación en producción

**Ruta:** `agentes/rust/ness_relay_v2.0.0/install_relay.sh`

Este script instala el agente en el servidor/VM donde se va a usar (NO donde lo compilaste). Es un **instalador interactivo completo** que guía al usuario paso a paso. Lo que hace:

```
1. Muestra el banner de NESS Relay
2. Muestra los términos de uso y solicita aceptación (ACEPTO/rechazo)
3. Verifica que se ejecute como root (sudo)
4. Busca el binario ness-relay en dist/ o en el directorio actual
5. Solicita la selección del servidor (1=On-premise, 2=Testing, 3=Cloud)
6. Solicita el API Token de NESS HQ
7. Muestra menú interactivo de fabricantes con checkboxes:
   - Windows, Linux, Cisco, Fortinet, pfSense, MikroTik, UBNT, Cambium
   - MikroTik tiene sub-menú: RouterOS vs Firewall
8. Para cada fabricante seleccionado, solicita configuración de dispositivos:
   - IP/Host del dispositivo
   - Versión SNMP (v1, v2c, v3)
   - Para v1/v2c: community string
   - Para v3: usuario, protocolo auth (SHA/MD5), contraseña auth,
     protocolo de privacidad (AES128/AES192/AES256/DES), contraseña priv
   - Puerto SNMP (default: 161)
   - Descripción opcional
   - Opción de agregar más dispositivos del mismo fabricante
9. Muestra resumen y pide confirmación
10. Si ya existe /opt/ness_relay/, ofrece: reinstalar, actualizar o cancelar
    (con backup automático de la instalación anterior)
11. Crea la estructura de directorios organizada
12. Copia el binario a executables/
13. Configura variables de entorno en /etc/profile.d/ness_relay.sh
14. Genera devices.conf con toda la configuración de dispositivos
15. Crea view_config.sh (visor protegido de devices.conf, requiere API Token)
16. Crea run_relay.sh (wrapper inteligente: detecta terminal vs cron)
17. Configura cron job cada 5 minutos
18. Ofrece ejecutar una prueba inmediata para verificar
```

**Modos de uso:**

```bash
# Modo interactivo (recomendado — guía paso a paso):
sudo ./install_relay.sh

# Modo silencioso (para automatización con archivo de configuración):
sudo ./install_relay.sh --silent --config-file devices.conf --token TU_TOKEN --env 3

# Forzar reinstalación sin preguntar:
sudo ./install_relay.sh --force

# Ver ayuda:
./install_relay.sh --help
```

**Estructura final después de instalar:**

```
/opt/ness_relay/
├── configs/
│   └── devices.conf          ← configuración de dispositivos (permisos 600)
├── devices/                  ← datos de dispositivos monitoreados
├── executables/
│   ├── ness-relay            ← el binario ejecutable
│   ├── run_relay.sh          ← wrapper para ejecución (manual y cron)
│   └── view_config.sh        ← visor seguro de devices.conf
├── logs/
│   ├── install.log           ← log de la instalación
│   └── ness_relay.log        ← logs de ejecución
└── output/
    └── relay_data.json       ← el último reporte generado
```

**Variables de entorno (en `/etc/profile.d/ness_relay.sh`):**

```bash
export NESS_SERVER_ID="3"       # Solo el ID, las URLs están en el binario
export NESS_API_TOKEN="..."     # Token de autenticación de NESS HQ
export NESS_INSTALL_DIR="/opt/ness_relay"
export NESS_DEVICES_FILE="/opt/ness_relay/configs/devices.conf"
export NESS_OUTPUT_DIR="/opt/ness_relay/output"
export NESS_LOG_DIR="/opt/ness_relay/logs"
```

**El cron job que instala automáticamente:**

```
*/5 * * * * /opt/ness_relay/executables/run_relay.sh
```

Esto ejecuta el agente cada 5 minutos. El script `run_relay.sh` detecta automáticamente si se ejecuta desde un terminal interactivo (muestra diagnósticos) o desde cron (redirige al log).

**Seguridad:**
- `devices.conf` tiene permisos `600` (solo root puede leerlo)
- Para ver la configuración: `sudo /opt/ness_relay/executables/view_config.sh` (requiere el API Token como contraseña)
- Las URLs de los endpoints están hardcodeadas en el binario compilado — solo se almacena el SERVER_ID (1, 2 o 3)

---

## 8. Configuración del archivo `devices.conf`

El archivo `devices.conf` es el corazón de la configuración del agente. Usa un formato simple de `clave=valor`. Los dispositivos se identifican con el patrón: `{vendor}_{número}_{parámetro}`.

### Formato para SNMPv1 y SNMPv2c

```ini
# --------- pfSense con SNMPv2c ---------
pfsense_1_ip=192.168.1.1
pfsense_1_port=161
pfsense_1_snmp_version=2c
pfsense_1_community=public
pfsense_1_description=Firewall Principal

# --------- MikroTik con SNMPv1 ---------
mikrotik_1_ip=10.0.0.2
mikrotik_1_port=161
mikrotik_1_snmp_version=1
mikrotik_1_community=comunidad_privada
mikrotik_1_description=Router Sucursal Norte
```

### Formato para SNMPv3

```ini
# --------- pfSense con SNMPv3 ---------
pfsense_1_ip=192.168.1.1
pfsense_1_port=161
pfsense_1_snmp_version=3
pfsense_1_v3_user=ness_monitor
pfsense_1_v3_auth_protocol=SHA
pfsense_1_v3_auth_password=MiPasswordDeAuth2024!
pfsense_1_v3_priv_protocol=AES128
pfsense_1_v3_priv_password=MiPasswordDePriv2024!
pfsense_1_description=Firewall pfSense Sede Principal

# --------- Fortinet con SNMPv3 ---------
fortinet_1_ip=10.1.0.1
fortinet_1_port=161
fortinet_1_snmp_version=3
fortinet_1_v3_user=ness_monitor
fortinet_1_v3_auth_protocol=SHA
fortinet_1_v3_auth_password=PasswordAutenticacionForti!
fortinet_1_v3_priv_protocol=AES128
fortinet_1_v3_priv_password=PasswordPrivacidadForti!
fortinet_1_description=FortiGate DC
```

### Vendors soportados y sus nombres en el archivo

| Vendor real | Nombre en devices.conf | Protocolo SNMP |
|-------------|------------------------|----------------|
| pfSense | `pfsense` | v1, v2c, v3 |
| Fortinet FortiGate | `fortinet` | v1, v2c, v3 |
| MikroTik Router | `mikrotik` | v1, v2c, v3 |
| MikroTik Firewall | `mikrotik_fw` | v1, v2c, v3 |
| Cisco | `cisco` | v1, v2c, v3 |
| Ubiquiti (EdgeSwitch/etc.) | `ubnt` | v1, v2c, v3 |
| Cambium Networks | `c_n` | v1, v2c, v3 |
| Linux genérico | `linux` | v1, v2c, v3 |
| Windows + SNMP | `windows` | v1, v2c |
| Cualquier dispositivo | `generic` | v1, v2c, v3 |

### Parámetros SNMPv3 disponibles

| Parámetro | Valores posibles | Descripción |
|-----------|-----------------|-------------|
| `v3_user` | texto | Nombre del usuario SNMPv3 |
| `v3_auth_protocol` | `SHA`, `MD5`, `NONE` | Protocolo de autenticación |
| `v3_auth_password` | texto | Contraseña de autenticación (min. 8 chars) |
| `v3_priv_protocol` | `AES128`, `AES192`, `AES256`, `DES`, `NONE` | Protocolo de privacidad (cifrado) |
| `v3_priv_password` | texto | Contraseña de cifrado (min. 8 chars) |

---

## 9. Prueba de fuego: pfSense con SNMPv3

Esta es la prueba más completa porque SNMPv3 incluye autenticación (HMAC) y cifrado (AES), que son las partes más complejas del agente.

### Paso 1: Configurar SNMP en pfSense

1. Abre la interfaz web de pfSense (normalmente en `https://192.168.1.1`)
2. Ve a **Services → SNMP**
3. Activa el servicio SNMP (checkbox "Enable")
4. En **SNMP v3 Users**, agrega un nuevo usuario:
   - **Username:** `ness_monitor`
   - **Auth Type:** `SHA` (recomendado)
   - **Auth Passphrase:** una contraseña de mínimo 8 caracteres (ej: `NessAuth2024!`)
   - **Priv Type:** `AES` (recomendado)
   - **Privacy Passphrase:** otra contraseña de mínimo 8 caracteres (ej: `NessPriv2024!`)
5. Guarda los cambios

### Paso 2: Verificar conectividad SNMP desde la VM

Antes de ejecutar el agente, verifica que el puerto SNMP está accesible:

```bash
# En la VM de Ubuntu, instalar snmpget para probar
sudo apt install -y snmp

# Probar con SNMPv3 (reemplaza los valores por los tuyos)
snmpget -v3 -u ness_monitor -l authPriv \
  -a SHA -A "NessAuth2024!" \
  -x AES -X "NessPriv2024!" \
  192.168.1.1 sysDescr.0
```

Si esto responde con información del sistema pfSense, la conectividad está OK. Si no hay respuesta, verifica:
- El firewall de pfSense permite tráfico UDP en puerto 161 desde la IP de la VM
- La IP de la VM está en la lista de hosts permitidos en la configuración SNMP de pfSense

### Paso 3: Crear el devices.conf para esta prueba

```bash
mkdir -p ~/ness_prueba
cd ~/ness_prueba
cat > devices.conf << 'EOF'
# Prueba pfSense con SNMPv3
pfsense_1_ip=192.168.1.1
pfsense_1_port=161
pfsense_1_snmp_version=3
pfsense_1_v3_user=ness_monitor
pfsense_1_v3_auth_protocol=SHA
pfsense_1_v3_auth_password=NessAuth2024!
pfsense_1_v3_priv_protocol=AES128
pfsense_1_v3_priv_password=NessPriv2024!
pfsense_1_description=pfSense Prueba SNMPv3
EOF
```

### Paso 4: Configurar las credenciales del servidor NESS

```bash
export NESS_SERVER_ID=2   # Usar 2 para testing, 3 para producción
export NESS_API_TOKEN=tu_token_aqui
```

### Paso 5: Ejecutar el agente en modo verbose

```bash
cd ~/ness_prueba
./ness-relay --config ./devices.conf
```

### Paso 6: Qué esperar en la salida

Una ejecución exitosa con pfSense SNMPv3 se verá así:

```
INFO  ness_relay] NESS Relay v2.0.0 iniciando — servidor: https://testing.nesshq.com/api/relay/data/
INFO  ness_relay::engine] [pfsense_1] Iniciando recolección — vendor=pfsense ip=192.168.1.1
INFO  ness_relay::engine] [pfsense_1] [1/8] Perfil cargado: pfSense
INFO  ness_relay::engine] [pfsense_1] [2/8] Probando conectividad SNMP…
INFO  ness_relay::engine] [pfsense_1] [2/8] Conectividad OK
INFO  ness_relay::engine] [pfsense_1] [3/8] Recolectando sistema…
INFO  ness_relay::engine] [pfsense_1] [4/8] Recolectando performance…
INFO  ness_relay::engine] [pfsense_1] [5/8] Recolectando interfaces…
INFO  ness_relay::engine] [pfsense_1] [6/8] Recolectando seguridad…
INFO  ness_relay::engine] [pfsense_1] [7/8] Recolectando datos del vendor…
INFO  ness_relay::engine] [pfsense_1] [8/8] Analizando alertas…
INFO  ness_relay::engine] [pfsense_1] Recolección completada — 0 alertas
INFO  ness_relay] Datos enviados al servidor NESS correctamente.
```

### Paso 7: Verificar el reporte JSON generado

```bash
# Ver el reporte generado (relay_output se crea automáticamente)
cat relay_output/relay_data.json | python3 -m json.tool | head -80
```

Deberías ver algo como:

```json
{
  "relay_version": "2.0.0",
  "relay_type": "ness-relay",
  "server_id": "3",
  "timestamp": "2026-03-10T10:00:00Z",
  "devices": [
    {
      "device": {
        "id": "pfsense_1",
        "vendor": "pfsense",
        "ip": "192.168.1.1"
      },
      "system": {
        "description": "pfSense 2.7.2-RELEASE ...",
        "name": "pfSense.local",
        "uptime": {
          "days": 15, "hours": 3, "minutes": 42, ...
        }
      },
      "performance": {
        "cpu": { "cpu_usage_percent": 12.5 },
        "memory": { "used_gb": 1.2, "total_gb": 4.0, "usage_percent": 30.0 }
      },
      ...
    }
  ]
}
```

---

## 10. Variables de entorno disponibles

El agente no usa un archivo `.ini` de configuración global. En cambio, se configura con variables de entorno. Esto es estándar en sistemas de monitoreo y permite usar el mismo binario en distintos entornos sin recompilarlo.

Cuando se usa `install_relay.sh`, estas variables se configuran automáticamente en `/etc/profile.d/ness_relay.sh`.

| Variable | Descripción | Valor por defecto |
|----------|-------------|-------------------|
| `NESS_SERVER_ID` | ID del servidor NESS (1=On-premise, 2=Testing, 3=Cloud) | `3` |
| `NESS_API_TOKEN` | Token de API para autenticar con el servidor | _(vacío)_ |
| `NESS_SERVER_URL` | URL completa del servidor (override manual, normalmente no necesaria) | _(auto por SERVER_ID)_ |
| `NESS_DEVICES_FILE` | Ruta al archivo devices.conf | `<dir_del_binario>/devices.conf` |
| `NESS_OUTPUT_DIR` | Directorio para guardar relay_data.json | `<dir_del_binario>/relay_output` |
| `NESS_LOG_DIR` | Directorio para guardar los archivos de log | `<dir_del_binario>/logs` |
| `NESS_INSTALL_DIR` | Directorio de instalación | `/opt/ness_relay` |
| `NESS_HOSTING_URL` | URL base para descarga de actualizaciones | `https://nesshq.com/agents/ness-relay/linux/ubuntu` |
| `NESS_VERSION_CHECK_URL` | URL de verificación de versión | `{NESS_HOSTING_URL}/version.json` |
| `NESS_UPDATE_REPORT_URL` | URL para reportar actualizaciones realizadas | `https://nesshq.com/api/report-relay-update/` |

**Nota sobre las URLs del servidor:**

Las URLs de los endpoints están **hardcodeadas dentro del ejecutable** por seguridad. El instalador solo maneja IDs (1, 2, 3) sin exponer las rutas reales:

```bash
# El agente resuelve la URL internamente según NESS_SERVER_ID:
# 1 → http://172.206.0.217:8080/api/relay/data/
# 2 → https://testing.nesshq.com/api/relay/data/
# 3 → https://cloud.nesshq.com/api/relay/data/

# Para uso normal, solo define el SERVER_ID:
export NESS_SERVER_ID=3

# Solo si necesitas forzar una URL personalizada (no recomendado):
# export NESS_SERVER_URL=https://custom-server.example.com/api/relay/data/
```

---

## 11. Cómo leer los logs del agente

Los logs se guardan en el directorio `logs/` relativo al directorio donde está el binario (o en `NESS_LOG_DIR` si lo configuras).

### Estructura de los logs

```
logs/
├── relay.2026-03-10.log     ← log del día actual
├── relay.2026-03-09.log     ← log de ayer
└── relay.2026-03-08.log     ← anteayer
```

Los archivos rotan automáticamente cada día a medianoche. Se guardan todos los mensajes de nivel INFO y superior en el archivo.

### Ver los logs en tiempo real

```bash
# Ver el log del día en tiempo real (equivale a "tail -f")
tail -f logs/relay.$(date +%Y-%m-%d).log

# Ver las últimas 50 líneas del log
tail -50 logs/relay.$(date +%Y-%m-%d).log

# Buscar errores en el log
grep "ERROR\|WARN" logs/relay.$(date +%Y-%m-%d).log
```

### Niveles de log

| Nivel | Cuando aparece | ¿Sale en consola? |
|-------|---------------|-------------------|
| `TRACE` | Detalles de bajo nivel | No |
| `DEBUG` | Información de depuración | No |
| `INFO` | Progreso normal del agente | No (solo en archivo) |
| `WARN` | Algo no funcionó bien pero el agente sigue | Sí |
| `ERROR` | Error grave, la operación falló | Sí |

> **Nota:** En la consola (terminal) solo aparecen WARNING y ERROR. Los mensajes INFO y DEBUG solo van al archivo de log. Si quieres ver todos los mensajes en la consola (para debugging), usa la variable:
> ```bash
> RUST_LOG=debug ./ness-relay
> ```

---

## 12. Cómo verificar que el reporte se generó correctamente

### Verificación rápida

```bash
# Ver si el archivo existe y cuándo se modificó por última vez
ls -lh relay_output/relay_data.json

# Ver el tamaño del archivo (debe ser de varios KB, no 0 bytes)
wc -c relay_output/relay_data.json
```

### Verificación del contenido

```bash
# Ver el reporte completo formateado (si tienes python3)
python3 -m json.tool relay_output/relay_data.json

# Ver solo los campos principales del primer dispositivo
python3 -c "
import json
with open('relay_output/relay_data.json') as f:
    data = json.load(f)
d = data['devices'][0]
print('=== SISTEMA ===')
print('Descripción:', d['system'].get('description', 'N/A'))
print('Nombre:', d['system'].get('name', 'N/A'))
print()
print('=== CPU ===')
cpu = d.get('performance', {}).get('cpu', {})
print('Uso CPU:', cpu.get('cpu_usage_percent', 'N/A'), '%')
print()
print('=== MEMORIA ===')
mem = d.get('performance', {}).get('memory', {})
print('Uso Memoria:', mem.get('usage_percent', 'N/A'), '%')
print('Total:', mem.get('total_gb', 'N/A'), 'GB')
print()
print('=== ALERTAS ===')
alerts = d.get('alerts', [])
print(len(alerts), 'alertas generadas')
for a in alerts:
    print(' -', a['level'].upper(), ':', a['message'])
"
```

### Qué validar en el reporte

Para que la prueba sea exitosa, el reporte debe contener:

- ✅ `system.description` — contiene texto con la descripción del pfSense  
- ✅ `system.uptime` — objeto con días/horas/minutos de uptime
- ✅ `performance.cpu.cpu_usage_percent` — número entre 0 y 100
- ✅ `performance.memory.total_gb` — número mayor que 0
- ✅ `network` — array con las interfaces de red del pfSense
- ✅ `vendor_specific` — datos específicos de pfSense (estados del firewall, rx/tx WAN, etc.)
- ✅ `alerts` — array (puede estar vacío si todo está bien)

---

## 13. Cómo verificar que el reporte se envió al servidor

### Verificación en logs

```bash
grep "servidor NESS\|HTTP\|enviado" logs/relay.$(date +%Y-%m-%d).log
```

Busca estas líneas:
- `INFO ... Datos enviados al servidor NESS correctamente.` ✓ (éxito)
- `ERROR ... Error enviando al servidor NESS: ...` ✗ (fallo)

### Verificación con curl (simular lo que hace el agente)

```bash
# Verificar que el servidor responde
curl -s -o /dev/null -w "HTTP %{http_code}\n" \
  -H "Authorization: Token $NESS_API_TOKEN" \
  https://app.nesshq.com/api/relay/ping/
```

Respuesta esperada: `HTTP 200`

### Verificación en la plataforma NESS

1. Entra a `https://app.nesshq.com` con tu cuenta
2. Ve a la sección de Relays o Agentes
3. Busca el servidor con ID `NESS_SERVER_ID`
4. Verifica que la última actualización sea reciente

---

## 14. Prueba en múltiples distribuciones Linux

Una de las ventajas clave del binario estático es que funciona en cualquier distribución. Esta es la forma de probarlo:

### Usando Docker para probar múltiples distros sin instalar VMs

```bash
# Probar en Ubuntu 20.04
docker run --rm -v $(pwd):/test ubuntu:20.04 /test/ness-relay --version

# Probar en Ubuntu 18.04
docker run --rm -v $(pwd):/test ubuntu:18.04 /test/ness-relay --version

# Probar en Debian 11
docker run --rm -v $(pwd):/test debian:11 /test/ness-relay --version

# Probar en CentOS 7 (la más difícil por su glibc viejo)
docker run --rm -v $(pwd):/test centos:7 /test/ness-relay --version

# Probar en Alpine Linux (usa musl nativamente)
docker run --rm -v $(pwd):/test alpine:latest /test/ness-relay --version

# Probar en Fedora
docker run --rm -v $(pwd):/test fedora:latest /test/ness-relay --version
```

Todos deberían responder:
```
NESS Relay Multi-Vendor v2.0.0 (ness-relay)
```

### Instalación masiva (ejemplo para múltiples servidores)

Si tienes acceso SSH a múltiples servidores y quieres instalar el agente en todos:

```bash
#!/bin/bash
SERVIDORES=("192.168.1.100" "192.168.1.101" "10.0.0.50")
USUARIO="ubuntu"
BINARIO="./ness-relay"

for SERVER in "${SERVIDORES[@]}"; do
  echo "Instalando en $SERVER..."
  scp "$BINARIO" "$USUARIO@$SERVER:/tmp/ness-relay"
  ssh "$USUARIO@$SERVER" "
    sudo mkdir -p /opt/ness_relay
    sudo mv /tmp/ness-relay /opt/ness_relay/
    sudo chmod +x /opt/ness_relay/ness-relay
    /opt/ness_relay/ness-relay --version
  "
done
```

---

## 15. Tabla de comandos de referencia rápida

### Compilación

```bash
# Compilar (modo release, estático)
cd agentes/rust/ness_relay_v2.0.0
./build_relay.sh

# Compilar manualmente
RUSTFLAGS="-C target-feature=+crt-static" \
  cargo build --release --target x86_64-unknown-linux-musl

# Verificar que el binario es estático
ldd target/x86_64-unknown-linux-musl/release/ness-relay
# Resultado esperado: "not a dynamic executable"
```

### Ejecución

```bash
# Una sola ejecución
./ness-relay

# Con config específica
./ness-relay --config /ruta/devices.conf

# Ciclos cada 5 minutos
./ness-relay --continuous 5

# Sin output en consola
./ness-relay --silent

# Ver versión
./ness-relay --version

# Ver ayuda completa
./ness-relay --help

# Buscar actualizaciones
./ness-relay --update
```

### Logs y debugging

```bash
# Ver logs en tiempo real
tail -f logs/relay.$(date +%Y-%m-%d).log

# Ver TODOS los mensajes (modo debug)
RUST_LOG=debug ./ness-relay

# Buscar errores
grep "ERROR" logs/relay.*.log

# Ver el último reporte generado
cat relay_output/relay_data.json | python3 -m json.tool | head -100
```

### Variables de entorno (una línea)

```bash
NESS_SERVER_ID=3 NESS_API_TOKEN=mi_token NESS_ENV=testing ./ness-relay
```

---

## 16. Solución de problemas comunes

### ❌ Error: `permission denied` al ejecutar el binario

```bash
chmod +x ./ness-relay
```

### ❌ Error: `No such file or directory` al ejecutar en otra distro

Esto indica que el binario **no** es estático. Verifica con `ldd` y recompila con `--target x86_64-unknown-linux-musl` y `RUSTFLAGS="-C target-feature=+crt-static"`.

### ❌ Error: `No se encontraron dispositivos en devices.conf`

El agente no encuentra el archivo de configuración. Soluciones:
1. Verifica que `devices.conf` existe en el mismo directorio que el binario
2. Usa `--config` para especificar la ruta: `./ness-relay --config /ruta/completa/devices.conf`
3. Verifica que el archivo tiene al menos un dispositivo con el campo `_ip` definido

### ❌ Error: `Fallo de conectividad` o `timeout`

El agente no puede llegar al dispositivo SNMP. Pasos:
1. Verifica que el dispositivo está encendido y en la red
2. Verifica la IP en `devices.conf`
3. Verifica que el puerto UDP 161 está abierto: `nc -uvz 192.168.x.x 161`
4. Verifica que el community string o credenciales SNMPv3 son correctos
5. Verifica que la IP de la VM está en la lista de hosts permitidos en el dispositivo

### ❌ Error: `Autenticación rechazada` (HTTP 401/403 al enviar al servidor)

El token de API es incorrecto o expiró. Verifica:
```bash
echo $NESS_API_TOKEN   # debe tener el token correcto
```
Genera un nuevo token desde la plataforma NESS si es necesario.

### ❌ Error al compilar: `linker 'x86_64-linux-musl-gcc' not found`

```bash
# Ubuntu/Debian
sudo apt install musl-tools

# Verificar que está instalado
which x86_64-linux-musl-gcc
```

### ❌ El binario compila pero da errores SNMPv3: `Auth error`

Verifica:
1. La contraseña de autenticación tiene mínimo 8 caracteres
2. El protocolo configurado en `devices.conf` coincide exactamente con el del dispositivo:
   - `SHA` para SHA-1 (pfSense lo llama "SHA1" o "SHA")
   - `MD5` para MD5
3. Los campos en `devices.conf` usan los nombres completos:
   - `v3_auth_protocol` (no `v3_auth_proto`)
   - `v3_auth_password` (no `v3_auth_pass`)
   - `v3_priv_protocol` (no `v3_priv_proto`)
   - `v3_priv_password` (no `v3_priv_pass`)

### ❌ Warning: `NESS_API_TOKEN no está configurado`

El agente igualmente recolecta los datos y genera el JSON local, pero no puede enviarlos al servidor. Configura el token:
```bash
export NESS_API_TOKEN=tu_token
```

Si usaste `install_relay.sh`, el token se configura automáticamente en `/etc/profile.d/ness_relay.sh`. Recarga las variables con:
```bash
source /etc/profile.d/ness_relay.sh
```

---

*Documento generado para NESS Relay v2.0.0 — Network is Colombia S.A.S.*  
*Última actualización: Junio 2026*
