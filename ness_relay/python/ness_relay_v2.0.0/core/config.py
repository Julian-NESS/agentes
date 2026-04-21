# ==============================================================================
# NESS Relay v2.0.0 - Configuration
# ==============================================================================
# Constantes globales, rutas, URLs de servidor y carga de configuración
# de dispositivos desde devices.conf y variables de entorno.
# ==============================================================================

import logging
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple
from urllib.parse import urlparse

logger = logging.getLogger("ness_relay")

# ==============================================================================
# VERSIÓN Y TIPO
# ==============================================================================

RELAY_VERSION = "2.0.0"
RELAY_TYPE = "ness-relay-ubuntu"

# ==============================================================================
# DETECCIÓN DE DIRECTORIO BASE
# ==============================================================================
# En el entorno instalado, la estructura es:
#   /opt/ness_relay/               ← INSTALL_DIR (BASE_DIR)
#   ├── configs/devices.conf
#   ├── devices/
#   ├── executables/ness-relay-ubuntu   ← sys.executable
#   ├── logs/
#   └── output/
#
# El binario está en executables/, pero BASE_DIR debe apuntar al directorio
# raíz de la instalación (/opt/ness_relay/).
# ==============================================================================

if getattr(sys, 'frozen', False):
    # Ejecutable PyInstaller
    # 1. Prioridad: variable de entorno NESS_INSTALL_DIR
    _env_install_dir = os.environ.get('NESS_INSTALL_DIR', '')
    if _env_install_dir and Path(_env_install_dir).is_dir():
        BASE_DIR = Path(_env_install_dir)
    else:
        # 2. Fallback: detectar si el ejecutable está dentro de executables/
        _exec_parent = Path(sys.executable).parent
        if _exec_parent.name == 'executables':
            BASE_DIR = _exec_parent.parent
        else:
            BASE_DIR = _exec_parent
else:
    # Ejecución desde fuente: core/config.py → parent.parent = raíz del proyecto
    BASE_DIR = Path(__file__).resolve().parent.parent

# ==============================================================================
# RUTAS DE ARCHIVOS
# ==============================================================================

OUTPUT_DIR = BASE_DIR / "output"
LOG_DIR = BASE_DIR / "logs"
LOG_FILE = LOG_DIR / "ness_relay.log"
JSON_FILE = OUTPUT_DIR / "relay_data.json"
CONFIG_FILE = BASE_DIR / "configs" / "devices.conf"

# Crear directorios si no existen
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
LOG_DIR.mkdir(parents=True, exist_ok=True)

# ==============================================================================
# CONFIGURACIÓN DE SERVIDOR NESS
# ==============================================================================

SERVER_ID = os.environ.get('NESS_SERVER_ID', '3')  # Default: Public Cloud

# IMPORTANTE: Las URLs están hardcodeadas por seguridad.
# El instalador solo maneja IDs (1, 2, 3) sin exponer las rutas reales.
NESS_SERVER_URLS: Dict[str, str] = {
    '1': 'http://172.206.0.217:8080/api/relay/data/',
    '2': 'https://testing.nesshq.com/api/relay/data/',
    '3': 'https://cloud.nesshq.com/api/relay/data/',
}

NESS_SERVER_URL = os.environ.get(
    'NESS_SERVER_URL',
    NESS_SERVER_URLS.get(SERVER_ID, NESS_SERVER_URLS['3'])
)

NESS_API_TOKEN = os.environ.get('NESS_API_TOKEN', '')

# ==============================================================================
# CONFIGURACIÓN DE ACTUALIZACIÓN
# ==============================================================================

# URLs base para el sistema de actualización
HOSTING_BASE_URL = os.environ.get('NESS_HOSTING_URL', 'https://nesshq.com/agents/ness-relay/linux/ubuntu')
VERSION_CHECK_URL = os.environ.get('NESS_VERSION_CHECK_URL', f'{HOSTING_BASE_URL}/version.json')
UPDATE_REPORT_URL = os.environ.get('NESS_UPDATE_REPORT_URL', 'https://nesshq.com/api/report-relay-update/')

# ==============================================================================
# VENDORS SOPORTADOS
# ==============================================================================

SUPPORTED_VENDORS = [
    'pfsense', 'cisco', 'fortinet',
    'mikrotik', 'mikrotik_fw',      # mikrotik = RouterOS, mikrotik_fw = Firewall/Gateway
    'c_n', 'ubnt',
    'linux', 'windows', 'generic',
]

# ==============================================================================
# CARGA DE CONFIGURACIÓN DE DISPOSITIVOS
# ==============================================================================


