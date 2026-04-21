# NESS Relay — Roadmap Multi-Vendor Enterprise

> **Proyecto:** Transformación del agente NESS Relay de un monitor pfSense-only a una plataforma Multi-Vendor Enterprise  
> **Fecha de inicio:** 20 de febrero de 2026  
> **Versión objetivo:** v2.0.0  
> **Autor:** NESS HQ Development Team – Network Is Colombia S.A.S  

---

## Tabla de Contenido

1. [Resumen Ejecutivo](#1-resumen-ejecutivo)
2. [Análisis del Estado Actual (v1.0.4)](#2-análisis-del-estado-actual-v104)
3. [Regla de Oro: OIDs Estándar vs Vendor-Specific](#3-regla-de-oro-oids-estándar-vs-vendor-specific)
4. [Arquitectura Objetivo](#4-arquitectura-objetivo)
5. [Fases de Desarrollo](#5-fases-de-desarrollo)
   - [Fase 1 – Diseño del Sistema de Perfiles de Dispositivo](#fase-1--diseño-del-sistema-de-perfiles-de-dispositivo)
   - [Fase 2 – Extracción y Organización de OIDs](#fase-2--extracción-y-organización-de-oids)
   - [Fase 3 – Motor de Recolección Multi-Vendor](#fase-3--motor-de-recolección-multi-vendor)
   - [Fase 4 – Perfiles Vendor: pfSense, Fortinet, Cisco, MikroTik](#fase-4--perfiles-vendor-pfsense-fortinet-cisco-mikrotik)
   - [Fase 5 – Actualización del Sistema de Build e Instalador](#fase-5--actualización-del-sistema-de-build-e-instalador)
   - [Fase 6 – Testing, Validación y QA](#fase-6--testing-validación-y-qa)
   - [Fase 7 – Documentación y Release](#fase-7--documentación-y-release)
6. [Estructura de Archivos Objetivo](#6-estructura-de-archivos-objetivo)
7. [Mapa de OIDs: Estándar vs Vendor-Specific](#7-mapa-de-oids-estándar-vs-vendor-specific)
8. [Diseño del Device Profile (Especificación)](#8-diseño-del-device-profile-especificación)
9. [Compatibilidad hacia Atrás](#9-compatibilidad-hacia-atrás)
10. [Consideraciones para Futuras Expansiones](#10-consideraciones-para-futuras-expansiones)
11. [Registro de Progreso](#11-registro-de-progreso)

---

## 1. Resumen Ejecutivo

El NESS Relay v1.0.4 es un agente SNMP diseñado exclusivamente para monitorear firewalls pfSense. Toda su lógica de recolección, los OIDs y las funciones de análisis están acopladas en un único archivo Python (`ness_relay_v1.0.4.py`) de ~2,285 líneas.

**El objetivo de esta transformación es:**

- Convertir el agente en una plataforma **Multi-Vendor** capaz de monitorear dispositivos de **pfSense, Fortinet, Cisco y MikroTik**.
- Implementar un **Sistema de Perfiles de Dispositivo** escalable que permita agregar nuevos fabricantes y tipos de dispositivo (firewalls, switches, access points) sin modificar el core del agente.
- Aplicar el principio de **Regla de Oro**: ~70% de los OIDs son estándar (RFC) y compartidos entre todos los fabricantes; solo ~30% son vendor-specific.
- Mantener la compatibilidad con el sistema de build (PyInstaller) y el instalador existente.

---

## 2. Análisis del Estado Actual (v1.0.4)

### 2.1 Arquitectura Actual

```
ness_relay_v1.0.4.py (ARCHIVO ÚNICO - 2,285 líneas)
├── Configuración y constantes
├── Diccionario OIDS (78 OIDs hardcodeados)
├── Logging
├── Sistema de actualización automática
├── Funciones SNMP (get, bulk)
├── Utilidades de conversión
├── Recolección por categorías:
│   ├── collect_system_data()         → OIDs estándar (RFC 1213)
│   ├── collect_performance_data()    → OIDs UCD-SNMP-MIB + estándar
│   ├── collect_network_data()        → OIDs IF-MIB (estándar)
│   ├── collect_security_data()       → OIDs estándar (TCP/UDP/IP/ICMP/SNMP)
│   └── collect_pfsense_specific_data() → OIDs pfSense (PF-MIB)
├── Análisis y alertas
├── Exportación y envío a servidor
└── Flujo principal (main)
```

### 2.2 Problemas Identificados

| # | Problema | Impacto |
|---|---------|---------|
| 1 | **OIDs hardcodeados** en el archivo principal | Imposible agregar vendors sin modificar el core |
| 2 | **Función `collect_pfsense_specific_data()`** acoplada | Cada vendor nuevo requeriría una función dedicada en el mismo archivo |
| 3 | **Sin sistema de perfiles** | No hay manera de definir qué OIDs consultar según el fabricante |
| 4 | **Diccionario OIDS plano** | No distingue entre OIDs estándar y vendor-specific |
| 5 | **`collect_all_data()` secuencial y fijo** | Los 8 pasos están hardcodeados, no se adaptan al tipo de dispositivo |
| 6 | **Metadata sin vendor info** | El JSON enviado no identifica el fabricante/tipo de dispositivo |
| 7 | **Sin detección automática de vendor** | No hay sysObjectID lookup para auto-identificar dispositivos |

### 2.3 Fortalezas a Preservar

- ✅ Motor SNMP asíncrono robusto (`snmp_get`, `snmp_bulk`)
- ✅ Sistema de configuración multi-dispositivo via `devices.conf`
- ✅ Sistema de actualización automática
- ✅ Normalización de datos (KB→GB, cálculo de porcentajes)
- ✅ Análisis de alertas de seguridad y rendimiento
- ✅ Logging con rotación
- ✅ Compatibilidad con PyInstaller (frozen executables)
- ✅ Soporte SNMPv1/v2c/v3

---

## 3. Regla de Oro: OIDs Estándar vs Vendor-Specific

> **Regla de Oro:** Antes de buscar OIDs específicos para cada fabricante, debemos reconocer que **~70% de los datos son idénticos entre todas las marcas** porque usan MIBs estándar del IETF.

### 3.1 Clasificación de los OIDs Actuales

De los 78 OIDs actuales en `ness_relay_v1.0.4.py`:

| Categoría | Cantidad | MIB Estándar | Compartidos entre vendors |
|-----------|----------|-------------|---------------------------|
| Sistema Básico | 6 | RFC 1213 (SNMPv2-MIB) | ✅ **100%** — Todos los dispositivos SNMP |
| CPU | 8 | UCD-SNMP-MIB | ⚠️ ~60% — pfSense/Linux sí, Cisco/Fortinet/MikroTik usan OIDs propios |
| Memoria | 8 | UCD-SNMP-MIB | ⚠️ ~60% — Similar a CPU |
| Disco | 7 | UCD-SNMP-MIB | ⚠️ ~50% — Varía por plataforma |
| Interfaces | 17 | IF-MIB (RFC 2863) | ✅ **100%** — Estándar universal |
| Interfaces HC | 3 | IF-MIB (RFC 2863) | ✅ **100%** — Estándar universal |
| TCP | 10 | RFC 4022 (TCP-MIB) | ✅ **100%** — Estándar universal |
| UDP | 4 | RFC 4113 (UDP-MIB) | ✅ **100%** — Estándar universal |
| IP | 14 | RFC 4293 (IP-MIB) | ✅ **100%** — Estándar universal |
| ICMP | 11 | RFC 2011 (ICMP-MIB) | ✅ **100%** — Estándar universal |
| SNMP Stats | 11 | RFC 3418 (SNMPv2-MIB) | ✅ **100%** — Estándar universal |
| pfSense | 8 | PF-MIB (privado) | ❌ **Solo pfSense** |

**Resultado:** De 78 OIDs → **70 son estándar/compartidos** (89.7%) y **8 son pfSense-specific** (10.3%).

### 3.2 Implicación Arquitectónica

```
┌─────────────────────────────────────────────────────────┐
│                    PERFILES DE OIDs                       │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌──────────────────────────────────────┐                │
│  │     OIDs ESTÁNDAR (Base Común)       │  ← Todos      │
│  │  RFC 1213, IF-MIB, TCP/UDP/IP/ICMP   │    los vendors │
│  │  ~70 OIDs compartidos                │                │
│  └──────────────────────────────────────┘                │
│                                                           │
│  ┌────────────┐ ┌────────────┐ ┌───────────┐ ┌────────┐ │
│  │  pfSense   │ │  Fortinet  │ │   Cisco   │ │MikroTik│ │
│  │ PF-MIB     │ │ FORTINET-  │ │ CISCO-    │ │MIKROTIK│ │
│  │ 8 OIDs     │ │ FORTIGATE  │ │ PROCESS   │ │-MIB    │ │
│  │            │ │ -MIB       │ │ -MIB      │ │        │ │
│  │            │ │ ~15 OIDs   │ │ ~15 OIDs  │ │~12 OIDs│ │
│  └────────────┘ └────────────┘ └───────────┘ └────────┘ │
│                                                           │
│  Nota: CPU y Memoria usan OIDs diferentes por vendor     │
│  porque UCD-SNMP-MIB no es universal en todos            │
└─────────────────────────────────────────────────────────┘
```

---

## 4. Arquitectura Objetivo

### 4.1 Diagrama de Componentes

```
ness_relay_v2.0/
│
├── ness_relay.py                  ← Entry point (simplificado)
│
├── core/
│   ├── __init__.py
│   ├── engine.py                  ← Motor de recolección multi-vendor
│   ├── snmp_client.py             ← Funciones SNMP (get, bulk) - extraídas del v1.0.4
│   ├── config.py                  ← Gestión de configuración y constantes
│   ├── logging_setup.py           ← Logging con rotación
│   └── updater.py                 ← Sistema de actualización automática
│
├── profiles/
│   ├── __init__.py
│   ├── base_profile.py            ← Clase abstracta BaseDeviceProfile
│   ├── profile_loader.py          ← Carga y registra perfiles dinámicamente
│   ├── standard_oids.py           ← OIDs estándar compartidos (RFC)
│   │
│   ├── vendors/
│   │   ├── __init__.py
│   │   ├── pfsense.py             ← Perfil pfSense (firewall)
│   │   ├── fortinet.py            ← Perfil Fortinet FortiGate (firewall)
│   │   ├── cisco.py               ← Perfil Cisco (router/switch/firewall)
│   │   └── mikrotik.py            ← Perfil MikroTik RouterOS
│   │
│   └── device_types/
│       ├── __init__.py
│       ├── firewall.py            ← Tipo: Firewall (funciones comunes de FW)
│       ├── router.py              ← Tipo: Router (futuro)
│       ├── switch.py              ← Tipo: Switch (futuro)
│       └── access_point.py        ← Tipo: Access Point (futuro)
│
├── collectors/
│   ├── __init__.py
│   ├── system_collector.py        ← Recolección de datos del sistema (estándar)
│   ├── performance_collector.py   ← CPU, Memoria, Disco
│   ├── network_collector.py       ← Interfaces de red
│   ├── security_collector.py      ← TCP/UDP/IP/ICMP/SNMP stats
│   └── vendor_collector.py        ← Recolección de datos vendor-specific
│
├── analyzers/
│   ├── __init__.py
│   ├── security_analyzer.py       ← Análisis de amenazas
│   └── performance_analyzer.py    ← Análisis de rendimiento
│
├── exporters/
│   ├── __init__.py
│   ├── json_exporter.py           ← Exportación a JSON
│   └── server_sender.py           ← Envío a servidor NESS
│
└── utils/
    ├── __init__.py
    ├── conversions.py             ← kb_to_gb, safe_int, safe_float, etc.
    ├── helpers.py                 ← Funciones auxiliares
    └── crypto_init.py             ← Inicialización de crypto para PyInstaller
```

### 4.2 Flujo de Ejecución Multi-Vendor

```
                        ┌─────────────────┐
                        │  ness_relay.py   │
                        │  (entry point)   │
                        └────────┬────────┘
                                 │
                    ┌────────────▼────────────┐
                    │   Cargar devices.conf    │
                    │   (lista de dispositivos)│
                    └────────────┬────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │  Para cada dispositivo:  │
                    │  1. Leer vendor field     │
                    │  2. Cargar perfil vendor  │
                    └────────────┬────────────┘
                                 │
               ┌─────────────────┼─────────────────┐
               │                 │                   │
        ┌──────▼──────┐  ┌──────▼──────┐  ┌────────▼────────┐
        │ Recolectar   │  │ Recolectar   │  │  Recolectar      │
        │ OIDs         │  │ OIDs         │  │  OIDs            │
        │ ESTÁNDAR     │  │ VENDOR-      │  │  DEVICE TYPE     │
        │ (base común) │  │ SPECIFIC     │  │  (firewall, etc) │
        └──────┬──────┘  └──────┬──────┘  └────────┬────────┘
               │                 │                   │
               └─────────────────┼─────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │  Unificar datos en      │
                    │  estructura JSON estándar│
                    └────────────┬────────────┘
                                 │
               ┌─────────────────┼─────────────────┐
               │                                     │
        ┌──────▼──────┐                      ┌──────▼──────┐
        │ Análisis de  │                      │ Exportar     │
        │ seguridad y  │                      │ JSON + Enviar│
        │ rendimiento  │                      │ a servidor   │
        └─────────────┘                      └─────────────┘
```

### 4.3 Patrón de Diseño: Device Profile

```python
# Concepto del BaseDeviceProfile (pseudocódigo ilustrativo)

class BaseDeviceProfile:
    """Clase base para todos los perfiles de dispositivo."""
    
    vendor: str           # "pfsense", "fortinet", "cisco", "mikrotik"
    vendor_display: str   # "pfSense", "Fortinet FortiGate", etc.
    device_type: str      # "firewall", "router", "switch", "access_point"
    version: str          # Versión del perfil
    
    # sysObjectID patterns para auto-detección
    sys_object_id_patterns: List[str]
    
    # OIDs vendor-specific (se suman a los estándar)
    vendor_oids: Dict[str, str]
    
    # Mapeo de CPU OIDs (porque varían entre vendors)
    cpu_oids: Dict[str, str]
    
    # Mapeo de Memory OIDs (porque varían entre vendors)
    memory_oids: Dict[str, str]
    
    # Métodos de recolección vendor-specific
    async def collect_vendor_data(snmp_client) -> Dict
    
    # Método para interpretar datos crudos del vendor
    def normalize_vendor_data(raw_data) -> Dict
    
    # Umbrales de alerta específicos del vendor
    def get_alert_thresholds() -> Dict
```

---

## 5. Fases de Desarrollo

---

### Fase 1 — Diseño del Sistema de Perfiles de Dispositivo
**Estado:** ⬜ No iniciada  
**Prioridad:** 🔴 Crítica  
**Estimación:** 2-3 días  

#### Objetivo
Definir e implementar la estructura base del sistema de perfiles que permitirá registrar y cargar configuraciones de OIDs por fabricante de manera modular.

#### Tareas

| # | Tarea | Estado |
|---|-------|--------|
| 1.1 | Crear la estructura de directorios del proyecto (`core/`, `profiles/`, `collectors/`, etc.) | ⬜ |
| 1.2 | Implementar `BaseDeviceProfile` (clase abstracta con interfaz estándar) | ⬜ |
| 1.3 | Implementar `ProfileLoader` (registry pattern para cargar perfiles por vendor name) | ⬜ |
| 1.4 | Implementar `DeviceType` base classes (Firewall, Router, Switch, AccessPoint) | ⬜ |
| 1.5 | Definir el contrato de datos: estructura JSON estándar que todo perfil debe producir | ⬜ |
| 1.6 | Diseñar sistema de auto-detección de vendor via `sysObjectID` (OID 1.3.6.1.2.1.1.2.0) | ⬜ |

#### Decisiones de Diseño

- **¿Por qué clases Python y no archivos JSON/YAML para los perfiles?**
  - Los perfiles necesitan lógica de normalización específica por vendor (ej: Fortinet reporta CPU diferente a pfSense).
  - Los archivos JSON/YAML solo pueden definir OIDs pero no cómo interpretar las respuestas.
  - Con PyInstaller, los archivos Python se compilan en el binario; archivos externos requerirían `--add-data` y acceso a filesystem.
  - **Decisión:** Perfiles como clases Python con herencia + diccionarios de OIDs como atributos de clase.

- **¿Cómo se selecciona el perfil para cada dispositivo?**
  1. **Explícito (prioritario):** El campo `vendor` en `devices.conf` indica directamente el perfil.
  2. **Auto-detección (fallback):** Si `vendor=auto`, el sistema consulta `sysObjectID` y busca en el registry de perfiles.

#### Entregables
- [ ] `profiles/base_profile.py` — Clase abstracta
- [ ] `profiles/profile_loader.py` — Registry de perfiles
- [ ] `profiles/device_types/firewall.py` — Tipo base Firewall
- [ ] Especificación del contrato de datos JSON

---

### Fase 2 — Extracción y Organización de OIDs
**Estado:** ⬜ No iniciada  
**Prioridad:** 🔴 Crítica  
**Estimación:** 1-2 días  

#### Objetivo
Extraer los OIDs del archivo monolítico actual y organizarlos en dos capas: OIDs estándar (base común) y OIDs vendor-specific.

#### Tareas

| # | Tarea | Estado |
|---|-------|--------|
| 2.1 | Crear `profiles/standard_oids.py` con todos los OIDs estándar RFC organizados por categoría | ⬜ |
| 2.2 | Clasificar los 78 OIDs actuales: separar estándar de pfSense-specific | ⬜ |
| 2.3 | Documentar cada OID con su MIB de origen y RFC | ⬜ |
| 2.4 | Definir estructura de diccionario por categoría (system, cpu, memory, disk, interfaces, security) | ⬜ |
| 2.5 | Implementar función `merge_oids()` que combine base + vendor OIDs | ⬜ |

#### Estructura de `standard_oids.py`

```python
# Concepto de organización (cada sección referencia su RFC)

STANDARD_OIDS = {
    'system': {
        # RFC 1213 - SNMPv2-MIB — Universal en TODOS los dispositivos SNMP
        'sys_descr':    '1.3.6.1.2.1.1.1.0',
        'sys_object_id': '1.3.6.1.2.1.1.2.0',  # ← NUEVO: para auto-detección
        'sys_uptime':   '1.3.6.1.2.1.1.3.0',
        'sys_contact':  '1.3.6.1.2.1.1.4.0',
        'sys_name':     '1.3.6.1.2.1.1.5.0',
        'sys_location': '1.3.6.1.2.1.1.6.0',
        'sys_services': '1.3.6.1.2.1.1.7.0',
    },
    'interfaces': {
        # RFC 2863 - IF-MIB — Universal
        'if_descr': '...', 'if_type': '...', # etc.
    },
    'interfaces_hc': {
        # RFC 2863 - IF-MIB (64-bit counters) — Universal
    },
    'tcp': {
        # RFC 4022 - TCP-MIB — Universal
    },
    'udp': {
        # RFC 4113 - UDP-MIB — Universal
    },
    'ip': {
        # RFC 4293 - IP-MIB — Universal
    },
    'icmp': {
        # RFC 2011 - ICMP — Universal
    },
    'snmp_stats': {
        # RFC 3418 - SNMPv2-MIB — Universal
    },
}
```

#### Entregables
- [ ] `profiles/standard_oids.py` — OIDs estándar organizados con documentación RFC
- [ ] Documento de clasificación de OIDs (en este mismo archivo, sección 7)

---

### Fase 3 — Motor de Recolección Multi-Vendor
**Estado:** ⬜ No iniciada  
**Prioridad:** 🔴 Crítica  
**Estimación:** 3-4 días  

#### Objetivo
Refactorizar el motor de recolección para que sea agnóstico al vendor (vendor-agnostic). El engine debe recibir un perfil y ejecutar la recolección según lo que el perfil defina.

#### Tareas

| # | Tarea | Estado |
|---|-------|--------|
| 3.1 | Extraer `snmp_get()` y `snmp_bulk()` a `core/snmp_client.py` | ⬜ |
| 3.2 | Extraer utilidades de conversión a `utils/conversions.py` | ⬜ |
| 3.3 | Crear `core/engine.py` — Motor de recolección que orquesta todo el flujo | ⬜ |
| 3.4 | Refactorizar `collect_system_data()` → `collectors/system_collector.py` (usa OIDs estándar) | ⬜ |
| 3.5 | Refactorizar `collect_performance_data()` → `collectors/performance_collector.py` (vendor-aware para CPU/Mem) | ⬜ |
| 3.6 | Refactorizar `collect_network_data()` → `collectors/network_collector.py` (usa OIDs estándar) | ⬜ |
| 3.7 | Refactorizar `collect_security_data()` → `collectors/security_collector.py` (usa OIDs estándar) | ⬜ |
| 3.8 | Crear `collectors/vendor_collector.py` — Delega al perfil del vendor | ⬜ |
| 3.9 | Parametrizar `collect_all_data()` para recibir perfil como argumento | ⬜ |
| 3.10 | Extraer `analyze_security_threats()` → `analyzers/security_analyzer.py` | ⬜ |
| 3.11 | Extraer `analyze_performance_metrics()` → `analyzers/performance_analyzer.py` | ⬜ |
| 3.12 | Extraer exportación → `exporters/json_exporter.py` y `exporters/server_sender.py` | ⬜ |
| 3.13 | Extraer inicialización crypto → `utils/crypto_init.py` | ⬜ |
| 3.14 | Extraer sistema de actualización → `core/updater.py` | ⬜ |
| 3.15 | Crear logging setup → `core/logging_setup.py` | ⬜ |
| 3.16 | Crear config management → `core/config.py` | ⬜ |

#### Principio Clave del Engine

```
El Engine NO conoce los OIDs directamente.
El Engine le pide al Profile: "Dame tus OIDs de CPU" → El Profile responde con su diccionario.
El Engine usa el SNMP Client para consultar esos OIDs.
El Engine le pide al Profile: "Normaliza estos datos" → El Profile los interpreta.
```

#### Entregables
- [ ] `core/engine.py` — Motor principal
- [ ] `core/snmp_client.py` — Cliente SNMP extraído
- [ ] `collectors/` — 5 collectors refactorizados
- [ ] `analyzers/` — 2 analyzers extraídos
- [ ] `exporters/` — Exporter y sender extraídos
- [ ] `utils/` — Utilidades extraídas

---

### Fase 4 — Perfiles Vendor: pfSense, Fortinet, Cisco, MikroTik
**Estado:** ⬜ No iniciada  
**Prioridad:** 🟡 Alta  
**Estimación:** 4-6 días  

#### Objetivo
Implementar los perfiles completos para cada fabricante, incluyendo OIDs específicos, lógica de normalización y umbrales de alerta.

#### Sub-fase 4A — Perfil pfSense (Migración del código existente)

| # | Tarea | Estado |
|---|-------|--------|
| 4A.1 | Crear `profiles/vendors/pfsense.py` extrayendo los 8 OIDs PF-MIB del v1.0.4 | ⬜ |
| 4A.2 | Migrar `collect_pfsense_specific_data()` como método del perfil | ⬜ |
| 4A.3 | Migrar lógica de CPU UCD-SNMP-MIB (funciona en pfSense porque usa net-snmp) | ⬜ |
| 4A.4 | Migrar lógica de memoria UCD-SNMP-MIB con corrección FreeBSD `mem_avail_real` | ⬜ |
| 4A.5 | Validar que el JSON de salida sea idéntico al v1.0.4 (backward compatible) | ⬜ |

**OIDs vendor-specific de pfSense (PF-MIB — OID base: 1.3.6.1.4.1.12325):**
- `pf_states_current`, `pf_states_searches`, `pf_states_inserts`, `pf_states_removals`
- `pf_log_entries`, `pf_log_bytes`, `pf_block_packets`, `pf_block_bytes`

#### Sub-fase 4B — Perfil Fortinet FortiGate

| # | Tarea | Estado |
|---|-------|--------|
| 4B.1 | Investigar y documentar OIDs FORTINET-FORTIGATE-MIB | ✅ |
| 4B.2 | Crear `profiles/vendors/fortinet.py` | ✅ |
| 4B.3 | Implementar OIDs de CPU FortiGate (enterprise OID base: 1.3.6.1.4.1.12356) | ✅ |
| 4B.4 | Implementar OIDs de memoria FortiGate | ✅ |
| 4B.5 | Implementar OIDs de sesiones/firewall FortiGate | ✅ |
| 4B.6 | Implementar OIDs de VPN FortiGate (si aplica) | ✅ |
| 4B.7 | Implementar normalización de datos Fortinet → formato estándar NESS | ✅ |
| 4B.8 | Definir umbrales de alerta para Fortinet | ⬜ |

**OIDs vendor-specific principales de Fortinet FortiGate:**

| Categoría | OID | Descripción |
|-----------|-----|-------------|
| CPU | 1.3.6.1.4.1.12356.101.4.1.3.0 | fgSysCpuUsage (porcentaje) |
| Memoria | 1.3.6.1.4.1.12356.101.4.1.4.0 | fgSysMemUsage (porcentaje) |
| Memoria | 1.3.6.1.4.1.12356.101.4.1.5.0 | fgSysMemCapacity (KB) |
| Disco | 1.3.6.1.4.1.12356.101.4.1.6.0 | fgSysDiskUsage (MB) |
| Disco | 1.3.6.1.4.1.12356.101.4.1.7.0 | fgSysDiskCapacity (MB) |
| Sesiones | 1.3.6.1.4.1.12356.101.4.1.8.0 | fgSysSesCount |
| Sesiones | 1.3.6.1.4.1.12356.101.4.1.11.0 | fgSysSes6Count (IPv6) |
| Firmware | 1.3.6.1.4.1.12356.101.4.1.1.0 | fgSysVersion |
| Serial | 1.3.6.1.4.1.12356.100.1.1.1.0 | fnSysSerial |
| HA | 1.3.6.1.4.1.12356.101.13.1.1.0 | fgHaSystemMode |
| VPN | 1.3.6.1.4.1.12356.101.12.2.2.1.* | VPN tunnel stats (tabla) |
| Antivirus | 1.3.6.1.4.1.12356.101.8.2.1.1.0 | fgAvVirusDetected |
| IPS | 1.3.6.1.4.1.12356.101.9.2.1.1.0 | fgIpsIntrusionsDetected |

#### Sub-fase 4C — Perfil Cisco

| # | Tarea | Estado |
|---|-------|--------|
| 4C.1 | Investigar y documentar OIDs CISCO-PROCESS-MIB, CISCO-MEMORY-POOL-MIB, CISCO-ENVMON-MIB | ⬜ |
| 4C.2 | Crear `profiles/vendors/cisco.py` | ⬜ |
| 4C.3 | Implementar OIDs de CPU Cisco (CISCO-PROCESS-MIB) | ⬜ |
| 4C.4 | Implementar OIDs de memoria Cisco (CISCO-MEMORY-POOL-MIB) | ⬜ |
| 4C.5 | Implementar OIDs de temperatura/estado Cisco (CISCO-ENVMON-MIB) | ⬜ |
| 4C.6 | Implementar OIDs de firewall Cisco ASA (si aplica) | ⬜ |
| 4C.7 | Implementar normalización de datos Cisco → formato estándar NESS | ⬜ |
| 4C.8 | Definir umbrales de alerta para Cisco | ⬜ |

**OIDs vendor-specific principales de Cisco (enterprise OID base: 1.3.6.1.4.1.9):**

| Categoría | OID | Descripción |
|-----------|-----|-------------|
| CPU 5s | 1.3.6.1.4.1.9.9.109.1.1.1.1.6.* | cpmCPUTotal5sec |
| CPU 1min | 1.3.6.1.4.1.9.9.109.1.1.1.1.7.* | cpmCPUTotal1min |
| CPU 5min | 1.3.6.1.4.1.9.9.109.1.1.1.1.8.* | cpmCPUTotal5min |
| Mem Used | 1.3.6.1.4.1.9.9.48.1.1.1.5.* | ciscoMemoryPoolUsed |
| Mem Free | 1.3.6.1.4.1.9.9.48.1.1.1.6.* | ciscoMemoryPoolFree |
| Mem Name | 1.3.6.1.4.1.9.9.48.1.1.1.2.* | ciscoMemoryPoolName |
| Temp | 1.3.6.1.4.1.9.9.13.1.3.1.3.* | ciscoEnvMonTemperatureValue |
| Temp Status | 1.3.6.1.4.1.9.9.13.1.3.1.6.* | ciscoEnvMonTemperatureState |
| Fan Status | 1.3.6.1.4.1.9.9.13.1.4.1.3.* | ciscoEnvMonFanState |
| PSU Status | 1.3.6.1.4.1.9.9.13.1.5.1.3.* | ciscoEnvMonSupplyState |
| Firmware | 1.3.6.1.4.1.9.9.25.1.1.1.2.* | ciscoImageString |

#### Sub-fase 4D — Perfil MikroTik RouterOS

| # | Tarea | Estado |
|---|-------|--------|
| 4D.1 | Investigar y documentar OIDs MIKROTIK-MIB | ✅ |
| 4D.2 | Crear `profiles/vendors/mikrotik.py` | ✅ |
| 4D.3 | Implementar OIDs de CPU MikroTik | ✅ |
| 4D.4 | Implementar OIDs de memoria MikroTik | ✅ |
| 4D.5 | Implementar OIDs de disco MikroTik | ✅ |
| 4D.6 | Implementar OIDs de licencia y firmware MikroTik | ✅ |
| 4D.7 | Implementar OIDs de wireless MikroTik (para futuros AP) | ✅ |
| 4D.8 | Implementar normalización MikroTik → formato estándar NESS | ✅ |
| 4D.9 | Definir umbrales de alerta para MikroTik | ⬜ |

**OIDs vendor-specific principales de MikroTik (enterprise OID base: 1.3.6.1.4.1.14988):**

| Categoría | OID | Descripción |
|-----------|-----|-------------|
| CPU Load | 1.3.6.1.2.1.25.3.3.1.2.* | hrProcessorLoad (HOST-RESOURCES-MIB, MikroTik lo implementa) |
| Total Mem | 1.3.6.1.2.1.25.2.3.1.5.* | hrStorageSize (HOST-RESOURCES-MIB) |
| Used Mem | 1.3.6.1.2.1.25.2.3.1.6.* | hrStorageUsed (HOST-RESOURCES-MIB) |
| Firmware | 1.3.6.1.4.1.14988.1.1.4.4.0 | mtxrFirmwareVersion |
| License | 1.3.6.1.4.1.14988.1.1.4.3.0 | mtxrLicSoftwareId |
| Serial | 1.3.6.1.4.1.14988.1.1.7.3.0 | mtxrSerialNumber |
| Board Temp | 1.3.6.1.4.1.14988.1.1.3.10.0 | mtxrHlTemperature |
| Voltage | 1.3.6.1.4.1.14988.1.1.3.8.0 | mtxrHlVoltage |
| Active Users | 1.3.6.1.4.1.14988.1.1.5.1.0 | mtxrWirelessClientCount (para AP) |
| Disk Total | 1.3.6.1.4.1.14988.1.1.3.1.0 | mtxrHlDiskTotal (bytes) |
| Disk Used | 1.3.6.1.4.1.14988.1.1.3.2.0 | mtxrHlDiskUsed (bytes) |
| Queue Stats | 1.3.6.1.4.1.14988.1.1.2.* | Tabla de colas (QoS) |

**Nota sobre MikroTik:** MikroTik implementa HOST-RESOURCES-MIB (RFC 2790) para CPU y almacenamiento, lo cual es un estándar ampliamente soportado. Esto significa que muchos OIDs de MikroTik son técnicamente "estándar" pero bajo una MIB diferente a la UCD-SNMP-MIB usada por pfSense.

#### Entregables
- [x] `profiles/vendors/pfsense.py` — Perfil completo (migración)
- [x] `profiles/vendors/fortinet.py` — Perfil completo (nuevo)
- [ ] `profiles/vendors/cisco.py` — Perfil completo (nuevo)
- [x] `profiles/vendors/mikrotik.py` — Perfil completo (nuevo)
- [ ] `profiles/vendors/c_n.py` — Perfil Cambium Networks (stub creado)
- [ ] `profiles/vendors/ubnt.py` — Perfil Ubiquiti (stub creado)
- [ ] Tests de validación por vendor

---

### Fase 5 — Actualización del Sistema de Build e Instalador
**Estado:** ✅ Completada  
**Prioridad:** 🟡 Alta  
**Estimación:** 2-3 días  

#### Objetivo
Adaptar `build_relay_executable.sh` y `install_relay.sh` para que soporten la nueva estructura modular multi-vendor.

#### Tareas

| # | Tarea | Estado |
|---|-------|--------|
| 5.1 | Actualizar PyInstaller spec para incluir todos los módulos del paquete | ✅ |
| 5.2 | Agregar `--hidden-import` para cada módulo de perfil vendor | ✅ |
| 5.3 | Verificar que todos los archivos `.py` del paquete se incluyen en el binario | ✅ |
| 5.4 | Actualizar `install_relay.sh` para soportar los nuevos vendors (MikroTik ya no es solo listado) | ✅ |
| 5.5 | Agregar opción en el instalador para seleccionar vendor MikroTik | ✅ |
| 5.6 | Actualizar generación de `devices.conf` con campos adicionales (device_type, etc.) | ⬜ |
| 5.7 | Actualizar `RELAY_VERSION` a `2.0.0` | ✅ |
| 5.8 | Actualizar banner y mensajes del build script | ✅ |
| 5.9 | Test de build completo en Ubuntu | ⬜ |
| 5.10 | Test de instalación completa con múltiples vendors | ⬜ |

#### Consideraciones PyInstaller

El build actual ya maneja:
- Hidden imports para pysnmp, pycryptodome, pysnmpcrypto
- Hooks personalizados en `/hooks/`

Lo que se debe agregar:
- ~~Todas las rutas de imports de los nuevos módulos (`core.*`, `profiles.*`, `collectors.*`, etc.)~~ ✅ Agregado
- ~~El entry point cambia de `ness_relay_v1.0.4.py` a `ness_relay.py`~~ ✅ Actualizado

#### Entregables
- [x] `build_relay_executable.sh` actualizado
- [x] `install_relay.sh` actualizado
- [ ] Hooks de PyInstaller actualizados (si es necesario) — No requirieron cambios
- [ ] Test de build y despliegue exitoso — Pendiente Fase 6

---

### Fase 6 — Testing, Validación y QA
**Estado:** ⬜ No iniciada  
**Prioridad:** 🟡 Alta  
**Estimación:** 2-3 días  

#### Objetivo
Validar que la refactorización no rompe funcionalidad existente y que los nuevos vendors funcionan correctamente.

#### Tareas

| # | Tarea | Estado |
|---|-------|--------|
| 6.1 | Test de regresión: comparar JSON v1.0.4 vs v2.0 con dispositivo pfSense | ⬜ |
| 6.2 | Test de conectividad SNMP con dispositivo Fortinet real o simulado | ⬜ |
| 6.3 | Test de conectividad SNMP con dispositivo Cisco real o simulado | ⬜ |
| 6.4 | Test de conectividad SNMP con dispositivo MikroTik real o simulado | ⬜ |
| 6.5 | Test multi-dispositivo: ejecutar relay con devices.conf de múltiples vendors | ⬜ |
| 6.6 | Test de auto-detección de vendor via sysObjectID | ⬜ |
| 6.7 | Test del binario compilado con PyInstaller | ⬜ |
| 6.8 | Test de instalación y cron | ⬜ |
| 6.9 | Test de envío de datos al servidor NESS con nuevo formato | ⬜ |
| 6.10 | Validar compatibilidad con el backend API de NESS HQ | ⬜ |

#### Entregables
- [ ] Reporte de tests de regresión
- [ ] Reporte de tests por vendor
- [ ] Validación de compatibilidad de API

---

### Fase 7 — Documentación y Release
**Estado:** ⬜ No iniciada  
**Prioridad:** 🟢 Normal  
**Estimación:** 1-2 días  

#### Tareas

| # | Tarea | Estado |
|---|-------|--------|
| 7.1 | Actualizar README.md del relay | ⬜ |
| 7.2 | Documentar cómo agregar un nuevo vendor (guía de desarrollo) | ⬜ |
| 7.3 | Documentar estructura JSON de salida por vendor | ⬜ |
| 7.4 | Documentar OIDs por vendor (referencia técnica) | ⬜ |
| 7.5 | Crear changelog v1.0.4 → v2.0.0 | ⬜ |
| 7.6 | Release de binario v2.0.0 | ⬜ |

#### Entregables
- [ ] README.md actualizado
- [ ] Guía "Cómo agregar un nuevo vendor"
- [ ] Changelog v2.0.0
- [ ] Binario release

---

## 6. Estructura de Archivos Objetivo

```
agentes/relay/
│
├── ness_relay.py                          ← Entry point principal (simplificado)
│
├── core/
│   ├── __init__.py
│   ├── engine.py                          ← Motor de recolección multi-vendor
│   ├── snmp_client.py                     ← snmp_get(), snmp_bulk() extraídos
│   ├── config.py                          ← Constantes, paths, configuración
│   ├── logging_setup.py                   ← Setup de logging con rotación
│   └── updater.py                         ← Sistema de actualización automática
│
├── profiles/
│   ├── __init__.py
│   ├── base_profile.py                    ← ABC con interfaz estándar
│   ├── profile_loader.py                  ← Registry pattern - carga perfiles
│   ├── standard_oids.py                   ← OIDs estándar RFC (base común)
│   │
│   ├── vendors/
│   │   ├── __init__.py
│   │   ├── pfsense.py                     ← 8 OIDs PF-MIB + CPU/Mem UCD-SNMP
│   │   ├── fortinet.py                    ← OIDs FORTINET-FORTIGATE-MIB
│   │   ├── cisco.py                       ← OIDs CISCO-MIBs
│   │   └── mikrotik.py                    ← OIDs MIKROTIK-MIB + HOST-RESOURCES
│   │
│   └── device_types/
│       ├── __init__.py
│       ├── firewall.py                    ← Interfaz para tipo Firewall
│       ├── router.py                      ← Interfaz para tipo Router (futuro)
│       ├── switch.py                      ← Interfaz para tipo Switch (futuro)
│       └── access_point.py               ← Interfaz para tipo AP (futuro)
│
├── collectors/
│   ├── __init__.py
│   ├── system_collector.py                ← Datos del sistema (RFC 1213)
│   ├── performance_collector.py           ← CPU, Mem, Disco (vendor-aware)
│   ├── network_collector.py               ← Interfaces (IF-MIB estándar)
│   ├── security_collector.py              ← TCP/UDP/IP/ICMP/SNMP (estándar)
│   └── vendor_collector.py                ← Delega al perfil vendor
│
├── analyzers/
│   ├── __init__.py
│   ├── security_analyzer.py               ← analyze_security_threats()
│   └── performance_analyzer.py            ← analyze_performance_metrics()
│
├── exporters/
│   ├── __init__.py
│   ├── json_exporter.py                   ← export_to_json()
│   └── server_sender.py                   ← send_data_to_server()
│
├── utils/
│   ├── __init__.py
│   ├── conversions.py                     ← kb_to_gb, safe_int, safe_float, etc.
│   ├── helpers.py                         ← _now_iso, print_simple, format_uptime
│   └── crypto_init.py                     ← Crypto backend para PyInstaller
│
├── hooks/                                 ← PyInstaller hooks (ya existente)
│   ├── hook-Crypto.py
│   ├── hook-Cryptodome.py
│   ├── hook-pysnmp.py
│   └── hook-pysnmpcrypto.py
│
├── build_relay_executable.sh              ← Script de build (actualizado)
├── install_relay.sh                       ← Script de instalación (actualizado)
├── MULTI_VENDOR_ROADMAP.md               ← Este documento
└── ness_relay_v1.0.4.py                  ← Versión anterior (backup/referencia)
```

---

## 7. Mapa de OIDs: Estándar vs Vendor-Specific

### 7.1 OIDs Estándar (Base Común — Todos los Vendors)

| Categoría | OID Key | OID | MIB / RFC | Shared |
|-----------|---------|-----|-----------|--------|
| **System** | sys_descr | 1.3.6.1.2.1.1.1.0 | SNMPv2-MIB / RFC 1213 | ✅ ALL |
| | sys_object_id | 1.3.6.1.2.1.1.2.0 | SNMPv2-MIB / RFC 1213 | ✅ ALL |
| | sys_uptime | 1.3.6.1.2.1.1.3.0 | SNMPv2-MIB / RFC 1213 | ✅ ALL |
| | sys_contact | 1.3.6.1.2.1.1.4.0 | SNMPv2-MIB / RFC 1213 | ✅ ALL |
| | sys_name | 1.3.6.1.2.1.1.5.0 | SNMPv2-MIB / RFC 1213 | ✅ ALL |
| | sys_location | 1.3.6.1.2.1.1.6.0 | SNMPv2-MIB / RFC 1213 | ✅ ALL |
| | sys_services | 1.3.6.1.2.1.1.7.0 | SNMPv2-MIB / RFC 1213 | ✅ ALL |
| **Interfaces** | if_descr | 1.3.6.1.2.1.2.2.1.2 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_type | 1.3.6.1.2.1.2.2.1.3 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_mtu | 1.3.6.1.2.1.2.2.1.4 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_speed | 1.3.6.1.2.1.2.2.1.5 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_admin_status | 1.3.6.1.2.1.2.2.1.7 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_oper_status | 1.3.6.1.2.1.2.2.1.8 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_last_change | 1.3.6.1.2.1.2.2.1.9 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_in_octets | 1.3.6.1.2.1.2.2.1.10 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_in_ucast_pkts | 1.3.6.1.2.1.2.2.1.11 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_in_nucast_pkts | 1.3.6.1.2.1.2.2.1.12 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_in_discards | 1.3.6.1.2.1.2.2.1.13 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_in_errors | 1.3.6.1.2.1.2.2.1.14 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_in_unknown_protos | 1.3.6.1.2.1.2.2.1.15 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_out_octets | 1.3.6.1.2.1.2.2.1.16 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_out_ucast_pkts | 1.3.6.1.2.1.2.2.1.17 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_out_nucast_pkts | 1.3.6.1.2.1.2.2.1.18 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_out_discards | 1.3.6.1.2.1.2.2.1.19 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_out_errors | 1.3.6.1.2.1.2.2.1.20 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_hc_in_octets | 1.3.6.1.2.1.31.1.1.1.6 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_hc_out_octets | 1.3.6.1.2.1.31.1.1.1.10 | IF-MIB / RFC 2863 | ✅ ALL |
| | if_high_speed | 1.3.6.1.2.1.31.1.1.1.15 | IF-MIB / RFC 2863 | ✅ ALL |
| **TCP** | tcp_active_opens | 1.3.6.1.2.1.6.5.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_passive_opens | 1.3.6.1.2.1.6.6.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_attempt_fails | 1.3.6.1.2.1.6.7.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_estab_resets | 1.3.6.1.2.1.6.8.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_curr_estab | 1.3.6.1.2.1.6.9.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_in_segs | 1.3.6.1.2.1.6.10.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_out_segs | 1.3.6.1.2.1.6.11.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_retrans_segs | 1.3.6.1.2.1.6.12.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_in_errs | 1.3.6.1.2.1.6.14.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| | tcp_out_rsts | 1.3.6.1.2.1.6.15.0 | TCP-MIB / RFC 4022 | ✅ ALL |
| **UDP** | udp_in_datagrams | 1.3.6.1.2.1.7.1.0 | UDP-MIB / RFC 4113 | ✅ ALL |
| | udp_no_ports | 1.3.6.1.2.1.7.2.0 | UDP-MIB / RFC 4113 | ✅ ALL |
| | udp_in_errors | 1.3.6.1.2.1.7.3.0 | UDP-MIB / RFC 4113 | ✅ ALL |
| | udp_out_datagrams | 1.3.6.1.2.1.7.4.0 | UDP-MIB / RFC 4113 | ✅ ALL |
| **IP** | ip_forwarding | 1.3.6.1.2.1.4.1.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_in_receives | 1.3.6.1.2.1.4.3.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_in_hdr_errors | 1.3.6.1.2.1.4.4.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_in_addr_errors | 1.3.6.1.2.1.4.5.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_forw_datagrams | 1.3.6.1.2.1.4.6.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_in_unknown_protos | 1.3.6.1.2.1.4.7.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_in_discards | 1.3.6.1.2.1.4.8.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_in_delivers | 1.3.6.1.2.1.4.9.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_out_requests | 1.3.6.1.2.1.4.10.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_out_discards | 1.3.6.1.2.1.4.11.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_out_no_routes | 1.3.6.1.2.1.4.12.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_frag_oks | 1.3.6.1.2.1.4.17.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_frag_fails | 1.3.6.1.2.1.4.18.0 | IP-MIB / RFC 4293 | ✅ ALL |
| | ip_frag_creates | 1.3.6.1.2.1.4.19.0 | IP-MIB / RFC 4293 | ✅ ALL |
| **ICMP** | icmp_in_msgs | 1.3.6.1.2.1.5.1.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_in_errors | 1.3.6.1.2.1.5.2.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_in_dest_unreachs | 1.3.6.1.2.1.5.3.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_in_time_excds | 1.3.6.1.2.1.5.4.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_in_parm_probs | 1.3.6.1.2.1.5.5.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_in_src_quenchs | 1.3.6.1.2.1.5.6.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_in_redirects | 1.3.6.1.2.1.5.7.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_in_echos | 1.3.6.1.2.1.5.8.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_in_echo_reps | 1.3.6.1.2.1.5.9.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_out_msgs | 1.3.6.1.2.1.5.14.0 | ICMP / RFC 2011 | ✅ ALL |
| | icmp_out_errors | 1.3.6.1.2.1.5.15.0 | ICMP / RFC 2011 | ✅ ALL |
| **SNMP Stats** | snmp_in_pkts | 1.3.6.1.2.1.11.1.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_out_pkts | 1.3.6.1.2.1.11.2.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_bad_versions | 1.3.6.1.2.1.11.3.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_bad_community_names | 1.3.6.1.2.1.11.4.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_bad_community_uses | 1.3.6.1.2.1.11.5.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_asn_parse_errs | 1.3.6.1.2.1.11.6.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_too_bigs | 1.3.6.1.2.1.11.8.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_no_such_names | 1.3.6.1.2.1.11.9.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_bad_values | 1.3.6.1.2.1.11.10.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_read_onlys | 1.3.6.1.2.1.11.11.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |
| | snmp_in_gen_errs | 1.3.6.1.2.1.11.12.0 | SNMPv2-MIB / RFC 3418 | ✅ ALL |

### 7.2 OIDs que Varían por Vendor (CPU / Memoria / Disco)

| Vendor | CPU OIDs | Memoria OIDs | Disco OIDs |
|--------|----------|--------------|------------|
| **pfSense** | UCD-SNMP-MIB (1.3.6.1.4.1.2021.10/11) | UCD-SNMP-MIB (1.3.6.1.4.1.2021.4) | UCD-SNMP-MIB (1.3.6.1.4.1.2021.9) |
| **Fortinet** | FORTINET-MIB fgSysCpuUsage | FORTINET-MIB fgSysMemUsage/Capacity | FORTINET-MIB fgSysDiskUsage/Capacity |
| **Cisco** | CISCO-PROCESS-MIB cpmCPUTotal | CISCO-MEMORY-POOL-MIB | N/A (IOS no expone disco estándar) |
| **MikroTik** | HOST-RESOURCES-MIB hrProcessorLoad | HOST-RESOURCES-MIB hrStorageSize/Used | MIKROTIK-MIB mtxrHlDisk |

---

## 8. Diseño del Device Profile (Especificación)

### 8.1 Interfaz BaseDeviceProfile

```python
# Especificación conceptual - se implementará en Fase 1

from abc import ABC, abstractmethod
from typing import Dict, List, Any, Optional

class BaseDeviceProfile(ABC):
    """Clase base para todos los perfiles de dispositivo Multi-Vendor."""
    
    # === Identificación del vendor ===
    vendor_id: str          # "pfsense", "fortinet", "cisco", "mikrotik"
    vendor_name: str        # "pfSense", "Fortinet FortiGate", "Cisco IOS", "MikroTik RouterOS"
    device_type: str        # "firewall", "router", "switch", "access_point"
    profile_version: str    # "1.0.0"
    
    # === Para auto-detección via sysObjectID ===
    sys_object_id_patterns: List[str]  # ["1.3.6.1.4.1.12325.*"]
    
    # === OIDs vendor-specific ===
    @abstractmethod
    def get_cpu_oids(self) -> Dict[str, str]:
        """Retorna diccionario de OIDs de CPU para este vendor."""
        
    @abstractmethod
    def get_memory_oids(self) -> Dict[str, str]:
        """Retorna diccionario de OIDs de memoria para este vendor."""
    
    @abstractmethod
    def get_disk_oids(self) -> Dict[str, str]:
        """Retorna diccionario de OIDs de disco para este vendor."""
    
    @abstractmethod
    def get_vendor_specific_oids(self) -> Dict[str, str]:
        """Retorna OIDs exclusivos del vendor (ej: pf_states para pfSense)."""
    
    # === Normalización de datos ===
    @abstractmethod
    def normalize_cpu_data(self, raw: Dict) -> Dict[str, Any]:
        """Convierte datos crudos de CPU al formato estándar NESS."""
        
    @abstractmethod
    def normalize_memory_data(self, raw: Dict) -> Dict[str, Any]:
        """Convierte datos crudos de memoria al formato estándar NESS."""
    
    @abstractmethod
    def normalize_disk_data(self, raw: Dict) -> Dict[str, Any]:
        """Convierte datos crudos de disco al formato estándar NESS."""
    
    @abstractmethod
    def normalize_vendor_data(self, raw: Dict) -> Dict[str, Any]:
        """Convierte datos vendor-specific al formato estructurado."""
    
    # === Umbrales de alerta ===
    @abstractmethod
    def get_alert_thresholds(self) -> Dict[str, Any]:
        """Retorna umbrales de alerta específicos del vendor."""
```

### 8.2 Formato JSON Estándar de Salida (Multi-Vendor)

```json
{
    "metadata": {
        "collection_start": "2026-02-20T10:00:00-05:00",
        "collection_end": "2026-02-20T10:00:05-05:00",
        "collection_duration_seconds": 5.2,
        "relay_version": "2.0.0",
        "relay_type": "ness-relay-ubuntu",
        "snmp_host": "192.168.1.1",
        "snmp_port": 161,
        "vendor": {
            "id": "fortinet",
            "name": "Fortinet FortiGate",
            "device_type": "firewall",
            "profile_version": "1.0.0"
        }
    },
    "system": {
        "basic_info": { "...estándar RFC 1213..." },
        "timestamp": "...",
        "collection_time_utc": "..."
    },
    "performance": {
        "cpu": {
            "cpu_usage_percent": 45.2,
            "load_1min": 0.5,
            "load_5min": 0.3,
            "load_15min": 0.2,
            "_source": "vendor_specific"
        },
        "memory": {
            "mem_usage_percent": 62.5,
            "mem_total_mb": 4096.0,
            "mem_used_mb": 2560.0,
            "mem_free_mb": 1536.0,
            "_source": "vendor_specific"
        },
        "disk": { "...normalizado igual para todos..." }
    },
    "network": {
        "interfaces": { "...estándar IF-MIB..." }
    },
    "security": {
        "tcp_security": { "...estándar TCP-MIB..." },
        "udp_security": { "...estándar UDP-MIB..." },
        "ip_security": { "...estándar IP-MIB..." },
        "icmp_security": { "...estándar ICMP..." },
        "snmp_security": { "...estándar SNMPv2-MIB..." },
        "normalized": { "..." }
    },
    "vendor_specific": {
        "fortinet": {
            "sessions": { "current": 15000, "ipv6": 200 },
            "firmware": "7.4.1",
            "serial": "FGT60F...",
            "ha_mode": "standalone",
            "antivirus": { "detected": 5 },
            "ips": { "detected": 12 },
            "vpn_tunnels": [ "..." ]
        }
    },
    "security_analysis": { "..." },
    "performance_analysis": { "..." }
}
```

---

## 9. Compatibilidad hacia Atrás

### 9.1 Estrategia de Migración

| Aspecto | v1.0.4 (actual) | v2.0.0 (objetivo) | Compatibilidad |
|---------|-----------------|-------------------|----------------|
| `devices.conf` | vendor: pfsense/cisco/fortinet/windows/linux | +mikrotik, +device_type field | ✅ Backward compatible |
| JSON output (pfSense) | Estructura actual | Misma estructura + `metadata.vendor` | ✅ Backward compatible |
| JSON output (nuevos vendors) | N/A | Nueva estructura con `vendor_specific` | ✅ Additive |
| NESS Server API | `/api/relay/data/` | Mismo endpoint + vendor metadata | ⚠️ Requiere validación |
| Variables de entorno | `NESS_API_TOKEN`, `NESS_SERVER_ID` | Sin cambios | ✅ Backward compatible |
| Archivo de configuración | `devices.conf` existente | Nuevos campos opcionales | ✅ Backward compatible |
| Binario ejecutable | `ness-relay-ubuntu` | `ness-relay-ubuntu` (mismo nombre) | ✅ Drop-in replacement |

### 9.2 Plan de Rollback
Si la v2.0.0 presenta problemas:
1. El binario v1.0.4 se preserva como backup
2. El `devices.conf` existente funciona sin cambios
3. Se puede volver al v1.0.4 simplemente reemplazando el binario

---

## 10. Consideraciones para Futuras Expansiones

### 10.1 Nuevos Tipos de Dispositivo

El sistema de perfiles está diseñado para soportar:

| Tipo | Vendors potenciales | Timeline |
|------|-------------------|----------|
| **Firewall** | pfSense, Fortinet, Cisco ASA, Palo Alto, Sophos | v2.0.0 (actual) |
| **Router** | Cisco IOS, MikroTik, Juniper | v2.1.0 |
| **Switch** | Cisco, HP/Aruba, MikroTik, Juniper | v2.2.0 |
| **Access Point** | Ubiquiti, MikroTik, Cisco, Aruba | v2.3.0 |
| **Servidor** | Linux (net-snmp), Windows (SNMP service) | v2.4.0 |

### 10.2 Nuevos Vendors (Post v2.0)

Para agregar un nuevo vendor, el desarrollador debe:
1. Crear `profiles/vendors/nuevo_vendor.py`
2. Heredar de `BaseDeviceProfile`
3. Implementar los métodos abstractos (OIDs y normalización)
4. Registrar el perfil en `profile_loader.py`
5. Agregar el vendor a `install_relay.sh`
6. Recompilar el binario

**Tiempo estimado para agregar un vendor nuevo:** 1-2 días (una vez el framework está en su lugar)

### 10.3 Consideraciones PyInstaller

Al agregar nuevos vendors, tener en cuenta:
- Los archivos `.py` de perfiles se compilan automáticamente en el binario
- No se necesitan archivos externos (JSON/YAML) — todo está embebido
- Los `--hidden-import` deben incluir cada nuevo módulo de vendor
- Los hooks personalizados (`hooks/`) no necesitan cambios por nuevos vendors

---

## 11. Registro de Progreso

### Historial de Cambios

| Fecha | Fase | Descripción | Estado |
|-------|------|-------------|--------|
| 2026-02-20 | — | Creación del roadmap y plan de desarrollo | ✅ Completado |
| 2026-02-20 | Fase 1 | Diseño del Sistema de Perfiles de Dispositivo | ✅ Completado |
| 2026-02-20 | Fase 2 | Extracción y Organización de OIDs | ✅ Completado |
| 2026-02-20 | Fase 3 | Motor de Recolección Multi-Vendor | ✅ Completado |
| 2026-02-20 | Fase 4A | Perfil pfSense (migración completa) | ✅ Completado |
| 2026-02-25 | Fase 4B | Perfil Fortinet FortiGate | ✅ Completado |
| | Fase 4C | Perfil Cisco | ⬜ Pendiente (stub creado) |
| 2026-02-25 | Fase 4D | Perfil MikroTik | ✅ Completado |
| 2026-02-25 | Fase 4E | Perfil Cambium Networks (Access Points) | ✅ Completado |
| 2026-02-25 | Fase 4F | Perfil Ubiquiti UBNT (Switches) | ✅ Completado |
| 2026-02-25 | Fase 4G | Fortinet WAN/Internet Channel Monitoring | ✅ Completado |
| 2026-02-26 | Fase 4H | Perfil MikroTik Firewall (`mikrotik_fw.py`): Netwatch, WAN interfaces, Queue Simple, canales Internet + submenu instalador | ✅ Completado |
| | Fase 5 | Actualización Build + Instalador | ✅ Completado |
| | Fase 6 | Testing y QA | 🔄 En progreso |
| | Fase 7 | Documentación y Release | ⬜ Pendiente |

### Notas de Desarrollo - Phase 6.1 (Corrección de bugs post-compilación)

**Fecha:** 24 de febrero de 2026

#### Primer test en entorno real — Errores detectados y corregidos

Tras compilar el binario con PyInstaller e instalarlo en una VM Ubuntu, la primera ejecución
falló con 3 bugs críticos. A continuación se documentan las causas raíz y las correcciones.

#### Bug 1 — BASE_DIR apuntaba a `/opt/ness_relay/executables/` en vez de `/opt/ness_relay/`

**Síntoma:** `WARNING - Archivo de configuración no encontrado: /opt/ness_relay/executables/configs/devices.conf`

**Causa raíz:** En `core/config.py`, la detección `BASE_DIR = Path(sys.executable).parent`
resolvia a `/opt/ness_relay/executables/` porque el binario PyInstaller está en esa subcarpeta.
Todas las rutas derivadas (OUTPUT_DIR, LOG_DIR, LOG_FILE, JSON_FILE, CONFIG_FILE) heredaban
el path incorrecto, creando directorios duplicados dentro de `executables/`.

**Corrección en `core/config.py`:**
```python
if getattr(sys, 'frozen', False):
    # 1. Prioridad: variable de entorno NESS_INSTALL_DIR
    _env_install_dir = os.environ.get('NESS_INSTALL_DIR', '')
    if _env_install_dir and Path(_env_install_dir).is_dir():
        BASE_DIR = Path(_env_install_dir)
    else:
        # 2. Fallback: detectar si está dentro de executables/
        _exec_parent = Path(sys.executable).parent
        if _exec_parent.name == 'executables':
            BASE_DIR = _exec_parent.parent
        else:
            BASE_DIR = _exec_parent
```

**Corrección en `install_relay.sh`:** Se agregó `export NESS_INSTALL_DIR` al archivo de
variables de entorno `/etc/profile.d/ness_relay.sh`.

#### Bug 2 — Formato de devices.conf incompatible con configparser

**Síntoma:** Incluso con el path corregido, el parser no encontraría dispositivos.

**Causa raíz:** `load_devices_from_config()` en v2.0 usaba `configparser.ConfigParser()` que
espera formato INI con `[secciones]`. Sin embargo, el instalador genera un formato plano
`key=value` heredado de v1.0.4: `pfsense_1_ip=10.0.0.1`, `pfsense_count=1`, etc.

**Corrección:** Se reescribió la función para parsear el formato plano key=value, iterando
por cada vendor en `SUPPORTED_VENDORS` y leyendo `{vendor}_count` / `{vendor}_{n}_{key}`.
Compatible 1:1 con el formato que genera el instalador y con los archivos existentes de v1.0.4.

#### Bug 3 — Directorio `output/` no se creaba en la instalación

**Síntoma:** El directorio `/opt/ness_relay/output/` no existía tras la instalación.

**Corrección:** Se agregó `mkdir -p "$INSTALL_DIR/output"` al instalador.

#### Bug 4 — `run_relay.sh` usaba rutas relativas y log incorrecto

**Síntoma:** El script pasaba `--config configs/devices.conf` (relativo) que dependía del CWD.
También apuntaba los mensajes de error a `relay.log` en vez de `ness_relay.log`.

**Corrección:** Se cambió a ruta absoluta `--config /opt/ness_relay/configs/devices.conf` y
se actualizaron las referencias de log a `ness_relay.log`.

#### Estructura de directorios correcta post-instalación

```
/opt/ness_relay/                     ← BASE_DIR
├── configs/
│   └── devices.conf                 ← Configuración de dispositivos
├── devices/                         ← Datos de dispositivos (futuro)
├── executables/
│   ├── ness-relay-ubuntu            ← Binario PyInstaller (~47MB)
│   ├── run_relay.sh                 ← Script de ejecución
│   └── view_config.sh              ← Visor seguro de configuración
├── logs/
│   ├── install.log                  ← Log de instalación
│   └── ness_relay.log               ← Log de operación del relay
└── output/
    └── relay_data.json              ← Datos JSON exportados
```

#### Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `core/config.py` | BASE_DIR detection con NESS_INSTALL_DIR + fallback executables/ parent; paths actualizados a `configs/devices.conf`; parser reescrito para formato plano key=value |
| `ness_relay.py` | Default de `--config` cambiado a `configs/devices.conf` |
| `install_relay.sh` | Agregado `output/` a mkdir; `NESS_INSTALL_DIR` en env file; `run_relay.sh` usa paths absolutos y `ness_relay.log` |

---

### Notas de Desarrollo - Phase 6.2 (Compatibilidad pysnmp v7 async API)

**Fecha:** 25 de febrero de 2026

#### Segundo test en entorno real — Error de API pysnmp v7

Tras corregir los bugs de Phase 6.1, se recompiló y reinstanló el binario. Las correcciones
de rutas y config funcionaron correctamente (el log muestra que `devices.conf` se carga OK),
pero apareció un nuevo error runtime relacionado con la API async de pysnmp v7.

#### Bug 5 — `UdpTransportTarget` requiere factory async en pysnmp v7.x

**Síntoma:**
```
File "core/snmp_client.py", line 100, in __init__
File "pysnmp/hlapi/transport.py", line 39, in __init__
Exception: Please call .create() to construct UdpTransportTarget object
```

**Causa raíz:** En pysnmp v7.x, `UdpTransportTarget` cambió su API: ya no se puede
instanciar con `UdpTransportTarget((host, port))`. Ahora requiere el factory method async:
`await UdpTransportTarget.create((host, port))`.

En `core/snmp_client.py`, el constructor `__init__` llamaba
`self._transport = UdpTransportTarget((self.host, self.port))` de forma síncrona,
lo cual es imposible en pysnmp v7 (el `__init__` de UdpTransportTarget lanza Exception).

**Corrección en `core/snmp_client.py`:**
- Se separó la inicialización en dos fases: `__init__` (síncrono, solo atributos) + 
  `create()` (classmethod async, crea transport)
- Se añadió `ContextData()` a las llamadas `get_cmd` y `bulk_cmd` (requerido por pysnmp v7)
- Se añadieron imports faltantes: `ContextData`, `usmNoAuthProtocol`, `usmNoPrivProtocol`

```python
@classmethod
async def create(cls, device_config: Dict[str, Any]) -> 'SnmpClient':
    instance = cls(device_config)
    instance._auth_data = instance._build_auth_data()
    instance._transport = await UdpTransportTarget.create(
        (instance.host, instance.port)
    )
    return instance
```

**Corrección en `core/engine.py`:**
- Línea 97: `client = SnmpClient(device_config)` → `client = await SnmpClient.create(device_config)`

#### Bug 6 — Test de crypto en build script usa import path obsoleto

**Síntoma:** Durante la compilación, la verificación de integración pysnmp+crypto falla:
```
ImportError: cannot import name 'UsmUserData' from 'pysnmp.hlapi'
```

**Causa raíz:** El script de build verifica la integración crypto importando desde
`pysnmp.hlapi` (path de pysnmp v4/v5/v6). En pysnmp v7, el import correcto es
`pysnmp.hlapi.v3arch.asyncio`.

**Corrección en `build_relay_executable.sh`:** Se actualizó el import de verificación a
`from pysnmp.hlapi.v3arch.asyncio import UsmUserData, ...`.

**Nota:** Este bug no impide la compilación (el check falla con warning y continúa), pero es
importante corregirlo para que la verificación de integridad sea confiable.

#### Diagnóstico de advertencias del compilador GCC

Las advertencias que aparecen durante la compilación de Python 3.12.12:

1. **`Parser/tokenizer.c:482` — `-Wimplicit-fallthrough`**: Fall-through intencional en switch
   de CPython. Es código upstream del proyecto Python — no afecta funcionalidad.

2. **`Modules/_decimal/libmpdec/io.c:348` — `-Wstringop-overflow`**: Advertencia del compilador
   sobre la biblioteca `libmpdec` (decimal de alta precisión) incluida en CPython. Es un falso
   positivo conocido del GCC con optimización `-O3` en este código. No afecta funcionalidad.

**Conclusión:** Ambas advertencias son de código C upstream de CPython, no de nuestro código.
El binario Python compilado funciona correctamente. Se pueden ignorar con seguridad.

#### Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `core/snmp_client.py` | Constructor refactorizado a factory async `create()`; añadido `ContextData()` a GET/BULK; imports actualizados para pysnmp v7 |
| `core/engine.py` | `SnmpClient(config)` → `await SnmpClient.create(config)` |
| `build_relay_executable.sh` | Verificación crypto usa `pysnmp.hlapi.v3arch.asyncio` |

---

### Notas de Desarrollo - Phase 1 (Implementación v2.0.0)

**Fecha:** 20 de febrero de 2026

#### Resumen de la implementación

Se completó la migración completa de v1.0.4 (monolítico, 2,285 líneas) a la arquitectura
modular v2.0.0 con 36 archivos Python organizados en 7 paquetes.

#### Archivos implementados (36 archivos)

**Paquete `utils/` (4 archivos)** — Utilidades de bajo nivel
- `crypto_init.py` — Inicialización Cryptodome para PyInstaller + SNMPv3
- `conversions.py` — kb_to_gb, format_uptime, safe_int, safe_float, calculate_percentage
- `helpers.py` — SnmpResult dataclass, now_iso(), print_simple(), safe_log()
- `__init__.py` — Re-exporta todas las utilidades

**Paquete `core/` (6 archivos)** — Núcleo del agente
- `config.py` — Constantes, rutas, URLs, load_devices_from_config(), get_snmp_config_from_env()
- `logging_setup.py` — setup_logging() con rotación manual de archivos
- `snmp_client.py` — Clase SnmpClient (encapsulada por dispositivo, SNMPv1/v2c/v3)
- `updater.py` — Sistema completo de auto-actualización (11 funciones extraídas)
- `engine.py` — CollectionEngine: orquestador vendor-agnostic
- `__init__.py` — Re-exporta constantes principales

**Paquete `profiles/` (10 archivos)** — Sistema de perfiles de dispositivo
- `standard_oids.py` — ~70 OIDs RFC universal (System, IF-MIB, TCP, UDP, IP, ICMP, SNMP Stats)
- `base_profile.py` — ABC BaseDeviceProfile con interfaz obligatoria
- `profile_loader.py` — Registry Pattern + auto-detección via sysObjectID
- `device_types/firewall.py` — Mixin FirewallMixin
- `device_types/router.py` — Stub RouterMixin (Phase 2)
- `device_types/switch.py` — Stub SwitchMixin (Phase 2)
- `device_types/access_point.py` — Stub AccessPointMixin (Phase 2)
- `vendors/pfsense.py` — Perfil COMPLETO: CPU/Memory/Disk (UCD-SNMP-MIB) + PF-MIB
- `vendors/cisco.py` — Stub (Phase 2)
- `vendors/fortinet.py` — Stub (Phase 2)
- `vendors/mikrotik.py` — Stub (Phase 2)
- 4x `__init__.py`

**Paquete `collectors/` (6 archivos)** — Recolectores de datos
- `system_collector.py` — collect_system_data(client) — OIDs RFC estándar
- `performance_collector.py` — collect_performance_data(client, profile) — delega normalización
- `network_collector.py` — collect_network_data(client) — IF-MIB universal
- `security_collector.py` — collect_security_data(client) — TCP/UDP/IP/ICMP/SNMP + normalización
- `vendor_collector.py` — collect_vendor_specific_data(client, profile) — delega al profile
- `__init__.py`

**Paquete `analyzers/` (3 archivos)** — Análisis y alertas
- `security_analyzer.py` — analyze_security_threats() con mismos umbrales que v1.0.4
- `performance_analyzer.py` — analyze_performance_metrics() con mismos umbrales que v1.0.4
- `__init__.py`

**Paquete `exporters/` (3 archivos)** — Exportación y envío
- `json_exporter.py` — export_to_json() — Guarda datos localmente
- `server_sender.py` — send_data_to_server() — Envía al servidor NESS via API REST
- `__init__.py`

**Entry Point**
- `ness_relay.py` — Punto de entrada principal con argparse, inicialización y orquestación

#### Decisiones de arquitectura

1. **SnmpClient encapsulado**: Cada dispositivo tiene su propia instancia de SnmpClient
   (no más variables globales SNMP_HOST, etc.)
2. **Profile-based**: El engine es 100% vendor-agnostic. Le pregunta al Profile qué OIDs
   usar y cómo normalizar los datos.
3. **Registry Pattern**: Los perfiles se registran en ProfileLoader y se obtienen por nombre
   de vendor o por auto-detección via sysObjectID.
4. **Collectors genéricos**: system_collector y network_collector son universales (usan OIDs
   RFC). performance_collector y vendor_collector delegan al Profile.
5. **Compatibilidad v1.0.4**: Mismos OIDs, misma lógica de normalización de memoria
   (mem_avail_real para FreeBSD/pfSense), mismos umbrales de alertas, mismo formato JSON.

#### Próximo paso

Fase 6 — Testing y QA: Compilar, desplegar en entorno de prueba, verificar que la
recolección de datos y el envío al servidor funcionan correctamente con un pfSense real.

---

**Documento mantenido por:** NESS HQ Development Team  
**Última actualización:** 25 de febrero de 2026

---

### Notas de Desarrollo - Phase 4H (MikroTik Firewall + Submenu Instalador)

**Fecha:** 26 de febrero de 2026

#### Contexto

Implementación del perfil de firewall para dispositivos MikroTik. Investigación previa confirmó
que MikroTik no tiene una MIB de firewall separada — todos los dispositivos (CHR, CCR, RB series)
usan el mismo `MIKROTIK-MIB` (enterprise OID `1.3.6.1.4.1.14988`). La distinción entre
"RouterOS" y "Firewall" es **funcional**, no a nivel SNMP.

La diferencia radica en qué datos adicionales se recolectan:
- `mikrotik` (RouterOS): Monitoreo general — CPU, memoria, disco, health, wireless clients
- `mikrotik_fw` (Firewall/Gateway): + Netwatch probes + WAN interface monitoring + Queue Simple bandwidth

#### Feature 1: Perfil MikroTik Firewall/Gateway

**Archivo:** `profiles/vendors/mikrotik_fw.py` (~530 líneas)

**MIBs utilizados:**

| MIB | Categoría | OID Base |
|-----|-----------|----------|
| MIKROTIK-MIB | Netwatch | `1.3.6.1.4.1.14988.1.1.8.1.1.*` |
| MIKROTIK-MIB | Queue Simple | `1.3.6.1.4.1.14988.1.1.2.1.1.*` |
| MIKROTIK-MIB | CPU/Mem/Disk/Health | `1.3.6.1.4.1.14988.1.1.3.*` |
| HOST-RESOURCES-MIB | CPU por core | `1.3.6.1.2.1.25.3.3.1.2` |
| HOST-RESOURCES-MIB | Disco | `1.3.6.1.2.1.25.2.3.1.*` |
| IF-MIB | WAN interfaces | `1.3.6.1.2.1.2.2.1.*` + `1.3.6.1.2.1.31.1.1.1.*` |

**Tabla Netwatch (mtxrNetwatchTable):**

| Sub-OID | Campo | Descripción |
|---------|-------|-------------|
| `.2` | Name | Nombre del probe (ej: `ETB-Principal`) |
| `.3` | Host/IP | IP o hostname monitoreado |
| `.4` | Interval | Intervalo de sondeo (segundos) |
| `.5` | Timeout | Timeout del probe (milisegundos) |
| `.6` | Status | 1=up, 2=down |

**Tabla Queue Simple (mtxrQueueSimpleTable):**

| Sub-OID | Campo | Descripción |
|---------|-------|-------------|
| `.2` | Name | Nombre de la cola |
| `.3` | SrcAddr | Dirección fuente |
| `.4` | DstAddr | Dirección destino |
| `.5` | Interface | Interfaz asociada |
| `.7` | TxByte | Bytes transmitidos |
| `.9` | RxByte | Bytes recibidos |
| `.11` | TxDrop | Paquetes drop TX |
| `.12` | RxDrop | Paquetes drop RX |

**Patrones de detección de interfaces WAN:**
```python
WAN_INTERFACE_PATTERNS = [
    'ether1', 'sfp1', 'sfp-sfpplus1', 'sfpplus1',
    'wan', 'internet', 'isp', 'uplink',
    'pppoe', 'pppoe-out',
    'etb', 'tigo', 'claro', 'movistar', 'azteca',
    'internexa', 'edatel', 'starlink', 'att', 'lte', '4g', 'mpls',
]
```

**Detección de ISPs colombianos/LATAM:**
- ETB, Tigo/UNE, Claro, Movistar, Azteca Comunicaciones
- InterNexa, Edatel, Starlink, AT&T
- LTE/4G Mobile, MPLS Private

**Métodos implementados:**

| Método | Descripción |
|--------|-------------|
| `_collect_wan_interfaces()` | Detecta interfaces WAN por patrón, estado, velocidad, tráfico, errores |
| `_collect_netwatch()` | Lee mtxrNetwatchTable — probes definidos por el administrador |
| `_collect_queues()` | Lee mtxrQueueSimpleTable — ancho de banda por cola |
| `_build_internet_channels_summary()` | Consolida WAN + Netwatch + Queues en vista unificada |
| `_check_channel_alerts()` | Alertas: channel_down, interface_errors, packet_drops |
| `_detect_isp_from_name()` | Detecta ISP del nombre de interfaz/probe |
| `_collect_cpu_per_core()` | hrProcessorLoad bulk walk (igual que RouterOS) |

**Estructura de datos de salida (`vendor_specific`):**
```json
{
  "system_info": { "board_name": "CCR2004-16G-2S+", "firmware": "7.14.2" },
  "health": { "temperature_celsius": 45.2, "voltage_v": 12.0 },
  "cpu_detailed": { "cores": 4, "average_percent": 32.0 },
  "wan_interfaces": [
    {
      "interface_index": "1",
      "name": "ether1",
      "alias": "ETB-Principal",
      "oper_status": "UP",
      "speed_mbps": 1000,
      "isp_detected": "ETB",
      "traffic_in_mbps": 45.2,
      "traffic_out_mbps": 12.1,
      "in_errors": 0,
      "out_errors": 0
    }
  ],
  "netwatch": {
    "available": true,
    "probes": [ { "name": "ETB-Check", "host": "8.8.8.8", "status": "up" } ],
    "summary": { "total": 2, "up": 2, "down": 0 }
  },
  "queues": {
    "available": true,
    "entries": [ { "name": "ISP-ETB", "interface": "ether1", "tx_bytes": 1234567 } ],
    "summary": { "total_queues": 2, "total_tx_bytes": 2345678 }
  },
  "internet_channels": {
    "channels": [ { "channel_name": "ether1", "isp": "ETB", "status": "UP" } ],
    "summary": { "total_channels": 2, "channels_up": 2, "channels_down": 0 }
  }
}
```

#### Feature 2: Submenu MikroTik en el instalador

**Archivo:** `install_relay.sh`

**Cambios:**
- Nueva variable: `VISIBLE_VENDOR_COUNT=8` — separa vendors visibles (8) del total del array (9)
- `mikrotik_fw` se agrega al array `VENDORS` pero con nombre vacío `""` en `VENDOR_NAMES`, ocultándolo del menú principal
- Vendor `mikrotik` en el menú muestra `"MikroTik Devices ▶"` indicando que tiene submenú
- Nueva función `show_mikrotik_submenu()` — presenta dos opciones:
  - `[1] RouterOS` — Dispositivos RouterOS genéricos (CHR, CCR, RB sin énfasis en WAN)
  - `[2] Firewalls / Gateways` — Dispositivos con salida a Internet (configuran `mikrotik_fw`)
- `interactive_vendor_selection()` usa `VISIBLE_VENDOR_COUNT` para el loop; al seleccionar mikrotik llama `show_mikrotik_submenu()`
- El "select all" omite entries con nombre vacío

#### Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `profiles/vendors/mikrotik_fw.py` | ✅ NUEVO — Perfil completo MikroTik Firewall (~530 líneas) |
| `profiles/vendors/__init__.py` | Añadido export: `MikroTikFirewallProfile` |
| `profiles/profile_loader.py` | Registrado `mikrotik_fw → MikroTikFirewallProfile` |
| `core/config.py` | `SUPPORTED_VENDORS`: añadido `'mikrotik_fw'` |
| `build_relay_executable.sh` | `--hidden-import=profiles.vendors.mikrotik_fw`; comentarios actualizados |
| `install_relay.sh` | `VISIBLE_VENDOR_COUNT=8`; `show_mikrotik_submenu()`; submenu routing en `interactive_vendor_selection()` |

#### Regla de mantenimiento

Cuando se agreguen nuevos vendors al array `VENDORS` en `install_relay.sh`, actualizar AMBAS variables:
- `VISIBLE_VENDOR_COUNT` si el vendor debe aparecer en el menú principal
- Agregar nombre vacío `""` en `VENDOR_NAMES` si debe estar oculto (accesible solo via submenú)

---

**Documento mantenido por:** NESS HQ Development Team  
**Última actualización:** 26 de febrero de 2026

---

### Notas de Desarrollo - Phase 4B & 4D (Implementación Fortinet y MikroTik)

**Fecha:** 25 de febrero de 2026

#### Contexto

Con el perfil pfSense funcionando end-to-end (recolección → JSON → servidor → API → BD),
se procedió a implementar los dos siguientes perfiles de vendor: Fortinet FortiGate y
MikroTik RouterOS.

#### Perfil Fortinet FortiGate (`profiles/vendors/fortinet.py`)

**MIB principal:** FORTINET-FORTIGATE-MIB (enterprise OID: 1.3.6.1.4.1.12356)

**Implementación de recolección:**

| Categoría | MIB / OIDs | Método de normalización |
|-----------|-----------|-------------------------|
| CPU | `fgSysCpuUsage` (porcentaje directo 0-100) | `normalize_cpu_data()` — Retorna porcentaje directo; load_1/5/15min = None (FortiGate no expone load averages UNIX) |
| Memoria | `fgSysMemUsage` (porcentaje) + `fgSysMemCapacity` (KB) | `normalize_memory_data()` — Calcula used_mb/free_mb a partir de porcentaje × capacidad; swap = 0 (FortiGate no tiene swap) |
| Disco | `fgSysDiskUsage` (MB) + `fgSysDiskCapacity` (MB) | `normalize_disk_data()` — Convierte MB→GB; valores escalares agrupados como partición "/" |
| Sesiones | `fgSysSesCount`, `fgSysSes6Count`, `fgSysSesRate1` | `collect_vendor_specific_data()` — IPv4, IPv6 y tasa/seg |
| HA | `fgHaSystemMode`, `fgHaGroupId`, `fgHaPriority` | Mapeo: 1=standalone, 2=active-active, 3=active-passive |
| VPN | `fgVpnTunTable` (tabla de túneles IPSec) | `_collect_vpn_tunnels()` — Bulk walk por columna: nombre, estado, tráfico in/out, uptime |
| Security | `fgAvVirusDetected/Blocked`, `fgIpsIntrusionsDetected/Blocked` | Contadores de detecciones |
| System | `fgSysVersion`, `fnSysSerial` | Firmware y número de serie |

**Detección automática:** sysObjectID prefix `1.3.6.1.4.1.12356.101.1` (FortiGate) o
`1.3.6.1.4.1.12356` (Fortinet general).

**Nota importante sobre disco:** Los OIDs de disco de Fortinet son escalares (no tabla).
Sin embargo, el `performance_collector.py` usa `client.bulk()` para todos los disk OIDs.
Los OIDs se definen sin el sufijo `.0` para que el bulk walk retorne el entry escalar
con índice `0`, compatible con el flujo genérico del collector.

#### Perfil MikroTik RouterOS (`profiles/vendors/mikrotik.py`)

**MIBs principales:** HOST-RESOURCES-MIB (RFC 2790) + MIKROTIK-MIB (1.3.6.1.4.1.14988)

**Implementación de recolección:**

| Categoría | MIB / OIDs | Método de normalización |
|-----------|-----------|-------------------------|
| CPU | `hrProcessorLoad` (HOST-RESOURCES-MIB, tabla por core) | `normalize_cpu_data()` — Usa primer entry de get(); CPU detallado (promedio real de todos los cores) en vendor-specific via `_collect_cpu_per_core()` |
| Memoria | `mtxrHlTotalMemory` + `mtxrHlFreeMemory` (MIKROTIK-MIB, bytes) | `normalize_memory_data()` — Convierte bytes→MB; swap = 0 (RouterOS no tiene swap) |
| Disco | `hrStorageTable` (HOST-RESOURCES-MIB, tabla) | `normalize_disk_data()` — Filtra RAM entries por keywords; calcula total/used/free via alloc_units × size |
| Health | `mtxrHlTemperature`, `mtxrHlVoltage`, `mtxrHlCurrent`, `mtxrHlFanSpeed1/2` | Conversiones: temp ÷10 = °C, voltage ÷10 = V |
| System | `mtxrFirmwareVersion`, `mtxrSerialNumber`, `mtxrLicenseId`, `mtxrBoardName` | Strings informativos |
| Wireless | `mtxrWlRtabAddr` (tabla de registrations) o `mtxrWlApClientCount` | `_collect_wireless_clients()` — Cuenta entries via bulk walk |
| Interfaces | `ifNumber` (RFC, total) | Scalar GET |
| Disco fallback | `mtxrHlDiskTotal` + `mtxrHlDiskUsed` (MIKROTIK-MIB, bytes) | Se usa si hrStorageTable no retorna datos de disco |

**Detección automática:** sysObjectID prefix `1.3.6.1.4.1.14988.1` (RouterOS) o
`1.3.6.1.4.1.14988` (MikroTik general).

**Nota sobre hrStorageTable y filtrado de RAM:** El `hrStorageTable` de MikroTik incluye
entries de "Real Memory", "Virtual Memory" y discos/flash. Se filtran los entries que
contienen keywords de RAM (`real memory`, `ram`, `virtual memory`, `swap`, `memory buffers`)
para reportar solo almacenamiento persistente.

#### Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `profiles/vendors/fortinet.py` | Stub → implementación completa (~300 líneas): OIDs, normalización, VPN tunnels, security, HA |
| `profiles/vendors/mikrotik.py` | Stub → implementación completa (~370 líneas): OIDs, normalización, health, wireless, CPU per-core |
| `profiles/vendors/__init__.py` | Añadidos exports: FortinetProfile, MikroTikProfile |
| `profiles/profile_loader.py` | Fortinet/MikroTik: ImportError silencioso → warning con log; Añadidos bloques para CambiumNetworksProfile y UbiquitiProfile (stubs futuros) |
| `core/config.py` | `SUPPORTED_VENDORS` expandido: +cambium_networks, +ubnt |

#### Nuevos vendors pendientes (stubs creados por el equipo)

- **`profiles/vendors/c_n.py`** — Cambium Networks (stub, implementación posterior)
- **`profiles/vendors/ubnt.py`** — Ubiquiti Networks (stub, implementación posterior)

Ambos stubs ya tienen sus bloques de import en `profile_loader.py` y sus nombres
en `SUPPORTED_VENDORS`. Se implementarán una vez completado el testing de Fortinet y MikroTik.

---

### Notas de Desarrollo - Phase 4E, 4F & 4G (UBNT, Cambium y Fortinet WAN Monitoring)

**Fecha:** 25 de febrero de 2026

#### Contexto

Implementación urgente solicitada por cliente que necesita monitoreo de canales de Internet
(ISPs: ETB, Tigo, Claro) en firewalls Fortinet. Además se completaron los perfiles de
Ubiquiti (switches) y Cambium Networks (access points).

#### Feature 1: Fortinet WAN/Internet Channel Monitoring (URGENTE)

**Actualización:** `profiles/vendors/fortinet.py` (~1215 líneas, +780 nuevas)

**MIBs utilizados:**
- `IF-MIB` (RFC 2863) — Contadores de tráfico de interfaz (64-bit)
- `FORTINET-FORTIGATE-MIB` — SD-WAN health checks y SLA monitoring

**Nuevos OIDs implementados:**

| Categoría | OID Base | Descripción |
|-----------|----------|-------------|
| IF-MIB | `1.3.6.1.2.1.2.2.1.*` | ifDescr, ifType, ifSpeed, ifOperStatus |
| IF-MIB HC | `1.3.6.1.2.1.31.1.1.1.*` | ifHCInOctets, ifHCOutOctets (64-bit), ifHighSpeed |
| SD-WAN Health | `1.3.6.1.4.1.12356.101.4.9.2.1.*` | fgVWLHealthCheckLinkLatency, Jitter, PacketLoss, Bandwidth |
| SD-WAN Members | `1.3.6.1.4.1.12356.101.4.9.1.1.*` | fgVWLMemberVdom, Interface, Volume, Sessions |

**Patrones de detección de interfaces WAN:**
```python
WAN_INTERFACE_PATTERNS = [
    'wan', 'internet', 'isp', 'etb', 'tigo', 'claro', 'movistar',
    'fiber', 'mpls', 'lte', '4g', '5g', 'dsl', 'adsl', 'pppoe',
    'uplink', 'external', 'outside', 'port1', 'port2',
]
```

**Detección automática de ISP colombianos/LATAM:**
- ETB, Tigo, Claro, Movistar, Level3, IFX Networks, Internexa, etc.

**Métodos implementados:**

| Método | Descripción |
|--------|-------------|
| `_collect_wan_interfaces()` | Detecta interfaces WAN por patrón, recolecta estado, velocidad, tráfico, errores |
| `_collect_sdwan_health()` | Recolecta SLA health checks: latencia, jitter, packet loss |
| `_detect_isp_from_name()` | Detecta proveedor ISP del nombre de interfaz |
| `_build_internet_channels_summary()` | Consolida datos WAN + SD-WAN en vista unificada por canal |
| `_calculate_health_score()` | Calcula score 0-100 basado en estado, latencia, jitter, packet loss |
| `_check_channel_alerts()` | Genera alertas: channel_down, high_latency, packet_loss, high_jitter |

**Estructura de datos de salida:**
```python
{
    "internet_channels": [
        {
            "interface_index": "1",
            "interface_name": "WAN_ETB",
            "isp_detected": "ETB",
            "status": "UP",
            "speed_mbps": 100,
            "traffic_in_mb": 1234.56,
            "traffic_out_mb": 567.89,
            "latency_ms": 15,
            "jitter_ms": 2,
            "packet_loss_percent": 0.1,
            "health_score": 95,
            "alerts": []
        }
    ],
    "sdwan_health": { ... },
    "wan_interfaces": [ ... ]
}
```

**Umbrales de alertas:**
- `channel_down`: ifOperStatus != 1
- `high_latency`: latencia > 100ms (warning), >200ms (critical)
- `packet_loss`: >1% (warning), >5% (critical)
- `high_jitter`: >30ms (warning), >50ms (critical)
- `interface_errors`: error_rate > 0.01%

#### Feature 2: Perfil Ubiquiti UBNT para Switches

**Archivo:** `profiles/vendors/ubnt.py` (~470 líneas)

**Enterprise OID:** `1.3.6.1.4.1.41112` (Ubiquiti Networks)

**Productos soportados:**
- EdgeSwitch series
- UniFi Switch (USW) series

**MIBs implementados:**

| MIB | Categoría | OIDs |
|-----|-----------|------|
| HOST-RESOURCES-MIB | CPU | hrProcessorLoad |
| HOST-RESOURCES-MIB | Memory/Disk | hrStorageTable |
| IF-MIB | Puertos | ifDescr, ifOperStatus, ifHCInOctets, ifHCOutOctets |
| POWER-ETHERNET-MIB | PoE | pethMainPse*, pethPsePort* |
| Q-BRIDGE-MIB | VLANs | dot1qVlanStaticName, dot1qPvid |
| BRIDGE-MIB | MAC Table | dot1dTpFdbAddress, dot1dTpFdbPort |
| UBNT-UniFi-MIB | Sistema | unifi_model, unifi_version, unifi_temp_* |

**Métodos de recolección:**
- `_collect_ports()` — Estado, velocidad, tráfico, errores de cada puerto
- `_collect_poe()` — Estado PoE global + por puerto (potencia, clase, consumo)
- `_collect_vlans()` — Lista de VLANs configuradas
- `_collect_mac_table()` — Tabla de direcciones MAC (muestra de 50)

#### Feature 3: Perfil Cambium Networks para Access Points

**Archivo:** `profiles/vendors/c_n.py` (~680 líneas)

**Enterprise OID:** `1.3.6.1.4.1.17713` (Cambium Networks)

**Productos soportados:**
- cnPilot E-series (Enterprise APs)
- ePMP (Point-to-Multipoint)
- PMP 450 series

**MIBs implementados:**

| MIB | Categoría | OIDs |
|-----|-----------|------|
| HOST-RESOURCES-MIB | CPU | hrProcessorLoad |
| HOST-RESOURCES-MIB | Memory/Disk | hrStorageTable |
| IF-MIB | Interfaces | ifDescr, ifOperStatus, ifHCInOctets, ifHCOutOctets |
| IEEE 802.11 MIB | Wireless | dot11Channel, dot11TxPower, dot11AssociatedStationCount |
| CAMBIUM-CNS-AP-MIB | Radio | cambium_radio_channel, tx_power, frequency, bandwidth |
| CAMBIUM-CNS-AP-MIB | Clientes | cambium_client_count, client_rssi, client_snr, client_tx_rate |
| CAMBIUM-CNS-AP-MIB | SSIDs | cambium_ssid_name, security, vlan |
| CAMBIUM-CNS-AP-MIB | RF | channel_util, noise_floor, interference |

**Métodos de recolección:**
- `_collect_radio_info()` — Canal, potencia TX, ancho de banda, banda (2.4/5GHz)
- `_collect_clients()` — Conteo total, por banda, detalles (MAC, RSSI, SNR, tasas)
- `_collect_ssids()` — SSIDs configurados con estado, VLAN, seguridad
- `_collect_rf_environment()` — Utilización de canal, piso de ruido, interferencia
- `_collect_interfaces()` — Interfaces ethernet + wireless

#### Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `profiles/vendors/fortinet.py` | +780 líneas: WAN monitoring, SD-WAN, ISP detection, health scoring, alertas |
| `profiles/vendors/ubnt.py` | Stub → implementación completa (~470 líneas) |
| `profiles/vendors/c_n.py` | Stub → implementación completa (~680 líneas) |
| `profiles/vendors/__init__.py` | Añadidos exports: UbiquitiProfile, CambiumProfile |
| `profiles/profile_loader.py` | Registros actualizados con warnings de carga |
| `core/config.py` | SUPPORTED_VENDORS: cambium_networks → c_n |

---

**Documento mantenido por:** NESS HQ Development Team  
**Última actualización:** 25 de febrero de 2026
