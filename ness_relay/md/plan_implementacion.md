# Diseño Hiperrealista de Dispositivos — Plan de Implementación v3

> **Documento vivo.** Esta versión (v3) reemplaza a v1 y v2.
> Recoge el análisis directo del código real:
>  - Proyecto V0 (Next.js 16 + React 19 + Tailwind 4) en [new_design/diseño_hiper_realista](../new_design/diseño_hiper_realista/)
>  - Vista Django en [nesshq/snmp/services/firewalls/firewall_detail.py](../../nesshq/snmp/services/firewalls/firewall_detail.py)
>  - Template en [nesshq/snmp/templates/firewalls/firewall_detail.html](../../nesshq/snmp/templates/firewalls/firewall_detail.html)
>  - Agente NESS Relay (Rust) en [agentes/ness_relay/rust/ness_relay_v2.1.0/](../ness_relay/rust/ness_relay_v2.1.0/)
>  - Modelos del backend en [nesshq/api/models.py](../../nesshq/api/models.py)
>
> **Estado de decisiones del usuario** (capturadas durante la revisión):
>  - **Detección copper/fiber:** Modificar agente + modelo Django (campo `media`).
>  - **Fallback vendor no mapeado:** Faceplate genérico (no grid antiguo).
>  - **Toggle LANs/VLANs:** Conservado, en estilo V0 (sobre el faceplate, fuera del título).
>  - **Sparkline:** SVG por defecto + ECharts al hacer hover (implementar ambos).

---

## 1. Resumen Ejecutivo

Reemplazar la visualización actual de puertos (iconos `bi-ethernet` en grid plano) por **faceplates hiperrealistas dibujados 100% con HTML + CSS vanilla**, replicando la técnica que V0/Vercel demostró con el TP-Link T1700G-28TQ:

- Chasis con `linear-gradient` + `repeating-linear-gradient` (metal cepillado).
- Puertos RJ45 con muesca superior, 4 pines dorados y LED dinámico.
- Jaulas SFP+ con apertura interna.
- Color = estado (verde/rojo) con `box-shadow` glow.

**La detección del modelo es dinámica**: el agente Rust ya envía `device_type` + `sys_descr`; el backend identifica el modelo canónico y selecciona el partial template correspondiente. Si el vendor/modelo no está mapeado, se usa un faceplate genérico (no se cae al grid de iconos).

> **No se usan imágenes ni SVG externos**: todo se renderiza con `<div>` + `<button>` + gradientes CSS. Esto garantiza nitidez a cualquier escala, tematización dinámica y peso ~0 bytes de assets.

---

## 2. Hallazgos Críticos del Análisis de Código (Pre-Implementación)

Antes de implementar, estos son los puntos descubiertos al leer el código real que **deben corregirse** o **aprovecharse**:

### 2.1 El agente ya recolecta `ifType` pero se pierde

