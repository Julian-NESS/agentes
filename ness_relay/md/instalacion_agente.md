# NESS Relay v2.0.0 — Guía completa de instalación del agente

> **Producto:** NESS Relay — Network Monitoring System  
> **Versión:** 2.0.0 Professional Multi-Vendor Edition  
> **Desarrollado por:** NETWORK IS COLOMBIA S.A.S  
> **Audiencia:** Técnicos e instaladores con acceso de administrador al servidor Linux donde se instalará el agente.

---

## Tabla de contenido

1. [Requisitos previos](#1-requisitos-previos)
2. [Archivos necesarios para la instalación](#2-archivos-necesarios-para-la-instalación)
3. [Verificar y asignar permisos de ejecución](#3-verificar-y-asignar-permisos-de-ejecución)
4. [Ejecutar el instalador](#4-ejecutar-el-instalador)
5. [Paso 1 — Aceptar términos y condiciones](#5-paso-1--aceptar-términos-y-condiciones)
6. [Paso 2 — Seleccionar el servidor de destino](#6-paso-2--seleccionar-el-servidor-de-destino)
7. [Paso 3 — Ingresar el API Token](#7-paso-3--ingresar-el-api-token)
8. [Paso 4 — Seleccionar fabricante y tipo de dispositivo](#8-paso-4--seleccionar-fabricante-y-tipo-de-dispositivo)
9. [Paso 5 — Configurar dispositivos por fabricante](#9-paso-5--configurar-dispositivos-por-fabricante)
10. [Paso 6 — Configuración de SNMP](#10-paso-6--configuración-de-snmp)
11. [Paso 7 — Confirmar y completar la instalación](#11-paso-7--confirmar-y-completar-la-instalación)
12. [Paso 8 — Primera ejecución del agente](#12-paso-8--primera-ejecución-del-agente)
13. [Estructura de archivos instalados](#13-estructura-de-archivos-instalados)
14. [Revisar los logs del agente](#14-revisar-los-logs-del-agente)
15. [Verificar la configuración de dispositivos guardada](#15-verificar-la-configuración-de-dispositivos-guardada)
16. [Verificar el cron (ejecución automática)](#16-verificar-el-cron-ejecución-automática)
17. [Comandos de referencia rápida](#17-comandos-de-referencia-rápida)
18. [Solución de problemas comunes](#18-solución-de-problemas-comunes)

---

## 1. Requisitos previos

Antes de comenzar la instalación, verifique que se cumplan los siguientes requisitos:

| Requisito | Detalle |
|-----------|---------|
| **Sistema operativo** | Linux 64-bit (Ubuntu 18+, Debian 10+, CentOS 7+, RHEL 8+) |
| **Kernel mínimo** | Linux 3.x o superior (cualquier distribución de los últimos 10 años) |
| **Python en el sistema** | ❌ NO requerido — el agente es un ejecutable autocontenido |
| **Permisos** | Acceso como `root` o mediante `sudo` |
| **Conectividad** | El servidor debe poder alcanzar los dispositivos a monitorear por SNMP (puerto 161 UDP por defecto) |
| **Conectividad saliente** | El servidor debe poder alcanzar el servidor NESS HQ por HTTPS |
| **API Token** | Token válido proporcionado por NESS HQ para autenticar el relay |

> **Nota:** El agente NESS Relay es un binario autocontenido compilado con PyInstaller. No requiere instalar Python, librerías ni dependencias adicionales en el sistema operativo.

---

## 2. Archivos necesarios para la instalación

La instalación requiere **dos archivos** ubicados en el **mismo directorio**:

```
/ruta/de/instalación/
├── install_relay.sh        ← Script de instalación interactivo
└── ness-relay-ubuntu       ← Ejecutable autocontenido del agente
```

> ⚠️ **Importante:** Ambos archivos deben estar en la misma carpeta. El script `install_relay.sh` buscará el ejecutable `ness-relay-ubuntu` en el directorio desde donde se ejecuta. Si el ejecutable no está presente, la instalación se detendrá con un error.

Para verificar que ambos archivos están presentes, ejecute:

```bash
ls -lh install_relay.sh ness-relay-ubuntu
```

La salida esperada debe mostrar ambos archivos:

```
-rwxrwxr-x 1 root root  45K mar 11 10:00 install_relay.sh
-rwxrwxr-x 1 root root 120M mar 11 10:00 ness-relay-ubuntu
```

---

## 3. Verificar y asignar permisos de ejecución

Para que el instalador y el ejecutable funcionen correctamente, ambos archivos deben tener **permisos de ejecución**.

### 3.1 Verificar permisos actuales

```bash
ls -l install_relay.sh ness-relay-ubuntu
```

Los permisos correctos se muestran con una `x` en la columna de permisos, por ejemplo:

```
-rwxrwxr-x  → tiene permisos de ejecución ✅
-rw-rw-r--  → NO tiene permisos de ejecución ❌
```

### 3.2 Asignar permisos si es necesario

Si algún archivo no tiene permisos de ejecución, asígnelos con el comando `chmod`:

```bash
# Asignar permisos a ambos archivos de una vez
chmod 775 install_relay.sh ness-relay-ubuntu
```

O de forma individual:

```bash
chmod 775 install_relay.sh
chmod 775 ness-relay-ubuntu
```

> **Explicación de `chmod 775`:**  
> - `7` (propietario): lectura + escritura + ejecución  
> - `7` (grupo): lectura + escritura + ejecución  
> - `5` (otros): lectura + ejecución

### 3.3 Confirmar que los permisos se aplicaron correctamente

```bash
ls -l install_relay.sh ness-relay-ubuntu
```

Ambos archivos deben mostrar `-rwxrwxr-x` o similar con la `x` presente.

---

## 4. Ejecutar el instalador

El instalador **debe ejecutarse como root**. Use `sudo` para garantizar los permisos necesarios para crear directorios en `/opt/` y configurar el cron del sistema.

```bash
sudo ./install_relay.sh
```

Al ejecutarlo, el instalador mostrará el banner corporativo de NESS Relay y comenzará el proceso de instalación guiada.

### Modo silencioso (opcional, para automatización)

Para instalaciones automatizadas sin interacción del usuario:

```bash
sudo ./install_relay.sh --silent --config-file devices.conf --token TU_API_TOKEN
```

> En este documento se describe el proceso **interactivo** (sin `--silent`), que es el modo recomendado para primeras instalaciones.

---

## 5. Paso 1 — Aceptar términos y condiciones

Al iniciar, el instalador mostrará la pantalla de **Términos de Uso y Licencia** que incluye:

- Información del desarrollador (NETWORK IS COLOMBIA S.A.S)
- Aviso de copyright (© 2026)
- Restricciones y prohibiciones de uso
- Consecuencias legales del uso indebido
- Política de privacidad y datos
- Limitación de garantía

Al final de la pantalla aparecerá el siguiente prompt:

```
¿Acepta los términos y condiciones? (ACEPTO/rechazo):
```

**Respuestas válidas:**

| Respuesta | Resultado |
|-----------|-----------|
| `ACEPTO` | Acepta los términos. La instalación continúa. |
| `rechazo` o `RECHAZO` | Rechaza los términos. La instalación se cancela. |

> ⚠️ **La respuesta es sensible a mayúsculas:** Debe escribir exactamente `ACEPTO` (en mayúsculas) para continuar. Cualquier otra respuesta mostrará un mensaje de error y volverá a solicitar la confirmación.

```bash
# Ejemplo de respuesta correcta
¿Acepta los términos y condiciones? (ACEPTO/rechazo): ACEPTO
```

Una vez aceptados, el instalador mostrará:

```
✅ Términos aceptados. Continuando con la instalación...
```

---

## 6. Paso 2 — Seleccionar el servidor de destino

El instalador solicitará que seleccione el entorno del servidor NESS HQ al cual el agente enviará los datos recolectados:

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                          CONFIGURACIÓN DEL SERVIDOR                          ║
╚══════════════════════════════════════════════════════════════════════════════╝

Selecciona el entorno del servidor:
  1) On-premise: 172.206.0.217
  2) Testing: testing.nesshq.com
  3) Public Cloud: cloud.nesshq.com

Ingresa 1, 2 o 3 [default: 3]:
```

**Opciones disponibles:**

| Opción | Entorno | Destino |
|--------|---------|---------|
| `1` | On-premise | Servidor local en la red del cliente (`172.206.0.217`) |
| `2` | Testing | Servidor de pruebas (`testing.nesshq.com`) |
| `3` | Public Cloud | Nube pública NESS HQ (`cloud.nesshq.com`) — **Valor por defecto** |

Ingrese el número correspondiente y presione **Enter**. Si presiona Enter sin ingresar nada, se usará la opción `3` (Public Cloud) por defecto.

```bash
Ingresa 1, 2 o 3 [default: 3]: 3
```

> **Nota de seguridad:** Las URLs completas de los endpoints están protegidas dentro del ejecutable compilado. El instalador solo guarda el identificador numérico del servidor (`1`, `2` o `3`) en las variables de entorno del sistema, sin exponer las rutas reales.

---

## 7. Paso 3 — Ingresar el API Token

El API Token es la credencial que identifica y autentica el relay ante el servidor NESS HQ. Es obligatorio para que el agente pueda enviar datos.

```
🔑 Ingresa el API Token de NESS HQ:
```

Ingrese el token que le fue proporcionado por el administrador de NESS HQ y presione **Enter**.

> ⚠️ **El campo no puede quedar vacío.** Si no ingresa ningún valor, el instalador volverá a solicitar el token hasta que se proporcione uno.

```bash
🔑 Ingresa el API Token de NESS HQ: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

Una vez ingresado correctamente:

```
✅ Token de API configurado
```

> **Dónde obtener el token:** El API Token se genera desde el panel de administración de NESS HQ, en la sección de configuración del cliente o sitio. Cada instalación de relay debe tener su propio token.

---

## 8. Paso 4 — Seleccionar fabricante y tipo de dispositivo

El instalador mostrará el menú de selección de fabricantes. Aquí debe indicar qué tipo de dispositivos va a monitorear.

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                     SELECCIÓN DE FABRICANTES/DISPOSITIVOS                    ║
╚══════════════════════════════════════════════════════════════════════════════╝

1. [   DISPONIBLE] Windows Servers
2. [   DISPONIBLE] Linux Servers
3. [   DISPONIBLE] Cisco Devices
4. [   DISPONIBLE] Fortinet Firewalls
5. [   DISPONIBLE] pfSense Firewalls
6. [   DISPONIBLE] MikroTik Devices ▶
7. [   DISPONIBLE] Ubiquiti Switches (UBNT)
8. [   DISPONIBLE] Cambium Networks APs

Opciones:
  a. Seleccionar/Deseleccionar todos
  c. Continuar con la instalación
  q. Salir
```

**Fabricantes soportados:**

| # | Fabricante | Tipos de dispositivo |
|---|------------|---------------------|
| 1 | **Windows Servers** | Servidores Windows con agente SNMP habilitado |
| 2 | **Linux Servers** | Servidores Linux con agente SNMP (`snmpd`) |
| 3 | **Cisco Devices** | Routers, switches y equipos Cisco |
| 4 | **Fortinet Firewalls** | Firewalls FortiGate |
| 5 | **pfSense Firewalls** | Firewalls pfSense / OPNsense |
| 6 | **MikroTik Devices ▶** | Abre sub-menú (ver abajo) |
| 7 | **Ubiquiti Switches (UBNT)** | Switches y APs Ubiquiti UniFi / EdgeOS |
| 8 | **Cambium Networks APs** | Puntos de acceso Cambium Networks |

### 8.1 Cómo seleccionar un fabricante

Escriba el número correspondiente al fabricante y presione **Enter**. Al seleccionarlo, el estado cambiará a `[✓ SELECCIONADO]` y el instalador abrirá inmediatamente la pantalla de configuración de dispositivos para ese fabricante (ver [Paso 5](#9-paso-5--configurar-dispositivos-por-fabricante)).

Puede seleccionar **múltiples fabricantes**. Después de configurar los dispositivos de un fabricante, volverá al menú para seleccionar el siguiente.

### 8.2 Sub-menú MikroTik

Al seleccionar la opción `6` (MikroTik), se abre un sub-menú para elegir el perfil específico:

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                       DISPOSITIVOS MIKROTIK                                  ║
╚══════════════════════════════════════════════════════════════════════════════╝

  1. [   DISPONIBLE] RouterOS — Routers y Switches
     Modelos: CHR, CCR, RB series (como router/switch).
     Monitoreo: CPU, memoria, disco, health (temp/voltaje), wireless.

  2. [   DISPONIBLE] Firewall — Gateway y Perimetral (CHR/CCR/RB)
     Modelos: CHR, CCR2004/2116/1036, RB4011, RB3011, RB1100, L009.
     Monitoreo: + Netwatch (ISP probes), interfaces WAN, canales de
     Internet (ETB/Tigo/Claro), Queue Simple (ancho de banda por canal).

  b. Volver al menú principal

Seleccione sub-tipo MikroTik [1/2/b]:
```

| Opción | Perfil | Uso recomendado |
|--------|--------|-----------------|
| `1` | RouterOS | Dispositivos MikroTik usados como router o switch de distribución |
| `2` | Firewall | Dispositivos MikroTik usados como gateway/perimetral con múltiples ISPs |
| `b` | Volver | Regresa al menú principal sin seleccionar |

> **Nota:** Ambos perfiles usan la misma MIKROTIK-MIB. La diferencia está en las métricas adicionales que activa el perfil Firewall (Netwatch, Queue Simple, interfaces WAN).

---

## 9. Paso 5 — Configurar dispositivos por fabricante

Una vez seleccionado un fabricante, el instalador abre la pantalla de configuración de dispositivos. Aquí registrará cada dispositivo que desea monitorear.

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                       CONFIGURACIÓN: [Nombre del Fabricante]                 ║
╚══════════════════════════════════════════════════════════════════════════════╝

📡 Dispositivo #1 para [Nombre del Fabricante]:
  🌐 IP/Host del dispositivo (o 'fin' para terminar):
```

### 9.1 Ingresar la dirección IP

Ingrese la **dirección IP** o el **hostname** del dispositivo a monitorear y presione **Enter**.

```bash
🌐 IP/Host del dispositivo (o 'fin' para terminar): 192.168.1.1
```

> - El campo no puede quedar vacío.
> - Para terminar de agregar dispositivos de un fabricante sin agregar más, escriba `fin` y presione **Enter**.

### 9.2 Agregar múltiples dispositivos del mismo fabricante

Al finalizar la configuración de un dispositivo, el instalador preguntará:

```
¿Agregar otro dispositivo [Fabricante]? (y/N):
```

- Ingrese `y` para agregar otro dispositivo del mismo fabricante.
- Presione **Enter** o ingrese `N` para terminar con ese fabricante y volver al menú de selección.

---

## 10. Paso 6 — Configuración de SNMP

Para cada dispositivo, el instalador solicitará la configuración del protocolo SNMP que tiene habilitado el dispositivo.

### 10.1 Selección de versión SNMP

```
  Selecciona la versión de SNMP:
    1) SNMPv1  (Community string - protocolo legacy, sin cifrado)
    2) SNMPv2c (Community string - mejor rendimiento, sin cifrado)
    3) SNMPv3  (Usuario/Contraseña - RECOMENDADO: con autenticación y cifrado)
  Selecciona 1, 2 o 3 [default: 3]:
```

| Versión | Seguridad | Rendimiento | Recomendado |
|---------|-----------|-------------|-------------|
| SNMPv1 | ❌ Sin cifrado, sin autenticación | Bajo | Solo si el dispositivo no soporta versiones superiores |
| SNMPv2c | ❌ Sin cifrado, community string | Alto | Compatible con la mayoría de dispositivos legacy |
| SNMPv3 | ✅ Con autenticación y cifrado | Alto | **Siempre que sea posible** |

Presione **Enter** sin ingresar nada para usar SNMPv3 por defecto.

---

### 10.2 Configuración SNMPv1 y SNMPv2c (Community String)

Si selecciona la versión `1` o `2`, el instalador solicitará únicamente el **community string**:

```
⚠️  SNMPv2c seleccionado - Sin cifrado de datos
🔑 Community string SNMP [default: public]:
```

Ingrese el community string configurado en el dispositivo. Si presiona **Enter** sin ingresar nada, se usará `public` por defecto.

```bash
🔑 Community string SNMP [default: public]: miCommunity123
```

> **Advertencia:** El community string viaja en texto plano por la red. Se recomienda usar SNMPv3 siempre que el dispositivo lo soporte.

---

### 10.3 Configuración SNMPv3 (Recomendado)

SNMPv3 requiere credenciales de autenticación y (opcionalmente) cifrado. El instalador solicitará cada parámetro paso a paso.

#### Usuario SNMPv3

```
  ═══ Configuración SNMPv3 ═══
  👤 Usuario SNMPv3:
```

Ingrese el nombre de usuario SNMP configurado en el dispositivo. Este campo es obligatorio.

```bash
👤 Usuario SNMPv3: adminsnmp
```

#### Protocolo de autenticación

```
  Protocolo de Autenticación:
    1) SHA (recomendado)
    2) MD5
    3) NONE (sin autenticación - no recomendado)
  Selecciona 1, 2 o 3 [default: 1]:
```

| Opción | Protocolo | Recomendación |
|--------|-----------|---------------|
| `1` | **SHA** | ✅ Recomendado — más seguro |
| `2` | MD5 | Compatible con dispositivos más antiguos |
| `3` | NONE | ❌ Sin autenticación — no recomendado |

Presione **Enter** para usar SHA por defecto.

#### Contraseña de autenticación

```
  🔐 Contraseña de Autenticación (mín. 8 caracteres):
```

Ingrese la contraseña de autenticación del usuario SNMP. Debe tener **al menos 8 caracteres**. La contraseña no se mostrará en pantalla mientras la escribe.

```bash
🔐 Contraseña de Autenticación (mín. 8 caracteres): MiPassAuth2024
```

#### Protocolo de privacidad (cifrado)

```
  Protocolo de Privacidad (Encriptación):
    1) AES128 (recomendado)
    2) AES192
    3) AES256 (máxima seguridad)
    4) DES (obsoleto)
    5) NONE (sin encriptación)
  Selecciona 1-5 [default: 1]:
```

| Opción | Protocolo | Recomendación |
|--------|-----------|---------------|
| `1` | **AES128** | ✅ Recomendado — balance seguridad/rendimiento |
| `2` | AES192 | Mayor seguridad, mayor carga computacional |
| `3` | AES256 | Máxima seguridad |
| `4` | DES | ❌ Obsoleto — vulnerable |
| `5` | NONE | Sin cifrado de datos |

Presione **Enter** para usar AES128 por defecto.

#### Contraseña de privacidad (cifrado)

```
  🔐 Contraseña de Privacidad (mín. 8 caracteres):
```

Ingrese la contraseña de privacidad (cifrado). Puede ser igual o diferente a la contraseña de autenticación. Debe tener **al menos 8 caracteres**.

```bash
🔐 Contraseña de Privacidad (mín. 8 caracteres): MiPassPriv2024
```

---

### 10.4 Puerto SNMP

Una vez configurado el tipo de SNMP y sus credenciales, el instalador solicitará el puerto:

```
🔌 Puerto SNMP [default: 161]:
```

El puerto estándar SNMP es el **161**. Presione **Enter** para usarlo por defecto, o ingrese un puerto diferente si el dispositivo tiene SNMP en un puerto personalizado.

```bash
🔌 Puerto SNMP [default: 161]: 161
```

### 10.5 Descripción del dispositivo (opcional)

```
📝 Descripción del dispositivo [opcional]:
```

Ingrese una descripción amigable para identificar el dispositivo (por ejemplo: "Firewall Principal Bogotá", "Switch Piso 3", "Router ISP ETB"). Este campo es opcional.

```bash
📝 Descripción del dispositivo [opcional]: Firewall principal - Sede central
```

### 10.6 Confirmación del dispositivo configurado

Al completar todos los campos, el instalador mostrará la confirmación:

**Para SNMPv2c:**
```
✅ Dispositivo SNMPv2c configurado: 192.168.1.1 (Firewall principal - Sede central)
⚠️  Recordatorio: SNMPv2c no cifra datos - considere SNMPv3
```

**Para SNMPv3:**
```
✅ Dispositivo SNMPv3 configurado: 192.168.1.1 (usuario: adminsnmp)
```

---

## 11. Paso 7 — Confirmar y completar la instalación

### 11.1 Resumen de configuración

Una vez configurados todos los fabricantes y dispositivos, el instalador mostrará un **resumen completo** antes de proceder:

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                          RESUMEN DE CONFIGURACIÓN                            ║
╚══════════════════════════════════════════════════════════════════════════════╝

[✓] pfSense Firewalls        → 2 dispositivos
[✓] MikroTik RouterOS        → 3 dispositivos
[✓] Cisco Devices            → 1 dispositivo

Total de dispositivos a monitorear: 6
```

### 11.2 Confirmación final

```
¿Continuar con la instalación? (Y/n):
```

- Presione **Enter** o ingrese `Y` para continuar.
- Ingrese `n` para cancelar sin realizar cambios.

### 11.3 Proceso de instalación automática

Al confirmar, el instalador realizará automáticamente las siguientes acciones:

1. **Crea la estructura de directorios** en `/opt/ness_relay/`
2. **Copia el ejecutable** `ness-relay-ubuntu` a `/opt/ness_relay/executables/`
3. **Configura las variables de entorno** en `/etc/profile.d/ness_relay.sh`
4. **Genera el archivo de configuración** `devices.conf` con todos los dispositivos ingresados
5. **Aplica permisos de seguridad** al archivo de configuración (`chmod 600`)
6. **Crea el script de ejecución** `run_relay.sh`
7. **Crea el script de acceso seguro** `view_config.sh` (protegido por contraseña)
8. **Configura el cron** para ejecución automática cada 5 minutos

Cada paso se muestra con su estado en tiempo real:

```
✅ Estructura de directorios creada
✅ Ejecutable instalado en: /opt/ness_relay/executables/ness-relay-ubuntu
✅ Variables de entorno configuradas en: /etc/profile.d/ness_relay.sh
✅ Configuración guardada en: /opt/ness_relay/configs/devices.conf
✅ Permisos de seguridad aplicados a devices.conf (600 - solo root)
✅ Script de ejecución creado: /opt/ness_relay/executables/run_relay.sh
✅ Script de protección creado: /opt/ness_relay/executables/view_config.sh
✅ Tarea programada configurada (cada 5 minutos)
```

---

## 12. Paso 8 — Primera ejecución del agente

Al finalizar la instalación, el instalador ofrecerá ejecutar el agente por primera vez para verificar que la configuración es correcta:

```
¿Desea ejecutar el relay por primera vez ahora para verificar? (Y/n):
```

Se recomienda responder `Y` (o simplemente presionar **Enter**) para validar la instalación inmediatamente.

### 12.1 Qué sucede durante la primera ejecución

El agente realizará:
1. Carga de variables de entorno y configuración
2. Conexión SNMP a cada dispositivo configurado
3. Recolección de métricas según el perfil del fabricante
4. Exportación de los datos en formato JSON a `/opt/ness_relay/output/relay_data.json`
5. Envío de los datos al servidor NESS HQ seleccionado
6. Registro de resultados en los logs

### 12.2 Ejecutar manualmente en cualquier momento

Si omitió la prueba durante la instalación, puede ejecutar el agente manualmente en cualquier momento:

```bash
sudo /opt/ness_relay/executables/run_relay.sh
```

La salida en terminal mostrará el progreso en tiempo real con el resultado de cada dispositivo.

### 12.3 Interpretación de la salida

Una ejecución exitosa mostrará mensajes similares a:

```
⏳ Iniciando recolección para 6 dispositivos...
✅ [192.168.1.1] pfSense - Datos recolectados correctamente
✅ [10.0.0.1]   MikroTik RouterOS - Datos recolectados correctamente
✅ Reporte generado en: /opt/ness_relay/output/relay_data.json
✅ Datos enviados al servidor NESS HQ correctamente
✅ Relay ejecutado exitosamente
```

Si hay errores de conexión SNMP, se verán mensajes como:

```
❌ [192.168.1.5] Error de conexión SNMP: timeout (verifique IP, community y puerto)
```

---

## 13. Estructura de archivos instalados

Después de una instalación exitosa, la estructura de directorios en el sistema será:

```
/opt/ness_relay/
├── configs/
│   └── devices.conf              ← Configuración de dispositivos (permisos 600, solo root)
├── devices/                      ← Datos históricos de dispositivos monitoreados
├── executables/
│   ├── ness-relay-ubuntu         ← Ejecutable principal del agente
│   ├── run_relay.sh              ← Script de ejecución (usado por cron y manualmente)
│   └── view_config.sh            ← Visor seguro de configuración (protegido por contraseña)
├── logs/
│   ├── install.log               ← Log del proceso de instalación
│   └── ness_relay.log            ← Log de operación del agente (crece con cada ejecución)
└── output/
    └── relay_data.json           ← Último reporte JSON generado por el agente

/etc/profile.d/
└── ness_relay.sh                 ← Variables de entorno del sistema (NESS_SERVER_ID, NESS_API_TOKEN)
```

**Variables de entorno configuradas en `/etc/profile.d/ness_relay.sh`:**

| Variable | Descripción |
|----------|-------------|
| `NESS_SERVER_ID` | Identificador del servidor destino (1, 2 o 3) |
| `NESS_API_TOKEN` | Token de autenticación del relay |
| `NESS_INSTALL_DIR` | Ruta de instalación (`/opt/ness_relay`) |

---

## 14. Revisar los logs del agente

Los logs son la herramienta principal para monitorear el funcionamiento del agente y diagnosticar problemas.

### 14.1 Ver los logs en tiempo real

```bash
tail -f /opt/ness_relay/logs/ness_relay.log
```

Este comando muestra los últimos registros y actualiza la pantalla cada vez que el agente escribe nuevas líneas. Use `Ctrl+C` para salir.

### 14.2 Ver los últimos N registros

```bash
# Ver las últimas 50 líneas del log
tail -n 50 /opt/ness_relay/logs/ness_relay.log

# Ver las últimas 100 líneas
tail -n 100 /opt/ness_relay/logs/ness_relay.log
```

### 14.3 Filtrar solo errores en los logs

```bash
grep -i "error\|ERROR" /opt/ness_relay/logs/ness_relay.log | tail -n 50
```

### 14.4 Filtrar logs por dispositivo (IP)

```bash
grep "192.168.1.1" /opt/ness_relay/logs/ness_relay.log | tail -n 30
```

### 14.5 Ver el log de instalación

```bash
cat /opt/ness_relay/logs/install.log
```

### 14.6 Interpretar los niveles de log

| Prefijo | Significado |
|---------|-------------|
| `[INFO]` | Información general del proceso |
| `[SUCCESS]` | Operación completada exitosamente |
| `[WARNING]` | Advertencia que no detiene el proceso |
| `[ERROR]` | Error que puede afectar el monitoreo |
| `[PROGRESS]` | Paso en progreso |

---

## 15. Verificar la configuración de dispositivos guardada

El archivo `devices.conf` contiene toda la configuración de los dispositivos en texto plano, incluyendo credenciales SNMP. Por este motivo está protegido con permisos `600` (solo lectura para root).

### 15.1 Ver la configuración de forma segura

El instalador crea un script especial que solicita autenticación antes de mostrar el archivo:

```bash
sudo /opt/ness_relay/executables/view_config.sh
```

El script solicitará la contraseña de acceso (que es el mismo `NESS_API_TOKEN` configurado durante la instalación):

```
╔═══════════════════════════════════════════════════════════╗
║       🔐  NESS RELAY - Acceso Protegido              ║
╚═══════════════════════════════════════════════════════════╝

Este archivo contiene información sensible de los dispositivos.
Se requiere autenticación para acceder.

Ingrese la contraseña de acceso:
```

Ingrese el API Token y presione **Enter**. Si es correcto, se mostrará el contenido del archivo.

### 15.2 Ver directamente como root (solo administradores)

```bash
sudo cat /opt/ness_relay/configs/devices.conf
```

### 15.3 Verificar el último JSON generado

Para confirmar que el agente está recolectando datos correctamente:

```bash
# Ver el JSON generado (forma estructurada)
sudo cat /opt/ness_relay/output/relay_data.json | python3 -m json.tool | head -n 50

# Verificar cuándo fue generado por última vez
ls -lh /opt/ness_relay/output/relay_data.json
```

---

## 16. Verificar el cron (ejecución automática)

El instalador configura automáticamente el cron para ejecutar el agente **cada 5 minutos**.

### 16.1 Verificar que el cron está activo

```bash
crontab -l | grep ness_relay
```

La salida esperada es:

```
*/5 * * * * /opt/ness_relay/executables/run_relay.sh
```

### 16.2 Ver el historial de ejecuciones del cron

```bash
grep "ness_relay" /var/log/syslog | tail -n 20
# En sistemas CentOS/RHEL:
grep "ness_relay" /var/log/cron | tail -n 20
```

### 16.3 Modificar la frecuencia de ejecución (opcional)

Si necesita cambiar la frecuencia de ejecución, edite el crontab:

```bash
crontab -e
```

Cambie la línea existente. Ejemplos de configuraciones comunes:

```bash
# Cada 5 minutos (configuración por defecto)
*/5 * * * * /opt/ness_relay/executables/run_relay.sh

# Cada 10 minutos
*/10 * * * * /opt/ness_relay/executables/run_relay.sh

# Cada hora
0 * * * * /opt/ness_relay/executables/run_relay.sh
```

---

## 17. Comandos de referencia rápida

### Operación del agente

| Acción | Comando |
|--------|---------|
| Ejecutar el relay manualmente | `sudo /opt/ness_relay/executables/run_relay.sh` |
| Ver logs en tiempo real | `tail -f /opt/ness_relay/logs/ness_relay.log` |
| Ver últimos 100 errores | `tail -n 100 /opt/ness_relay/logs/ness_relay.log \| grep -i error` |
| Ver configuración guardada | `sudo /opt/ness_relay/executables/view_config.sh` |
| Ver el último JSON generado | `sudo cat /opt/ness_relay/output/relay_data.json` |
| Ver el cron configurado | `crontab -l \| grep ness_relay` |

### Diagnóstico del sistema

| Acción | Comando |
|--------|---------|
| Ver estructura de instalación | `tree -L 2 /opt/ness_relay` |
| Ver variables de entorno NESS | `cat /etc/profile.d/ness_relay.sh` |
| Verificar que el ejecutable existe | `ls -lh /opt/ness_relay/executables/ness-relay-ubuntu` |
| Probar conectividad SNMP manual | `snmpwalk -v2c -c public IP_DISPOSITIVO .1.3.6.1.2.1.1` |
| Verificar puerto SNMP | `nc -uzv IP_DISPOSITIVO 161` |

### Gestión de logs

| Acción | Comando |
|--------|---------|
| Ver log de instalación | `cat /opt/ness_relay/logs/install.log` |
| Ver log de operación | `cat /opt/ness_relay/logs/ness_relay.log` |
| Limpiar log de operación | `sudo truncate -s 0 /opt/ness_relay/logs/ness_relay.log` |
| Ver tamaño de los logs | `du -sh /opt/ness_relay/logs/*` |

---

## 18. Solución de problemas comunes

### Error: "No se encuentra el archivo 'ness-relay-ubuntu'"

**Causa:** El ejecutable no está en el mismo directorio que el script de instalación.

**Solución:**
```bash
# Verificar archivos presentes en el directorio actual
ls -lh

# Asegúrese de que ambos archivos estén juntos antes de ejecutar
ls install_relay.sh ness-relay-ubuntu
```

---

### Error: "Este script debe ejecutarse como root"

**Causa:** El instalador se ejecutó sin `sudo`.

**Solución:**
```bash
sudo ./install_relay.sh
```

---

### Error al conectar SNMP: "Timeout" o "No response"

**Causas posibles y soluciones:**

| Causa | Verificación | Solución |
|-------|-------------|---------|
| IP incorrecta | `ping IP_DISPOSITIVO` | Corregir la IP en `devices.conf` y reinstalar |
| Puerto bloqueado por firewall | `nc -uzv IP 161` | Abrir el puerto 161 UDP en el firewall del dispositivo |
| Community string incorrecto | — | Verificar el community en el dispositivo y reinstalar |
| SNMP desactivado en el dispositivo | — | Activar SNMP en la configuración del dispositivo |
| SNMP restringido por IP | — | Agregar la IP del servidor relay a la lista de hosts permitidos |

---

### Error SNMPv3: "Authentication failure" o "Wrong digest"

**Causa:** Las credenciales SNMPv3 no coinciden con las configuradas en el dispositivo.

**Verificaciones:**
1. El usuario SNMPv3 existe en el dispositivo
2. El protocolo de autenticación coincide (SHA/MD5)
3. La contraseña de autenticación es correcta (mín. 8 caracteres)
4. El protocolo de privacidad coincide (AES128/AES256/DES)
5. La contraseña de privacidad es correcta

```bash
# Probar SNMPv3 manualmente
snmpwalk -v3 -l authPriv -u USUARIO -a SHA -A PASS_AUTH -x AES -X PASS_PRIV IP_DISPOSITIVO .1.3
```

---

### El agente se ejecuta pero no envía datos al servidor

**Verificaciones:**
```bash
# 1. Verificar conectividad al servidor NESS HQ
curl -I https://cloud.nesshq.com

# 2. Verificar que el token es válido en los logs
grep -i "token\|auth\|401\|403" /opt/ness_relay/logs/ness_relay.log | tail -n 20

# 3. Verificar el servidor configurado
cat /etc/profile.d/ness_relay.sh
```

---

### El cron no está ejecutando el agente

**Verificaciones:**
```bash
# 1. Verificar que el cron está configurado
crontab -l | grep ness_relay

# 2. Verificar que el servicio cron está activo
systemctl status cron     # Ubuntu/Debian
systemctl status crond    # CentOS/RHEL

# 3. Revisar logs del cron
grep "ness_relay" /var/log/syslog | tail -n 20
```

---

### Reinstalar o actualizar la configuración

Si necesita agregar nuevos dispositivos o cambiar la configuración existente, ejecute nuevamente el instalador:

```bash
sudo ./install_relay.sh
```

El instalador detectará la instalación existente y ofrecerá las opciones:

```
⚠️  INSTALACIÓN EXISTENTE DETECTADA

Selecciona una opción:
  1) Reinstalar completamente (elimina todo y crea una instalación nueva)
  2) Actualizar configuración (mantiene estructura, actualiza configuraciones)
  3) Cancelar instalación
```

- **Opción 1:** Reinstalación completa. Se elimina toda la configuración anterior.
- **Opción 2:** Solo se actualiza la configuración. El ejecutable y los logs se conservan.
- **Opción 3:** No realiza cambios.

---

## Información de soporte

| Canal | Dirección |
|-------|-----------|
| **Web** | https://nesshq.com |
| **Soporte técnico** | https://soporte.nesshq.com |

---

> © 2026 NETWORK IS COLOMBIA S.A.S — Todos los derechos reservados.  
> Este documento es de uso interno. No distribuir sin autorización.
