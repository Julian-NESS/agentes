# Corrección de Detección Inteligente de Dispositivos

## Diagnóstico de los Problemas

Se identificaron **3 problemas raíz** que están interrelacionados:

---

### Problema 1: MikroTik detectado como "Linux"

La documentación de `automatizar_agente.txt` muestra la evidencia:

```
[Fase C] Deep SNMP Validation
Dispositivo linux_1 (172.17.2.124)
[OK] MDT sysName leído correctamente: MikroTik
```

**Causa raíz**: El flujo actual tiene un conflicto de **doble capa**:

1. **Capa del instalador**: El autocompletado guarda el dispositivo con vendor `linux` (o cualquiera que el usuario seleccione manualmente). El `device_id` resultante en `connection.config` es `linux_1`, lo que hace que `engine.rs` cargue inicialmente el perfil `Linux` (línea 64: `self.profile_loader.get_profile(&device.vendor)`).

2. **Capa del agente (resolve_profile)**: Luego, `engine.rs` (línea 102) intenta re-detectar con `resolve_profile()` usando `sysObjectID` y `sysDescr`. Aquí está el bug:  
   - `mikrotik.rs` y `mikrotik_fw.rs` **ambos** devuelven `true` para `matches_sys_object_id("1.3.6.1.4.1.14988...")`.
   - `auto_detect()` itera sobre un `HashMap`, cuyo orden es **arbitrario**. Esto significa que a veces detecta `mikrotik`, otras `mikrotik_fw`, y nunca se sabe cuál va primero.  
   - Si `auto_detect()` no se ejecuta correctamente (por ejemplo si el sysObjectID viene vacío), se cae al fallback de `sysDescr` donde MikroTik RouterOS contiene "linux" en su descripción del kernel → cae al perfil `linux`.

> [!CAUTION]
> Este mismo bug probablemente afecta a **cualquier vendor** cuyo sysDescr contenga palabras como "linux", "freebsd" o "windows" en su kernel description. Muchos firewalls y switches basados en Linux reportan "Linux" en su sysDescr.

### Problema 2: El autocompletado sigue pidiendo tipo de dispositivo

En `install_relay.sh`, líneas 1299-1338, la sección "AUTOCOMPLETADO CON DATOS DEL SMART TESTER" todavía muestra el menú completo de 8 vendors (Windows, Linux, Cisco, etc.) y le pide al usuario que seleccione uno manualmente. Esto contradice totalmente la filosofía de auto-detección.

### Problema 3: Perfiles Windows/Linux no deberían existir

El agente Ness Relay solo monitorea **firewalls, switches, access points e impresoras** vía SNMP. Los perfiles `linux` y `windows` (usando `GenericProfile`) no tienen sentido porque:
- No son dispositivos de red
- Generan ruido en la detección (el sysDescr de MikroTik contiene "Linux")
- El dashboard no está diseñado para mostrar "servidores"

---

## Propuesta de Cambios

### Componente 1: Motor de Detección en Rust

#### [MODIFY] [loader.rs](file:///home/nessuser/agentes/ness_relay/rust/ness_relay_v2.0.2/src/profiles/loader.rs)

1. **Eliminar los perfiles `linux` y `windows`** del `register_all()`.
2. **Refactorizar `auto_detect()`** para usar un **vector ordenado** de prioridad en vez de iterar un HashMap. El orden debe ser:
   - Primero: vendors con `sysObjectID` más específico (MikroTik FW antes que MikroTik RouterOS)
   - Luego: vendors genéricos como fallback
3. **Resolver el conflicto MikroTik/MikroTik FW**: Ambos comparten el OID `1.3.6.1.4.1.14988`. La distinción actual entre "router" y "firewall" era decidida por el **usuario** durante la instalación. Con auto-detección, proponemos que:
   - `mikrotik` (RouterOS) sea el perfil por defecto cuando se detecte el OID `14988`
   - El usuario **no necesita** distinguir esto durante la instalación

> [!IMPORTANT]
> **Pregunta para ti**: ¿Quieres mantener ambos perfiles (`mikrotik` y `mikrotik_fw`) con la distinción automática basada en alguna heurística (por ejemplo, si el MikroTik tiene Netwatch configurado → es firewall, sino → es router)? ¿O prefieres unificarlos en un solo perfil `mikrotik` que recolecte todo (tanto datos de router como los de firewall: Netwatch, Queues, Internet Channels)?

4. **Eliminar `linux` y `freebsd` del fallback de sysDescr** para evitar falsos positivos.

#### [MODIFY] [vendors/mod.rs](file:///home/nessuser/agentes/ness_relay/rust/ness_relay_v2.0.2/src/profiles/vendors/mod.rs)

- Eliminar `pub mod generic` **NO** — el perfil `generic` sigue siendo necesario como fallback para dispositivos desconocidos.
- Pero sí eliminar la lógica que crea instancias de "linux" y "windows".

---

### Componente 2: Script de Instalación

#### [MODIFY] [install_relay.sh](file:///home/nessuser/agentes/ness_relay/rust/ness_relay_v2.0.2/scripts/install_relay.sh)

1. **Sección de autocompletado (líneas 1299-1338)**: Eliminar completamente el menú de "SELECCIÓN DE DISPOSITIVO" con los 8 vendors. En su lugar, guardar automáticamente el vendor como `generic` sin preguntar nada al usuario.
2. **Texto descriptivo (línea 1265)**: Cambiar "Solo necesitará seleccionar: servidor, token API y tipo de dispositivo" → "Solo necesitará seleccionar: servidor y token API."
3. **Config key**: El dispositivo se guarda como `generic_1` en vez de `linux_1`, `cisco_1`, etc.

---

### Componente 3: Frontend (Django + HTML)

#### [MODIFY] [firewalls.py](file:///home/nessuser/nesshq/snmp/services/firewalls/firewalls.py)

- Eliminar `'generic'` del filtro `device_type__in` (ya no debería llegar ese tipo si la detección funciona).
- Mantener los tipos válidos: `'pfsense'`, `'fortinet'`, `'mikrotik_fw'`, `'firewall'`, `'switch'`, `'router'`, `'ap'`.

#### [MODIFY] [firewalls.html](file:///home/nessuser/nesshq/snmp/templates/firewalls/firewalls.html)

- **Línea 1970 (vendor badge)**: Ampliar la lógica para mostrar badges de todos los vendors soportados (Cisco, Huawei, TP-Link, Dell, UBNT, Cambium, etc.), no solo MikroTik/Fortinet/pfSense.
- **Línea 3143 (AJAX vendor badge)**: Misma ampliación en la lógica JavaScript.

---

## Open Questions

> [!IMPORTANT]
> **MikroTik Router vs Firewall**: Como ambos comparten el OID `1.3.6.1.4.1.14988`, ¿prefieres unificarlos en un solo perfil MikroTik que recolecte todo (Netwatch + Queues + Health), o mantenerlos separados con alguna heurística automática para diferenciarlos?

> [!IMPORTANT]
> **Perfil `generic`**: ¿Qué debe pasar si un dispositivo SNMP no puede ser identificado por ningún vendor conocido? ¿Se descarta silenciosamente, o se reporta como "Dispositivo Desconocido" con métricas básicas (CPU/RAM/interfaces)?

---

## Verificación

### Compilación
- `cargo check` después de los cambios en Rust

### Script
- `bash -n install_relay.sh` para validar sintaxis

### Test funcional
- El MikroTik de la prueba (`172.17.2.124`) deberá aparecer como `mikrotik` (router) y **no** como `linux`