def load_devices_from_config(config_path: str = 'configs/devices.conf') -> List[Dict[str, Any]]:
    """
    Carga la configuración de dispositivos desde un archivo devices.conf.
    
    Soporta el formato plano key=value generado por el instalador:
    ```
    pfsense_count=1
    pfsense_1_ip=10.0.0.1
    pfsense_1_port=161
    pfsense_1_vendor=pfsense
    pfsense_1_description=Firewall Principal
    pfsense_1_snmp_version=3
    pfsense_1_v3_user=ness_user
    pfsense_1_v3_auth_protocol=SHA
    pfsense_1_v3_auth_password=mypass
    pfsense_1_v3_priv_protocol=AES128
    pfsense_1_v3_priv_password=mypass
    ```
    
    Returns:
        Lista de diccionarios con la configuración de cada dispositivo.
    """
    devices: List[Dict[str, Any]] = []
    
    # Buscar el archivo de configuración
    config_file = Path(config_path)
    if not config_file.is_absolute():
        config_file = BASE_DIR / config_path
    
    if not config_file.exists():
        logger.warning(f"Archivo de configuración no encontrado: {config_file}")
        return devices
    
    logger.info(f"Cargando configuración desde: {config_file}")
    
    # Leer todas las configuraciones en formato plano key=value
    config_data: Dict[str, str] = {}
    try:
        with open(config_file, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                # Ignorar líneas vacías y comentarios
                if not line or line.startswith('#'):
                    continue
                if '=' in line:
                    key, value = line.split('=', 1)
                    config_data[key.strip()] = value.strip()
    except Exception as e:
        logger.error(f"Error leyendo archivo de configuración: {e}")
        return devices
    
    # Procesar dispositivos por vendor
    for vendor in SUPPORTED_VENDORS:
        count_key = f"{vendor}_count"
        if count_key in config_data:
            try:
                count = int(config_data[count_key])
                for i in range(1, count + 1):
                    prefix = f"{vendor}_{i}"
                    snmp_version = config_data.get(f"{prefix}_snmp_version", '2c')
                    
                    device: Dict[str, Any] = {
                        'vendor': config_data.get(f"{prefix}_vendor", vendor),
                        'ip': config_data.get(f"{prefix}_ip", ''),
                        'port': int(config_data.get(f"{prefix}_port", '161')),
                        'description': config_data.get(f"{prefix}_description", ''),
                        'snmp_version': snmp_version,
                    }
                    
                    # Configuración específica según versión SNMP
                    if snmp_version == '3':
                        device.update({
                            'v3_user': config_data.get(f"{prefix}_v3_user", ''),
                            'v3_auth_protocol': config_data.get(f"{prefix}_v3_auth_protocol", 'SHA').upper(),
                            'v3_auth_password': config_data.get(f"{prefix}_v3_auth_password", ''),
                            'v3_priv_protocol': config_data.get(f"{prefix}_v3_priv_protocol", 'AES128').upper(),
                            'v3_priv_password': config_data.get(f"{prefix}_v3_priv_password", ''),
                        })
                        logger.info(
                            f"Dispositivo SNMPv3 cargado: {device['ip']} "
                            f"(usuario: {device['v3_user']})"
                        )
                    else:
                        device['community'] = config_data.get(f"{prefix}_community", 'public')
                        logger.info(
                            f"Dispositivo SNMPv{snmp_version} cargado: {device['ip']} "
                            f"(community: {device['community']})"
                        )
                    
                    if device['ip']:
                        devices.append(device)
                    else:
                        logger.warning(f"IP vacía para [{prefix}], omitiendo dispositivo")
                        
            except Exception as e:
                logger.error(f"Error procesando dispositivos {vendor}: {e}")
    
    logger.info(f"Total de dispositivos cargados: {len(devices)}")
    return devices


def get_snmp_config_from_env() -> Tuple[str, str, int]:
    """
    Obtiene configuración SNMP desde variables de entorno (fallback).
    
    Returns:
        Tupla (host, community, port).
    """
    host = os.environ.get('SNMP_HOST', '').strip()
    community = os.environ.get('SNMP_COMMUNITY', 'public').strip()
    port = int(os.environ.get('SNMP_PORT', '161'))
    
    if not host:
        logger.error("Variable de entorno SNMP_HOST no configurada")
    
    return host, community, port


def get_server_display_name() -> str:
    """Retorna nombre legible del servidor configurado."""
    server_names = {
        '1': 'On-premise',
        '2': 'Testing',
        '3': 'Public Cloud'
    }
    return server_names.get(SERVER_ID, 'Desconocido')