- [collectors/network.rs:31](../ness_relay/rust/ness_relay_v2.1.0/src/collectors/network.rs#L31) ejecuta `client.bulk(oids["ifType"], 50)` y lo guarda con clave `"type"` en el JSON interno (línea ~157).
- [exporters/payload_compat.rs:75-100](../ness_relay/rust/ness_relay_v2.1.0/src/exporters/payload_compat.rs#L75-L100) (`transform_network`) **descarta** ese campo. Solo persiste `index`, `name`, `admin_status`, `operational_status`, `speed_mbps`, `traffic_*_mb`, `errors_*`, `discards_*`.
- [api/models.py:1500-1530](../../nesshq/api/models.py#L1500-L1530) (`RelayNetworkInterface`) tampoco tiene un campo `media`.

**Implicación:** para que el faceplate pueda separar bancos RJ45 de jaulas SFP+, **el campo `media` debe propagarse agente → modelo Django**. Sin esto, todo se vería como cobre (incorrecto para 25-28 de un T1700G-28TQ).

**Decisión adoptada:** agregar `media` en los tres puntos:
1. `payload_compat.rs::transform_network` — incluir `"media"` mapeado desde `ifType` numérico (6=ethernetCsmacd, 117=gige → copper; valores altos tipo gigabitEthernet y fiber → fiber; fallback por nombre).
2. `RelayNetworkInterface` — agregar `media = models.CharField(max_length=10, blank=True, null=True)` con `choices=[('copper', 'Cobre'), ('fiber', 'Fibra')]`.
3. `firewall_detail.py::_make_port_entry` — usar el campo, con heurística de respaldo por nombre (`'Te', 'Fo', 'Sf', 'Xg'` → fiber; resto → copper).

### 2.2 La app `snmp` no tiene directorio `static/`

- `nesshq/snmp/apps.py` declara `name = 'snmp'`.
- `nesshq/snmp/static/` **no existe** (verificado con `list_dir` y `file_search`).
- El template actual usa `<style>` inline gigante (~1700 líneas) y `{% static 'assets/libs/echarts/echarts.min.js' %}` que se resuelve contra el `staticfiles/` colectado de otra app.

**Implicación:** el plan v1-v2 proponía `snmp/static/css/faceplates.css`. Esto es **válido** (Django lo encuentra automáticamente), pero hay que:
- Crear el directorio `nesshq/snmp/static/css/` y `nesshq/snmp/static/js/faceplates.js`.
- Cargar con `{% static 'css/faceplates.css' %}` — Django servirá desde `STATICFILES_DIRS` o `staticfiles/` tras `collectstatic`.
- En desarrollo (DEBUG=True), `runserver` lo sirve directo desde `snmp/static/`.

### 2.3 `sys_descr` + `sys_info` ya llegan al backend

- [firewall_detail.py:1100-1115](../../nesshq/snmp/services/firewalls/firewall_detail.py#L1100-L1115) ya construye `device_identity` a partir de `sys_info.mtxr_board_name/board_name/model/platform/sys_name`.
- El campo `device_vendor = device.vendor or device.device_type` mapea al `VENDOR_SECTION_REGISTRY`.

**Implicación:** se puede introducir un campo canónico `device_model_code` (slug normalizado) sin tocar el JSON del agente, derivándolo en `firewall_detail_view` con regex sobre `sys_descr` o `sys_info`. Ejemplo: `T1700G-28TQ` → `tp_link_t1700g_28tq`. Esto permite que el faceplate sea por **modelo**, no solo por vendor.

### 2.4 V0 usa `oklch()` y CSS variables de shadcn

- [new_design/.../app/globals.css:88-100](../new_design/diseño_hiper_realista/app/globals.css#L88-L100) define `--status-up: oklch(0.62 0.17 145)`, `--status-down: oklch(0.58 0.21 25)`.
- El soporte de `oklch()` está en Chrome 111+, Firefox 113+, Safari 15.4+. Aceptable en navegadores modernos pero **mejor precomputar a hex** para audit/consistencia.

**Decisión:** mantener el `oklch()` en el CSS (legible y copiable directamente desde V0), y exponer las mismas variables en `:root` con fallback hex para seguridad:
```css
:root {
  --faceplate-up: oklch(0.62 0.17 145);
  --faceplate-up-fallback: #2ea043;
  --faceplate-down: oklch(0.58 0.21 25);
  --faceplate-down-fallback: #e23b3b;
}
```

### 2.5 `selectPort()` y la lógica de tráfico ya existen

- [firewall_detail.html (script, líneas ~2900-3846)](../../nesshq/snmp/templates/firewalls/firewall_detail.html) ya define `window.selectPort = function(el) { ... }` que lee `data-iface-index`, llama a la API y pinta el chart de ECharts en `#portTrafficChart`.
- El plan v1-v2 preguntaba si mantener o mejorar. **Respuesta basada en el código:** se mantiene tal cual. El faceplate solo cambia la **presentación visual** del puerto; los `data-*` y `onclick="selectPort(this)"` se conservan idénticos.

### 2.6 El `ethernet-ports-grid` actual consume el ancho completo de la tarjeta

- [firewall_detail.html:1844-1935](../../nesshq/snmp/templates/firewalls/firewall_detail.html#L1844-L1935) define `.ethernet-ports-grid` con `grid-template-columns: repeat(auto-fill, minmax(90px, 1fr))`.
- Con 24+4 puertos, el wrap horizontal funciona. El faceplate de V0 es **horizontal y continuo** (una sola fila visual). Si la cantidad de cobre supera el ancho del contenedor, el chasis debe envolver a una segunda fila de bancos.

**Decisión:** el template del faceplate calculará el número de bancos en función de `copper_ports|length`. Para TP-Link con 24 cobre, 3 bancos de 8 (idéntico a V0). Para 48 cobre, 3 bancos de 16 (dos filas) o un grid 2×24 — a definir al implementar.

### 2.7 El agente identifica el vendor por `sys_descr`, no por `model`

- [profiles/loader.rs:360-365](../ness_relay/rust/ness_relay_v2.1.0/src/profiles/loader.rs#L360-L365) detecta TP-Link buscando `tp-link` o `tplink` en `sys_descr`. El perfil carga el board name y version de OIDs propietarios `1.3.6.1.4.1.11863.*`.
- `sys_descr` para un T1700G-28TQ real luce así:
  `TP-Link JetStream 24-Port Gigabit Stackable Smart Switch with 4 10GE SFP+ Slots T1700G-28TQ 2.0.1 Build 20170608 Rel.61525(s)`

**Implicación:** el `model_code` se puede extraer con un regex simple sobre `sys_descr` o `sys_info.mtxr_board_name` (si está disponible). Para TP-Link, además, el prefijo `1.3.6.1.4.1.11863` confirma el vendor.

### 2.8 El proyecto V0 modela la cara frontal completa: marca, indicadores, puertos

- [components/switch-faceplate.tsx](../new_design/diseño_hiper_realista/components/switch-faceplate.tsx) renderiza 4 zonas:
  1. **BrandBlock** (logo + descripción + modelo) — ancho fijo 120px.
  2. **IndicatorPanel** (LEDs Link/Act numerados, PWR/SYS/Master, leyenda 10Gbps/1Gbps, display 7 segmentos "Unit", botón Reset).
  3. **3 bancos de cobre** (par arriba / impar abajo).
  4. **Jaulas SFP+** (alineadas a la derecha).

- Los **indicadores** (PWR/SYS/Master) en V0 son **siempre verdes** (hardcoded en el style). En la realidad deben reflejar el estado del switch: PWR siempre on, SYS parpadea, Master solo en stacks. Para v3 los dejaremos como en V0 (siempre verde) por simplicidad; en v4 se podrán cablear a métricas reales (`device_status`).

---

## 3. Arquitectura de la Solución

### 3.1 Concepto Central: Device Faceplate Registry

Cada vendor/modelo tiene su propio **partial template** de faceplate. El backend selecciona cuál incluir según un código canónico derivado de `sys_descr` + `sys_info`. La carga es dinámica con `{% include faceplate_template %}`.

```
nesshq/snmp/
├── static/
│   ├── css/faceplates.css           ← [NUEVO] Vanilla CSS: chasis, RJ45, SFP+, LEDs
│   └── js/faceplates.js             ← [NUEVO] Click handler alternativo + accessibility
├── templates/firewalls/
│   ├── firewall_detail.html         ← [MODIFY] Integra {% include faceplate_template %}
│   └── faceplates/                  ← [NUEVO]
│       ├── _faceplate_tplink.html   ← Switches TP-Link (T1700G, TL-SG)
│       ├── _faceplate_huawei.html   ← Switches Huawei (S5735, S5720)
│       ├── _faceplate_mikrotik.html ← Routers/switches MikroTik
│       ├── _faceplate_fortinet.html ← FortiGate
│       ├── _faceplate_pfsense.html  ← pfSense / OPNsense
│       └── _faceplate_generic.html  ← Fallback universal
└── services/firewalls/firewall_detail.py  ← [MODIFY] Resolución de faceplate
```

### 3.2 Resolución del faceplate (Backend)

```python
# Mapa vendor → template por defecto.
# Dentro de cada vendor, se especializa por modelo si está disponible.
FACEPLATE_TEMPLATES = {
    'tp_link':    'firewalls/faceplates/_faceplate_tplink.html',
    'huawei':     'firewalls/faceplates/_faceplate_huawei.html',
    'mikrotik':   'firewalls/faceplates/_faceplate_mikrotik.html',
    'mikrotik_fw':'firewalls/faceplates/_faceplate_mikrotik.html',
    'fortinet':   'firewalls/faceplates/_faceplate_fortinet.html',
    'pfsense':    'firewalls/faceplates/_faceplate_pfsense.html',
    'opnsense':   'firewalls/faceplates/_faceplate_pfsense.html',
    'cisco':      'firewalls/faceplates/_faceplate_generic.html',
    'ubnt':       'firewalls/faceplates/_faceplate_generic.html',
    'dell':       'firewalls/faceplates/_faceplate_generic.html',
    # ... resto cae al genérico
}
_DEFAULT_FACEPLATE = 'firewalls/faceplates/_faceplate_generic.html'

# Mapa modelo → template (sobreescribe el del vendor).
# Permite que un mismo vendor tenga varios faceplates (ej: TP-Link 24p vs 48p).
FACEPLATE_BY_MODEL = {
    't1700g_28tq':  'firewalls/faceplates/_faceplate_tplink.html',
    't2600g_28ts':  'firewalls/faceplates/_faceplate_tplink.html',
    'tl_sg3428':    'firewalls/faceplates/_faceplate_tplink.html',
    's5735_l24t4s': 'firewalls/faceplates/_faceplate_huawei.html',
    # ... se irá nutriendo
}

def _resolve_faceplate(device, vendor_metrics_data):
    model_code = _extract_model_code(device, vendor_metrics_data)
    if model_code in FACEPLATE_BY_MODEL:
        return FACEPLATE_BY_MODEL[model_code]
    return FACEPLATE_TEMPLATES.get(device.vendor or device.device_type, _DEFAULT_FACEPLATE)


def _extract_model_code(device, vendor_metrics_data):
    """
    Devuelve un slug canónico del modelo (ej: 't1700g_28tq', 's5735_l24t4s').
    Prioriza:
      1. sys_info.mtxr_board_name / board_name / model (si está)
      2. Regex sobre sys_descr buscando patrones tipo 'T1700G-28TQ', 'S5735-L24T4S'
      3. None (cae al mapa del vendor)
    """
    sys_info = vendor_metrics_data.get('system_info', {})
    raw = (
        sys_info.get('mtxr_board_name')
        or sys_info.get('board_name')
        or sys_info.get('model')
        or device.sys_name
        or device.sys_descr
        or ''
    )
    # Regex: captura secuencias tipo T1700G-28TQ, S5735-L24T4S, CCR1009-7G
    m = re.search(r'\b([A-Za-z]{1,4}[-_]?\d{3,4}[A-Za-z]?[-_]?[A-Za-z0-9-]*)\b', raw)
    if m:
        return re.sub(r'[^a-z0-9]+', '_', m.group(1).lower()).strip('_')
    return None
```

### 3.3 Estructura del contexto (lo que verá el template)

```python
context = {
    # ... existentes ...
    'faceplate_template': _resolve_faceplate(device, vendor_metrics_data),
    'device_model_code':  _extract_model_code(device, vendor_metrics_data),
    'device_model_label': _extract_model_label(device, vendor_metrics_data),
    'copper_ports': [p for p in lan_ports if p.get('media') == 'copper'],
    'fiber_ports':  [p for p in lan_ports if p.get('media') == 'fiber'],
    'vlan_interfaces': vlan_ports,  # para el toggle
}
```

Y `_make_port_entry` se extiende con:
```python
def _make_port_entry(iface, idx):
    # ... código existente ...
    return {
        # ... campos existentes ...
        'media': _classify_media(iface),  # NUEVO
    }

def _classify_media(iface):
    """Clasifica un puerto como copper/fiber."""
    # 1) Si el campo del modelo ya viene seteado, usarlo.
    media = getattr(iface, 'media', None)
    if media in ('copper', 'fiber'):
        return media
    # 2) Heurística por nombre (fallback si la migración no se ha aplicado).
    name = (iface.name or '').lower()
    if any(t in name for t in ('te', 'fo', 'sf', 'xg', 'xe')):
        return 'fiber'
    if any(t in name for t in ('gi', 'fa', 'eth', 'fe', 'ge')):
        return 'copper'
    # 3) Por velocidad: 10G+ probablemente es fibra.
    if iface.speed_mbps and iface.speed_mbps >= 10000:
        return 'fiber'
    return 'copper'
```

---

## 4. Componente 1 — CSS de Faceplates (faceplates.css)

### 4.1 Ubicación y carga

- Archivo: `nesshq/snmp/static/css/faceplates.css`
- Carga en el template: `<link rel="stylesheet" href="{% static 'css/faceplates.css' %}">` dentro del bloque `{% block styles %}` (justo después del `<link>` de bootstrap-icons).

### 4.2 Estructura del CSS (traducción directa de V0 → vanilla)

El CSS replica 1:1 las clases que usa el template V0 (`device-chassis`, `rj45-port`, `sfp-cage`, `port-led`, `chassis-brand`, `indicator-panel`). El chasis es un `<div>` con `linear-gradient` + `repeating-linear-gradient` superpuesto. Los puertos son `<button>` con su LED como `<span>` hijo.

**Variables y tokens (en `:root`):**

```css
:root {
  /* Chasis metálicos */
  --chassis-tplink-top:    #3a3f44;
  --chassis-tplink-mid:    #2c3034;
  --chassis-tplink-bot:    #25282c;
  --chassis-tplink-border: #15171a;

  --chassis-mikrotik-top:  #1f2530;
  --chassis-mikrotik-bot:  #0d1014;

  --chassis-fortinet-top:  #1a1c1f;
  --chassis-fortinet-bot:  #0a0b0c;
  --chassis-fortinet-stripe: #ee3124;

  --chassis-huawei-top:    #4a4a4a;
  --chassis-huawei-bot:    #2a2a2a;

  --chassis-pfsense-top:   #d8d8d8;
  --chassis-pfsense-bot:   #a8a8a8;

  /* Puerto RJ45 (cobre) */
  --rj45-grad-top:    #d9d4cc;
  --rj45-grad-mid:    #b9b4ab;
  --rj45-grad-bot:    #8f8b83;
  --rj45-border:      #4a4843;
  --rj45-pin:         #c9a227;
  --rj45-notch:       #23262a;

  /* Jaula SFP+ */
  --sfp-grad-top:     #2a2d31;
  --sfp-grad-mid:     #1c1e21;
  --sfp-grad-bot:     #141517;
  --sfp-aperture-top: #0a0b0c;
  --sfp-aperture-bot: #16181b;

  /* Estado */
  --faceplate-up:        oklch(0.62 0.17 145);
  --faceplate-up-fb:     #2ea043;
  --faceplate-down:      oklch(0.58 0.21 25);
  --faceplate-down-fb:   #e23b3b;
}
```

**Clases principales (esqueleto, no exhaustivo):**

```css
.device-chassis {
  position: relative;
  display: flex;
  align-items: stretch;
  gap: 0.75rem;
  padding: 0.75rem;
  border-radius: 6px;
  background: linear-gradient(180deg, var(--chassis-tplink-top), var(--chassis-tplink-mid) 18%, var(--chassis-tplink-bot) 100%);
  border: 1px solid var(--chassis-tplink-border);
  box-shadow: inset 0 1px 0 rgba(255,255,255,0.08), 0 8px 24px rgba(0,0,0,0.35);
}
.device-chassis::before {                 /* textura metal cepillado */
  content: '';
  position: absolute; inset: 0;
  border-radius: inherit;
  background: repeating-linear-gradient(180deg, rgba(255,255,255,0.04) 0 1px, transparent 1px 3px);
  opacity: 0.3;
  pointer-events: none;
}

/* Modificadores por vendor */
.chassis--tplink    { background: linear-gradient(180deg, var(--chassis-tplink-top), var(--chassis-tplink-mid) 18%, var(--chassis-tplink-bot)); }
.chassis--mikrotik  { background: linear-gradient(180deg, var(--chassis-mikrotik-top), var(--chassis-mikrotik-bot)); }
.chassis--fortinet  { background: linear-gradient(180deg, var(--chassis-fortinet-top), var(--chassis-fortinet-bot)); border-bottom: 3px solid var(--chassis-fortinet-stripe); }
.chassis--huawei    { background: linear-gradient(180deg, var(--chassis-huawei-top), var(--chassis-huawei-bot)); }
.chassis--pfsense   { background: linear-gradient(180deg, var(--chassis-pfsense-top), var(--chassis-pfsense-bot)); }

.rj45-port {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  background: none;
  border: 0;
  cursor: pointer;
  font: inherit;
}
.rj45-body {
  position: relative;
  width: 24px; height: 20px;
  border: 1px solid var(--rj45-border);
  border-radius: 2px;
  background: linear-gradient(180deg, var(--rj45-grad-top) 0%, var(--rj45-grad-mid) 45%, var(--rj45-grad-bot) 100%);
  box-shadow: inset 0 -2px 3px rgba(0,0,0,0.35);
  transition: box-shadow .15s;
}
.rj45-port:hover .rj45-body { box-shadow: 0 0 0 1px rgba(255,255,255,0.4), inset 0 -2px 3px rgba(0,0,0,0.35); }
.rj45-port.is-selected .rj45-body { outline: 2px solid var(--vendor-color, #032647); outline-offset: 1px; }
.rj45-notch { position: absolute; left: 50%; top: 0; transform: translateX(-50%); width: 9px; height: 5px; background: var(--rj45-notch); border-radius: 0 0 2px 2px; }
.rj45-pins  { position: absolute; left: 4px; right: 4px; top: 6px; height: 3px; display: flex; justify-content: space-between; }
.rj45-pins > span { width: 1px; background: var(--rj45-pin); }

.port-led {
  position: absolute; bottom: 2px; left: 2px; width: 3px; height: 3px; border-radius: 50%;
  background: var(--faceplate-down-fb);
  opacity: 0.55;
}
.rj45-port.is-up   .port-led { background: var(--faceplate-up);   box-shadow: 0 0 4px var(--faceplate-up);   opacity: 1; }
.rj45-port.is-down .port-led { background: var(--faceplate-down); box-shadow: none; }
.rj45-port.is-unused .port-led { background: #3a3d41; box-shadow: none; opacity: 0.4; }

.sfp-cage {
  position: relative; width: 28px; height: 24px;
  border: 1px solid #0c0d0f; border-radius: 2px;
  background: linear-gradient(180deg, var(--sfp-grad-top) 0%, var(--sfp-grad-mid) 60%, var(--sfp-grad-bot) 100%);
  box-shadow: inset 0 1px 2px rgba(255,255,255,0.06);
}
.sfp-cage::after {     /* apertura interna */
  content: ''; position: absolute; left: 3px; right: 3px; top: 4px; bottom: 6px;
  background: linear-gradient(180deg, var(--sfp-aperture-top), var(--sfp-aperture-bot));
  border-radius: 1px;
}
.sfp-cage .port-led { left: 50%; transform: translateX(-50%); }
.sfp-cage.is-up   .port-led { background: var(--faceplate-up);   box-shadow: 0 0 4px var(--faceplate-up); }
.sfp-cage.is-down .port-led { background: var(--faceplate-down); }

.chassis-brand { width: 120px; flex-shrink: 0; display: flex; flex-direction: column; justify-content: space-between; }
.chassis-brand .brand-logo   { font-size: 15px; font-weight: 700; color: #f4f4f5; letter-spacing: -0.01em; }
.chassis-brand .brand-desc   { font-size: 7px;  color: #a1a1aa; line-height: 1.2; margin-top: 6px; }
.chassis-brand .brand-model  { font-size: 8px;  color: #d4d4d8; font-weight: 600; letter-spacing: 0.05em; margin-top: 12px; }

.indicator-panel { display: flex; align-items: stretch; gap: 8px; flex-shrink: 0; }
.indicator-led   { width: 6px; height: 6px; border-radius: 50%; background: var(--faceplate-up); box-shadow: 0 0 4px var(--faceplate-up); }
.indicator-led.is-off { background: #3a3d41; box-shadow: none; }
.led-row { display: flex; gap: 3px; }
.led-row > span { width: 5px; height: 5px; border-radius: 50%; }
.led-row > span.is-up   { background: var(--faceplate-up);   box-shadow: 0 0 3px var(--faceplate-up); }
.led-row > span.is-down { background: #3a3d41; box-shadow: none; }

.seven-segment {
  display: flex; align-items: center; justify-content: center;
  width: 20px; height: 28px;
  background: #0b0c0d; border: 1px solid #000; border-radius: 2px;
  font-family: ui-monospace, SFMono-Regular, monospace; font-size: 15px; font-weight: 700; line-height: 1;
  color: #e23b3b; text-shadow: 0 0 5px rgba(226,59,59,0.7);
}

.faceplate-legend { display: flex; align-items: center; gap: 1rem; padding-top: 0.75rem; margin-top: 0.75rem; border-top: 1px solid #DEE2E6; font-size: 0.75rem; color: var(--text-secondary); }
.legend-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 4px; vertical-align: middle; }
.legend-dot.is-up   { background: var(--faceplate-up-fb); }
.legend-dot.is-down { background: var(--faceplate-down-fb); }

/* Responsivo */
@media (max-width: 992px) {
  .device-chassis { flex-wrap: wrap; }
  .chassis-brand  { width: 100%; flex-direction: row; align-items: center; gap: 1rem; }
  .indicator-panel { width: 100%; justify-content: flex-start; }
}
```

> **Por qué CSS vanilla y no Tailwind:** el proyecto Django de nesshq no usa Tailwind (el CSS de `firewall_detail.html` es inline tradicional). Importar Tailwind solo para el faceplate añadiría un pipeline de build. V0 sí usa Tailwind 4 vía Vite, pero en Django basta con vanilla. Los valores se traducen 1:1 desde el JSX.

---

## 5. Componente 2 — Templates Parciales

### 5.1 `_faceplate_tplink.html` (caso hero)

Equivalente directo de [switch-faceplate.tsx](../new_design/diseño_hiper_realista/components/switch-faceplate.tsx). Renderiza:

- Chasis con clase `device-chassis chassis--tplink`.
- `chassis-brand` con "TP-LINK", descripción genérica del switch, y modelo (`{{ device_model_label }}`).
- `indicator-panel` con dos filas de LEDs Link/Act (pares arriba, impares abajo) + LEDs PWR/SYS/Master siempre encendidos + leyenda 10Gbps/1Gbps/Activity + display 7 segmentos con el número de unidad.
- Bancos de cobre: 3 bancos de 8 (par arriba / impar abajo) — **o se generaliza con `forloop.counter` para soportar 8/16/24/48**.
- Jaulas SFP+ alineadas a la derecha, con label "SFP+" debajo.

```django
{# faceplates/_faceplate_tplink.html #}
{% load static %}
<div class="device-chassis chassis--tplink" role="group" aria-label="Facepanel TP-Link {{ device_model_label }}">
  <div class="chassis-brand">
    <div>
      <span class="brand-logo">TP-LINK</span>
      <p class="brand-desc">JetStream Gigabit Stackable Smart Switch</p>
    </div>
    <p class="brand-model">{{ device_model_label|default:"T-Series" }}</p>
  </div>

  {# ── Panel de indicadores ── #}
  <div class="indicator-panel">
    <div class="indicator-block">
      <span class="indicator-label">Link/Act</span>
      <div class="led-row">
        {% for p in copper_ports %}{% if forloop.counter0|divisibleby:2 %}<span class="{% if p.state == 'up' %}is-up{% else %}is-down{% endif %}" title="{{ p.label }}"></span>{% endif %}{% endfor %}
      </div>
      <div class="led-row">
        {% for p in copper_ports %}{% if forloop.counter0|add:1|divisibleby:2 %}<span class="{% if p.state == 'up' %}is-up{% else %}is-down{% endif %}" title="{{ p.label }}"></span>{% endif %}{% endfor %}
      </div>
      <span class="indicator-label">1000Mbps</span>
    </div>
    <div class="indicator-stack">
      <span><span class="indicator-led"></span> PWR</span>
      <span><span class="indicator-led"></span> SYS</span>
      <span><span class="indicator-led"></span> Master</span>
    </div>
    <div class="indicator-legend">
      <span><span class="indicator-led"></span> 10Gbps</span>
      <span><span class="indicator-led is-off"></span> 1Gbps</span>
      <span><span class="indicator-led"></span> Activity</span>
      <div class="seven-segment-row">
        <div class="seven-segment">1</div>
        <button class="reset-button" type="button" aria-label="Reset" title="Reset"></button>
      </div>
      <span class="indicator-label">Unit</span>
    </div>
  </div>

  <div class="chassis-divider" aria-hidden></div>

  {# ── Bancos de cobre ── #}
  <div class="chassis-banks">
    {% for bank in copper_banks %}
    <div class="port-bank">
      <div class="port-row">
        {% for p in bank.top_row %}
        <button type="button" class="rj45-port is-{{ p.state }}{% if p.iface_index == selected_iface_index %} is-selected{% endif %}"
                data-iface-index="{{ p.iface_index }}" data-port-name="{{ p.label }}"
                data-port-rx="{{ p.rx }}" data-port-tx="{{ p.tx }}" data-port-speed="{{ p.speed }}"
                onclick="selectPort(this)" title="{{ p.label }} · {{ p.state|upper }}">
          <span class="rj45-label">{{ p.face_label }}</span>
          <span class="rj45-body">
            <span class="rj45-notch" aria-hidden></span>
            <span class="rj45-pins" aria-hidden><span></span><span></span><span></span><span></span></span>
            <span class="port-led" aria-hidden></span>
          </span>
        </button>
        {% endfor %}
      </div>
      <div class="port-row">
        {% for p in bank.bottom_row %}
        <button type="button" class="rj45-port is-{{ p.state }}{% if p.iface_index == selected_iface_index %} is-selected{% endif %}"
                data-iface-index="{{ p.iface_index }}" data-port-name="{{ p.label }}"
                data-port-rx="{{ p.rx }}" data-port-tx="{{ p.tx }}" data-port-speed="{{ p.speed }}"
                onclick="selectPort(this)" title="{{ p.label }} · {{ p.state|upper }}">
          <span class="rj45-label">{{ p.face_label }}</span>
          <span class="rj45-body">
            <span class="rj45-notch" aria-hidden></span>
            <span class="rj45-pins" aria-hidden><span></span><span></span><span></span><span></span></span>
            <span class="port-led" aria-hidden></span>
          </span>
        </button>
        {% endfor %}
      </div>
    </div>
    {% endfor %}

    {# ── Jaulas SFP+ ── #}
    <div class="sfp-stack">
      <div class="sfp-row">
        {% for p in fiber_ports %}
        <button type="button" class="sfp-cage is-{{ p.state }}{% if p.iface_index == selected_iface_index %} is-selected{% endif %}"
                data-iface-index="{{ p.iface_index }}" data-port-name="{{ p.label }}"
                data-port-rx="{{ p.rx }}" data-port-tx="{{ p.tx }}" data-port-speed="{{ p.speed }}"
                onclick="selectPort(this)" title="{{ p.label }} · {{ p.state|upper }}" aria-label="SFP+ {{ p.label }}">
          <span class="sfp-label">{{ p.face_label }}</span>
          <span class="port-led" aria-hidden></span>
        </button>
        {% endfor %}
      </div>
      <span class="sfp-tag">SFP+</span>
    </div>
  </div>
</div>
```

> **Nota:** la preparación de `copper_banks` (con `top_row` y `bottom_row` por banco) se hace en `firewall_detail.py` con una función auxiliar:
> ```python
> def _chunk_copper_banks(copper_ports, bank_size=8):
>     banks = []
>     for i in range(0, len(copper_ports), bank_size):
>         chunk = copper_ports[i:i+bank_size]
>         banks.append({
>             'top_row':    [p for j, p in enumerate(chunk) if j % 2 == 0],
>             'bottom_row': [p for j, p in enumerate(chunk) if j % 2 == 1],
>         })
>     return banks
> ```

### 5.2 `_faceplate_generic.html` (fallback)

Mismo patrón que TP-Link pero:
- Sin marca específica (logo genérico "Network Device").
- Chasis con color neutro (gris).
- Detección automática de bancos por cada 8 puertos cobre.
- Funciona para cualquier vendor (Cisco, Dell, Datacom, Aruba, Juniper EX, Extreme, Cambium).

### 5.3 `_faceplate_mikrotik.html`

Inspirado en MikroTik CCR/CRS/hEX:
- Chasis `chassis--mikrotik` (azul oscuro/negro).
- Logo "MikroTik" + un símbolo SVG inline del "reloj MikroTik" (puede ser un círculo con 4 radios).
- Puerto de consola serial decorativo a la izquierda (no interactivo).
- LEDs por puerto en fila separada arriba del puerto.
- Distribución horizontal con puertos numerados secuencialmente.

### 5.4 `_faceplate_fortinet.html`

Inspirado en FortiGate 60F/100F:
- Chasis `chassis--fortinet` con stripe rojo inferior (`border-bottom: 3px solid #ee3124`).
- Logo "FORTINET" centrado arriba.
- **Separación visual** entre puertos WAN (2 primeros) y LAN (resto): línea vertical sutil + label.
- Puerto MGMT diferenciado (icono diferente).
- Puerto USB decorativo.
- LEDs de estado en la zona derecha (Status, Alarm, HA, Power) cableados a `device_status`.

### 5.5 `_faceplate_huawei.html`

Inspirado en Huawei S5735/S5720:
- Chasis `chassis--huawei` (gris plomo → negro, distinto a TP-Link).
- Logo "HUAWEI" blanco, lado izquierdo.
- Puertos RJ45 en filas de 12 (no 8) para mejor uso del ancho.
- 4 puertos SFP+ empotrados a la derecha.
- Indicadores PWR1/PWR2, SYS, ALM cableados a `device_health` cuando existan.

### 5.6 `_faceplate_pfsense.html`

Para appliances Netgate (SG-3100, SG-5100) y pfSense/OPNsense genéricos:
- Chasis `chassis--pfsense` (plateado/gris claro — único faceplate claro, los demás son oscuros).
- Logo "pfSense" o "OPNsense" según `device_vendor`.
- Distribución: **2 WAN + N LAN + OPT** con separadores visuales.
- Puertos más espaciados (los appliances son más anchos).
- Menos decoración (son dispositivos de seguridad, no switches con muchos LEDs).

---

## 6. Componente 3 — Modificación del Template Principal

### 6.1 Carga del CSS

En [firewall_detail.html bloque `{% block styles %}`](../../nesshq/snmp/templates/firewalls/firewall_detail.html), después del `<link>` de bootstrap-icons, agregar:
```html
<link rel="stylesheet" href="{% static 'css/faceplates.css' %}">
```

### 6.2 Reemplazo de la sección "Puertos Ethernet"

En [firewall_detail.html líneas 2660-2720](../../nesshq/snmp/templates/firewalls/firewall_detail.html#L2660-L2720), reemplazar el grid de iconos por:

```django
<!-- ═══ Columna 1: Puertos Ethernet (Faceplate Hiperrealista) ═══ -->
<div class="command-card">
    <h3 class="command-card-title">
        <i class="bi bi-ethernet"></i>
        Puertos Ethernet
    </h3>

    {# ── Toggle LANs / VLANs al estilo V0 (sobre el faceplate) ── #}
    {% if vlan_interfaces %}
    <div class="faceplate-toggle">
        <span class="toggle-label {% if not show_vlans %}active{% endif %}"
              onclick="setFaceplateMode(false)">LANs</span>
        <label class="ness-switch small">
            <input type="checkbox" id="faceplateModeToggle" {% if show_vlans %}checked{% endif %}
                   onchange="setFaceplateMode(this.checked)">
            <span class="slider"></span>
        </label>
        <span class="toggle-label {% if show_vlans %}active{% endif %}"
              onclick="setFaceplateMode(true)">VLANs</span>
    </div>
    {% endif %}

    {# ── Vista física: faceplate hiperrealista ── #}
    {% if not show_vlans %}
        {% if copper_ports or fiber_ports %}
            {% include faceplate_template %}
        {% else %}
            <div class="ethernet-empty">No hay puertos físicos reportados por el agente.</div>
        {% endif %}
    {% else %}
        {# ── Vista lógica: VLANs en grid clásico (mantener UX actual) ── #}
        <div class="ethernet-ports-scroll">
            <div class="ethernet-ports-grid">
                {% for port in vlan_interfaces %}
                <div class="ethernet-port {{ port.state }}" title="{{ port.label }}"
                     data-iface-index="{{ port.iface_index }}" data-port-name="{{ port.label }}"
                     data-port-rx="{{ port.rx }}" data-port-tx="{{ port.tx }}" data-port-speed="{{ port.speed }}"
                     onclick="selectPort(this)">
                    <div class="ethernet-port-number">{{ port.number }}</div>
                    <div class="ethernet-port-icon"><i class="bi bi-diagram-3"></i></div>
                    <div class="ethernet-port-name" title="{{ port.label }}">{{ port.label }}</div>
                </div>
                {% endfor %}
            </div>
        </div>
    {% endif %}

    {# ── Leyenda y conteo ── #}
    <div class="faceplate-legend">
        <span class="legend-item"><span class="legend-dot is-up"></span> Activo</span>
        <span class="legend-item"><span class="legend-dot is-down"></span> Inactivo</span>
        {% if copper_ports %}<span class="legend-count">{{ copper_ports|length }} cobre</span>{% endif %}
        {% if fiber_ports %}<span class="legend-count">{{ fiber_ports|length }} fibra</span>{% endif %}
        <span class="ml-auto" id="faceplatePortCount">{{ copper_ports|length|add:fiber_ports|length }} puertos</span>
    </div>
</div>
```

### 6.3 JS auxiliar (incluido en el `<script>` existente)

```javascript
// Toggle entre vista física (faceplate) y vista lógica (VLANs).
window.setFaceplateMode = function (showVlans) {
    // Re-render no es trivial sin HTMX; usamos CSS para mostrar/ocultar.
    document.body.classList.toggle('show-vlans', showVlans);
    const label = document.getElementById('faceplatePortCount');
    if (label) {
        const count = showVlans
            ? {{ vlan_interfaces|length }}
            : {{ copper_ports|length|add:fiber_ports|length }};
        label.textContent = count + ' puertos';
    }
};
```

Y en el CSS:
```css
body.show-vlans .faceplate-physical { display: none; }
body:not(.show-vlans) .faceplate-logical { display: none; }
```

Esto evita recargar la página al alternar.

---

## 7. Componente 4 — Modificación del Backend

### 7.1 Cambios en [firewall_detail.py](../../nesshq/snmp/services/firewalls/firewall_detail.py)

1. **Agregar el registro de templates** (nuevo bloque después de `VENDOR_SECTION_REGISTRY`).
2. **Agregar `_extract_model_code()`** y `_extract_model_label()`.
3. **Agregar `_resolve_faceplate()`**.
4. **Agregar `_chunk_copper_banks()`**.
5. **Extender `_make_port_entry()`** con `media` y `face_label`.
6. **En `firewall_detail_view()`**:
   - Calcular `faceplate_template`, `device_model_code`, `device_model_label`.
   - Calcular `copper_ports`, `fiber_ports` y `copper_banks`.
   - Pasar `show_vlans` desde query string (`?view=vlans`) o session.

### 7.2 Cambios en el modelo [api/models.py:RelayNetworkInterface](../../nesshq/api/models.py)

```python
class RelayNetworkInterface(models.Model):
    # ... campos existentes ...
    media = models.CharField(
        max_length=10,
        choices=[('copper', 'Cobre'), ('fiber', 'Fibra')],
        blank=True, null=True,
        help_text='Tipo de medio físico (cobre/fibra) según ifType SNMP',
    )
```

Migración:
```bash
cd /home/nessuser/nesshq
python manage.py makemigrations api
python manage.py migrate
```

### 7.3 Cambios en el agente [payload_compat.rs](../ness_relay/rust/ness_relay_v2.1.0/src/exporters/payload_compat.rs)

Dentro de `transform_network`, después de construir cada interface, agregar:
```rust
// Clasificar media: ifType 6=ethernetCsmacd, 117=gige → copper;
// 62=fibreChannel, altos (>1Gbps típico) → fiber.
// Fallback por nombre si ifType es 0.
let if_type_val = iface.get("type").and_then(|v| v.as_i64()).unwrap_or(0);
let name_lower = name.to_lowercase();
let media = match if_type_val {
    6 | 117 => "copper",
    62 | 69 | 71 | 151 => "fiber",
    0 if name_lower.contains("te") || name_lower.contains("fo") 
         || name_lower.contains("sf") || name_lower.contains("xg") 
         || name_lower.contains("xe") => "fiber",
    _ if speed_mbps >= 10000 => "fiber",
    _ => "copper",
};
```

Y en el JSON resultante:
```rust
"media": json!(media),
```

> **Compatibilidad hacia atrás:** el campo `media` es opcional. Si el agente antiguo no lo envía, `_classify_media()` en Django hace fallback por nombre (Sección 3.3). La migración es **no destructiva**.

---

## 8. Componente 5 — Sparkline Híbrida (SVG + ECharts)

Reemplazar el chart actual de ECharts (línea ~3060 en [firewall_detail.html](../../nesshq/snmp/templates/firewalls/firewall_detail.html)) por:

- **SVG sparkline estática** (al estilo de [parameters-panel.tsx](../new_design/diseño_hiper_realista/components/parameters-panel.tsx)) — renderizada al cargar, ligera, sin librería.
- **ECharts al hacer hover** sobre la sparkline — el tooltip de ECharts se muestra solo cuando el usuario interactúa, evitando el coste permanente de mantener un chart en memoria.

```html
<div class="perf-sparkline-wrap" id="perfSparklineWrap">
  <svg id="perfSparklineSvg" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden>
    <polygon id="perfSparklineArea" fill="oklch(0.62 0.17 145 / 0.18)"></polygon>
    <polyline id="perfSparklineLine" fill="none" stroke="oklch(0.62 0.17 145)" stroke-width="1.5"
              vector-effect="non-scaling-stroke"></polyline>
  </svg>
  <div id="perfSparklineTooltip" class="perf-tooltip" style="display:none"></div>
</div>
```

JS (dentro del IIFE existente):
```javascript
function renderSparkline() {
  const data = window.overviewMiniPoints || [];
  if (!data.length) return;
  const max = Math.max(...data, 100);
  const points = data.map((v, i) => {
    const x = (i / (data.length - 1)) * 100;
    const y = 100 - (Math.max(0, Math.min(100, v)) / max) * 100;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  }).join(' ');
  document.getElementById('perfSparklineLine').setAttribute('points', points);
  document.getElementById('perfSparklineArea').setAttribute('points', `0,100 ${points} 100,100`);
}

// Lazy ECharts: solo se inicializa en el primer hover
let hoverEChart = null;
document.getElementById('perfSparklineWrap').addEventListener('mouseenter', function() {
  if (hoverEChart || typeof echarts === 'undefined') return;
  // ... inicializar mini ECharts en #perfSparklineTooltip ...
}, { once: true });
```

---

## 9. Plan de Ejecución (Orden)

| # | Tarea | Archivos | Dependencia | Estimación |
|---|---|---|---|---|
| 1 | Crear `faceplates.css` con chasis, RJ45, SFP+, LEDs, leyenda | `nesshq/snmp/static/css/faceplates.css` (NUEVO) | — | 1.5 h |
| 2 | Migración Django: agregar `media` a `RelayNetworkInterface` | `api/models.py` (MODIFY) + migración | — | 0.5 h |
| 3 | Modificar `payload_compat.rs::transform_network` para incluir `media` | `agentes/ness_relay/rust/.../payload_compat.rs` | #2 | 0.5 h |
| 4 | Modificar `_make_port_entry()` y agregar `_classify_media()` | `nesshq/snmp/services/firewalls/firewall_detail.py` | #2 | 0.5 h |
| 5 | Agregar `FACEPLATE_TEMPLATES`, `_extract_model_code`, `_resolve_faceplate`, `_chunk_copper_banks` | `nesshq/snmp/services/firewalls/firewall_detail.py` | — | 1.0 h |
| 6 | Crear `_faceplate_generic.html` (fallback robusto) | `nesshq/snmp/templates/firewalls/faceplates/_faceplate_generic.html` (NUEVO) | #1 | 1.5 h |
| 7 | Crear `_faceplate_tplink.html` (el caso hero) | `nesshq/snmp/templates/firewalls/faceplates/_faceplate_tplink.html` (NUEVO) | #1, #6 | 2.0 h |
| 8 | Modificar `firewall_detail.html`: cargar CSS, reemplazar grid, toggle, leyenda | `nesshq/snmp/templates/firewalls/firewall_detail.html` | #1, #5, #6, #7 | 1.5 h |
| 9 | Smoke test: dispositivo TP-Link real (SW_SRV-MaxMedia) | browser en `http://172.206.0.217:8080/firewall/detail/tp_link/37/` | #1-#8 | 0.5 h |
| 10 | Crear `_faceplate_huawei.html` | `nesshq/snmp/templates/firewalls/faceplates/_faceplate_huawei.html` | #1, #6 | 1.5 h |
| 11 | Crear `_faceplate_mikrotik.html` | `nesshq/snmp/templates/firewalls/faceplates/_faceplate_mikrotik.html` | #1, #6 | 1.5 h |
| 12 | Crear `_faceplate_fortinet.html` | `nesshq/snmp/templates/firewalls/faceplates/_faceplate_fortinet.html` | #1, #6 | 1.5 h |
| 13 | Crear `_faceplate_pfsense.html` | `nesshq/snmp/templates/firewalls/faceplates/_faceplate_pfsense.html` | #1, #6 | 1.5 h |
| 14 | Implementar sparkline híbrida SVG + ECharts-lazy | `nesshq/snmp/templates/firewalls/firewall_detail.html` | #8 | 1.0 h |
| 15 | Recompilar agente Rust + redeploy | `target/release/ness-relay` (release) | #3 | 0.5 h |
| 16 | Pruebas visuales: 4 vendors + responsive 768/1440 | browser | #1-#15 | 1.0 h |
| **Total** | | | | **~16 h** |

---

## 10. Riesgos y Mitigaciones

| Riesgo | Impacto | Mitigación |
|---|---|---|
| El agente actualizado no se redeploya en todos los relays | `media` llega vacío en algunos dispositivos | Heurística de fallback en `_classify_media()` (Sección 3.3). El faceplate sigue funcionando. |
| Un TP-Link con 48 puertos no entra en una sola fila | Layout roto | `_chunk_copper_banks` ya agrupa en bancos; CSS permite `flex-wrap: wrap` a 992px. Si >32 cobre, segunda fila automática. |
| `sys_descr` no incluye el modelo (caso raro) | `device_model_code = None` | Cai al mapa del vendor (`tp_link` → `_faceplate_tplink.html`). Nunca al grid de iconos. |
| El `setFaceplateMode()` recarga mucho | UX lenta | Solo alterna CSS (`body.show-vlans`). No hay fetch ni re-render. |
| El chart ECharts se duplica con la sparkline SVG | Conflicto visual | ECharts se monta **solo en hover** (`{ once: true }`), vive en un div separado `#perfSparklineTooltip`. |
| `oklch()` no soportado en navegador legacy | Colores rotos | Fallback hex en variables (`--faceplate-up-fb`). |
| `payload_compat.rs` cambia y rompe el parser Python | Backend no recibe datos | Tests: verificar que un payload de la versión anterior sigue funcionando (campos `media` ignorados si no existen en el modelo). |

---

## 11. Plan de Verificación

### 11.1 Pruebas Visuales

1. Abrir `http://172.206.0.217:8080/firewall/detail/tp_link/37/` → verificar faceplate TP-Link con 24 cobre (3 bancos × 8) + 4 SFP+ alineados a la derecha. Colores deben coincidir con el boceto de V0.
2. Verificar que los 4 puertos SFP+ (25-28) aparecen como jaulas oscuras (no RJ45).
3. Verificar que el LED de cada puerto cambia de verde a rojo según su estado (UP/DOWN).
4. Verificar el toggle LANs/VLANs: al activar VLANs, el faceplate se oculta y aparece el grid de VLANs (1, 104).
5. Probar con un dispositivo de otro vendor (cambiar el `device_type` en BD manualmente) → debe mostrar el faceplate genérico.
6. Responsive: redimensionar a 768px → el chasis debe envolver correctamente.

### 11.2 Pruebas Funcionales

1. Click en un puerto del faceplate → debe llamar a `selectPort()` y mostrar el chart de tráfico en la tarjeta central.
2. Click en un SFP+ → mismo comportamiento, pero la `selectPort` debe detectar `data-port-name` con sufijo "fiber".
3. Verificar que la API AJAX `firewall_detail_api` sigue devolviendo el mismo JSON (no se rompe la carga inicial).
4. Verificar que la migración es reversible (`migrate api zero` no debe perder datos).

### 11.3 Pruebas de Rendimiento

1. `curl -w "%{time_total}"` sobre la página → debe ser < 2.0s con el nuevo CSS.
2. Verificar que el CSS minificado es < 20 KB.
3. Lighthouse: la página debe mantener su puntaje actual (>= 85 en Performance).

### 11.4 Pruebas del Agente

1. Compilar con `cargo build --release` y comparar tamaño del binario.
2. Ejecutar contra un TP-Link real y verificar que el JSON incluye `media` en cada interface.
3. Ejecutar contra un switch no-TP-Link y verificar el fallback a heurística de nombre.

---

## 12. Out of Scope (v4+)

- **Vista 3D rotable del dispositivo** (Three.js, fuera de alcance).
- **Animaciones de tráfico en tiempo real** sobre los LEDs (puerto parpadeando cuando hay tráfico). Requiere WebSocket o polling agresivo; v3 se queda en estático.
- **Indicadores cableados a métricas reales** (SYS parpadea según CPU, PWR siempre on). Requiere API en tiempo real; v3 los deja siempre encendidos como V0.
- **Soporte para módulos apilables** (stacking TP-Link con 4 switches). Cada instancia mostraría su propio faceplate; v4 podría unificarlos visualmente.
- **Modo oscuro** (el `globals.css` de V0 tiene `.dark`). v3 usa solo la paleta clara del diseño actual de nesshq.
- **PDF report** con el faceplate. La vista `firewall_detail_report_pdf` usa ReportLab; requeriría dibujar el chasis vectorialmente o embeber HTML+CSS via `weasyprint`. v4.

---

## 13. Decisiones Cerradas (confirmadas con el usuario)

| Pregunta | Respuesta |
|---|---|
| ¿Cómo detectar copper vs fiber? | Modificar agente + modelo Django (campo `media` opcional + heurística de respaldo) |
| ¿Qué hacer con vendor no mapeado? | Faceplate genérico (no caer al grid antiguo) |
| ¿Dónde queda el toggle LANs/VLANs? | Conservado, estilo V0 (sobre el faceplate, fuera de `command-card-title`) |
| ¿Sparkline SVG o ECharts? | Implementar ambos: SVG por defecto, ECharts al hacer hover |
| ¿Modelo hero para TP-Link? | T1700G-28TQ (24 cobre + 4 SFP+) — coincide con el switch actualmente monitoreado en el ejemplo |
| ¿Tamaño del binario? | Sin restricción |

---

## 14. Próximos Pasos Inmediatos

1. **Crear la estructura de directorios**:
   ```bash
   mkdir -p /home/nessuser/nesshq/snmp/static/css
   mkdir -p /home/nessuser/nesshq/snmp/static/js
   mkdir -p /home/nessuser/nesshq/snmp/templates/firewalls/faceplates
   ```
2. **Ejecutar migración inicial** del modelo `RelayNetworkInterface`.
3. **Crear `faceplates.css`** (Tarea #1) — base para todos los templates.
4. **Crear `_faceplate_generic.html`** primero (Tarea #6) — valida la integración con el backend.
5. **Crear `_faceplate_tplink.html`** (Tarea #7) y verificar visualmente con SW_SRV-MaxMedia.
6. Iterar sobre los demás vendors en orden descendente de prioridad (Fortinet → pfSense → MikroTik → Huawei).
