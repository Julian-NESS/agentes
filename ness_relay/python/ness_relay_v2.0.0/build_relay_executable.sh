#!/bin/bash

###############################################################################
# NESS HQ - Script de Construcción de NESS Relay para Ubuntu Linux (v2.0)
#
# Este script compila un ejecutable autocontenido del NESS Relay que puede
# funcionar en sistemas Ubuntu sin necesidad de tener Python instalado.
#
# IMPORTANTE: Este script usa /tmp/ness_build como directorio de trabajo
#             para evitar problemas con espacios en los paths.
#
# Proceso:
# 1. Limpieza de compilaciones anteriores
# 2. Compilación de OpenSSL 1.1.1w (para compatibilidad SSL/TLS)
# 3. Compilación de Python 3.12.12 con soporte SSL
# 4. Creación del ejecutable con PyInstaller
#
# Uso:
# sudo ./build_relay_executable.sh
#
# Requisitos:
# - Ubuntu 18.04+ o derivado de Debian
# - Conexión a internet
# - Permisos de root (sudo)
#
###############################################################################

set -e  # Salir si hay errores

# Colores y estilos
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Versiones
PYTHON_VERSION="3.12.12"
OPENSSL_VERSION="1.1.1w"

# Archivos
RELAY_SCRIPT="ness_relay.py"
OUTPUT_NAME="ness-relay-ubuntu"

# Módulos internos del proyecto v2.0 (estructura modular)
# Estos directorios se copian al BUILD_DIR para que PyInstaller los encuentre
INTERNAL_PACKAGES=("core" "profiles" "collectors" "analyzers" "exporters" "utils")

# Directorio original (donde está el script)
ORIGINAL_DIR="$(cd "$(dirname "$0")" && pwd)"

# Directorio de trabajo temporal (SIN ESPACIOS - crítico para compilación)
BUILD_DIR="/tmp/ness_relay_build"

# Directorios de instalación personalizados
CUSTOM_OPENSSL_DIR="/usr/local/custom_openssl_relay"
PYTHON_INSTALL_DIR="/usr/local/python312_relay"

# Banner
show_banner() {
    clear
    echo -e "${PURPLE}${BOLD}"
    cat << 'EOF'
╔═════════════════════════════════════════════════════════════════════════════╗
║                                                                             ║
║          ███╗   ██╗███████╗███████╗███████╗    ██████╗ ███████╗██╗   ██╗    ║
║          ████╗  ██║██╔════╝██╔════╝██╔════╝    ██╔══██╗██╔════╝██║   ██║    ║
║          ██╔██╗ ██║█████╗  ███████╗███████╗    ██████╔╝█████╗  ██║   ██║    ║
║          ██║╚██╗██║██╔══╝  ╚════██║╚════██║    ██╔══██╗██╔══╝  ██║   ██║    ║
║          ██║ ╚████║███████╗███████║███████║    ██║  ██║███████╗█████╗██║    ║
║          ╚═╝  ╚═══╝╚══════╝╚══════╝╚══════╝    ╚═╝  ╚═╝╚══════╝╚════╝╚═╝    ║
║                                                                             ║
║              🔧 BUILD SYSTEM - NESS RELAY EXECUTABLE 🔧                     ║
║                        Ubuntu Linux Edition v2.0                            ║
║                                                                             ║
╚═════════════════════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"
    echo -e "${CYAN}Python: ${PYTHON_VERSION} | OpenSSL: ${OPENSSL_VERSION}${NC}"
    echo ""
}

# Verificar si se ejecuta como root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}❌ Por favor, ejecuta este script como root (sudo)${NC}"
    exit 1
fi

# Función de logging
log_message() {
    local level=$1
    local message=$2
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    case $level in
        "INFO")
            echo -e "${BLUE}ℹ️  [${timestamp}] ${message}${NC}"
            ;;
        "SUCCESS")
            echo -e "${GREEN}✅ [${timestamp}] ${message}${NC}"
            ;;
        "WARNING")
            echo -e "${YELLOW}⚠️  [${timestamp}] ${message}${NC}"
            ;;
        "ERROR")
            echo -e "${RED}❌ [${timestamp}] ${message}${NC}"
            ;;
        "PROGRESS")
            echo -e "${PURPLE}⚙️  [${timestamp}] ${message}${NC}"
            ;;
        "STEP")
            echo -e "${CYAN}📋 [${timestamp}] ${message}${NC}"
            ;;
    esac
}

# Función para verificar conectividad
check_network() {
    log_message "INFO" "Verificando conectividad de red..."
    if ! ping -c 1 8.8.8.8 &> /dev/null; then
        log_message "ERROR" "No hay conexión a internet. Verifica tu conexión de red."
        exit 1
    fi
    log_message "SUCCESS" "Conectividad de red verificada."
}

# Función para detectar la distribución
detect_distro() {
    log_message "INFO" "Detectando sistema operativo..."
    
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        DISTRO_NAME="$NAME"
        DISTRO_VERSION="$VERSION_ID"
        DISTRO_ID="$ID"
    else
        DISTRO_NAME="Unknown"
        DISTRO_VERSION="Unknown"
        DISTRO_ID="unknown"
    fi
    
    log_message "INFO" "Sistema detectado: $DISTRO_NAME $DISTRO_VERSION"
    
    # Verificar que sea una distribución compatible (Debian/Ubuntu)
    case "$DISTRO_ID" in
        ubuntu|debian|linuxmint|pop)
            log_message "SUCCESS" "Distribución compatible detectada: $DISTRO_ID"
            ;;
        *)
            log_message "WARNING" "Distribución no verificada: $DISTRO_ID"
            log_message "WARNING" "Este script está optimizado para Ubuntu/Debian. Continuando..."
            ;;
    esac
}

# Función de limpieza al salir
cleanup_on_exit() {
    log_message "INFO" "Limpiando recursos temporales..."
    # No eliminamos BUILD_DIR para poder inspeccionar en caso de error
}

trap cleanup_on_exit EXIT

# Mostrar banner
show_banner

# === PASO 0: PREPARACIÓN DEL ENTORNO ===
log_message "STEP" "═══════════════════════════════════════════════════════════"
log_message "STEP" "PASO 0: Preparando entorno de compilación..."
log_message "STEP" "═══════════════════════════════════════════════════════════"

# Verificar que existe el script del relay en el directorio original
if [ ! -f "${ORIGINAL_DIR}/${RELAY_SCRIPT}" ]; then
    log_message "ERROR" "No se encontró el archivo '${RELAY_SCRIPT}' en ${ORIGINAL_DIR}"
    log_message "ERROR" "Asegúrate de ejecutar este script desde el directorio donde está el relay."
    exit 1
fi
log_message "SUCCESS" "Entry point del relay encontrado: ${ORIGINAL_DIR}/${RELAY_SCRIPT}"

# Verificar que existen los paquetes internos del proyecto v2.0
log_message "PROGRESS" "Verificando estructura modular del proyecto v2.0..."
MISSING_PACKAGES=0
for pkg in "${INTERNAL_PACKAGES[@]}"; do
    if [ ! -d "${ORIGINAL_DIR}/${pkg}" ]; then
        log_message "ERROR" "Paquete '${pkg}/' no encontrado en ${ORIGINAL_DIR}"
        MISSING_PACKAGES=$((MISSING_PACKAGES + 1))
    else
        # Contar archivos .py en el paquete (recursivamente)
        PY_COUNT=$(find "${ORIGINAL_DIR}/${pkg}" -name "*.py" | wc -l)
        log_message "INFO" "  ✓ ${pkg}/ (${PY_COUNT} archivos Python)"
    fi
done

if [ $MISSING_PACKAGES -gt 0 ]; then
    log_message "ERROR" "Faltan ${MISSING_PACKAGES} paquete(s). Verifica la estructura del proyecto v2.0."
    exit 1
fi
log_message "SUCCESS" "Estructura modular v2.0 verificada correctamente."

# Crear directorio de trabajo limpio (sin espacios)
log_message "PROGRESS" "Creando directorio de trabajo en ${BUILD_DIR}..."
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

# Copiar el entry point y todos los paquetes internos al directorio de trabajo
log_message "PROGRESS" "Copiando proyecto v2.0 al directorio de trabajo..."
cp "${ORIGINAL_DIR}/${RELAY_SCRIPT}" "${BUILD_DIR}/"

for pkg in "${INTERNAL_PACKAGES[@]}"; do
    cp -r "${ORIGINAL_DIR}/${pkg}" "${BUILD_DIR}/"
    log_message "INFO" "  → Copiado: ${pkg}/"
done

log_message "SUCCESS" "Proyecto v2.0 copiado a directorio de trabajo."

# Cambiar al directorio de trabajo
cd "${BUILD_DIR}"
log_message "INFO" "Directorio de trabajo: $(pwd)"
echo ""

# === PASO 1: VERIFICACIONES PREVIAS ===
log_message "STEP" "═══════════════════════════════════════════════════════════"
log_message "STEP" "PASO 1: Verificaciones previas..."
log_message "STEP" "═══════════════════════════════════════════════════════════"

# Verificar conectividad
check_network

# Detectar distribución
detect_distro
echo ""

# === PASO 2: INSTALACIÓN DE DEPENDENCIAS DEL SISTEMA ===
log_message "STEP" "═══════════════════════════════════════════════════════════"
log_message "STEP" "PASO 2: Instalando dependencias del sistema..."
log_message "STEP" "═══════════════════════════════════════════════════════════"

log_message "PROGRESS" "Actualizando lista de paquetes..."
apt-get update -qq

log_message "PROGRESS" "Instalando herramientas de compilación y dependencias..."
apt-get install -y \
    build-essential \
    wget \
    curl \
    tar \
    zlib1g-dev \
    libncurses5-dev \
    libgdbm-dev \
    libnss3-dev \
    libreadline-dev \
    libffi-dev \
    libsqlite3-dev \
    libbz2-dev \
    liblzma-dev \
    uuid-dev \
    libgdbm-compat-dev \
    tk-dev \
    perl \
    ca-certificates

log_message "SUCCESS" "Dependencias del sistema instaladas correctamente."
echo ""

# === PASO 3: COMPILACIÓN DE OPENSSL ===
log_message "STEP" "═══════════════════════════════════════════════════════════"
log_message "STEP" "PASO 3: Compilando OpenSSL ${OPENSSL_VERSION}..."
log_message "STEP" "═══════════════════════════════════════════════════════════"

# Descargar OpenSSL
log_message "PROGRESS" "Descargando OpenSSL ${OPENSSL_VERSION}..."
wget -q --show-progress "https://www.openssl.org/source/openssl-${OPENSSL_VERSION}.tar.gz"

if [ $? -ne 0 ]; then
    log_message "ERROR" "Error al descargar OpenSSL. Verifica tu conexión."
    exit 1
fi

log_message "PROGRESS" "Extrayendo OpenSSL..."
tar -xf "openssl-${OPENSSL_VERSION}.tar.gz"

cd "openssl-${OPENSSL_VERSION}"

log_message "PROGRESS" "Configurando OpenSSL (compilación compartida)..."
./config shared --prefix=${CUSTOM_OPENSSL_DIR} --openssldir=${CUSTOM_OPENSSL_DIR}

log_message "PROGRESS" "Compilando OpenSSL (esto puede tomar varios minutos)..."
make -j$(nproc)

log_message "PROGRESS" "Instalando OpenSSL en ${CUSTOM_OPENSSL_DIR}..."
make install_sw

cd "${BUILD_DIR}"

# Registrar las bibliotecas de OpenSSL en el sistema ANTES de verificar
log_message "PROGRESS" "Registrando bibliotecas de OpenSSL en el sistema..."
echo "${CUSTOM_OPENSSL_DIR}/lib" > /etc/ld.so.conf.d/custom_openssl_relay.conf
ldconfig

# Verificar instalación de OpenSSL
if [ -f "${CUSTOM_OPENSSL_DIR}/bin/openssl" ]; then
    # Usar LD_LIBRARY_PATH como respaldo por si ldconfig no es suficiente
    OPENSSL_COMPILED_VERSION=$(LD_LIBRARY_PATH="${CUSTOM_OPENSSL_DIR}/lib" ${CUSTOM_OPENSSL_DIR}/bin/openssl version 2>&1)
    if [[ "$OPENSSL_COMPILED_VERSION" == *"OpenSSL"* ]]; then
        log_message "SUCCESS" "OpenSSL compilado exitosamente: $OPENSSL_COMPILED_VERSION"
    else
        log_message "ERROR" "OpenSSL instalado pero no funciona: $OPENSSL_COMPILED_VERSION"
        exit 1
    fi
else
    log_message "ERROR" "La compilación de OpenSSL falló. No se encontró el binario."
    exit 1
fi
echo ""

# === PASO 4: COMPILACIÓN DE PYTHON ===
log_message "STEP" "═══════════════════════════════════════════════════════════"
log_message "STEP" "PASO 4: Compilando Python ${PYTHON_VERSION}..."
log_message "STEP" "═══════════════════════════════════════════════════════════"

# Descargar Python
log_message "PROGRESS" "Descargando Python ${PYTHON_VERSION}..."
wget -q --show-progress "https://www.python.org/ftp/python/${PYTHON_VERSION}/Python-${PYTHON_VERSION}.tgz"

if [ $? -ne 0 ]; then
    log_message "ERROR" "Error al descargar Python. Verifica tu conexión."
    exit 1
fi

log_message "PROGRESS" "Extrayendo Python..."
tar -xf "Python-${PYTHON_VERSION}.tgz"

cd "Python-${PYTHON_VERSION}"

# Configurar variables de entorno para que Python use nuestro OpenSSL
log_message "PROGRESS" "Configurando Python con OpenSSL personalizado..."
export CFLAGS="-I${CUSTOM_OPENSSL_DIR}/include"
export LDFLAGS="-L${CUSTOM_OPENSSL_DIR}/lib -Wl,-rpath,${CUSTOM_OPENSSL_DIR}/lib"
export LD_LIBRARY_PATH="${CUSTOM_OPENSSL_DIR}/lib"

# Configurar Python SIN optimizaciones PGO (para evitar el bug con espacios/paths)
# --enable-optimizations causa problemas, lo omitimos
./configure \
    --prefix=${PYTHON_INSTALL_DIR} \
    --with-openssl=${CUSTOM_OPENSSL_DIR} \
    --enable-shared \
    --with-ensurepip=install

log_message "PROGRESS" "Compilando Python (esto puede tomar 5-10 minutos)..."
make -j$(nproc)

# Instalar Python
log_message "PROGRESS" "Instalando Python..."
make altinstall

# Actualizar la caché de librerías para Python
log_message "PROGRESS" "Registrando bibliotecas de Python en el sistema..."
echo "${PYTHON_INSTALL_DIR}/lib" > /etc/ld.so.conf.d/python312-relay.conf
ldconfig

cd "${BUILD_DIR}"

# Verificar instalación de Python
PYTHON_BIN="${PYTHON_INSTALL_DIR}/bin/python3.12"

if [ ! -f "${PYTHON_BIN}" ]; then
    log_message "ERROR" "La instalación de Python 3.12 falló. No se encontró ${PYTHON_BIN}"
    exit 1
fi

# Verificar que Python funciona
log_message "PROGRESS" "Verificando instalación de Python..."
PYTHON_TEST=$("${PYTHON_BIN}" --version 2>&1)
if [[ "$PYTHON_TEST" == *"Python 3.12"* ]]; then
    log_message "SUCCESS" "Python compilado exitosamente: $PYTHON_TEST"
else
    log_message "ERROR" "Python no funciona correctamente: $PYTHON_TEST"
    exit 1
fi

# Verificar soporte SSL
log_message "PROGRESS" "Verificando soporte SSL en Python..."
SSL_CHECK=$("${PYTHON_BIN}" -c "import ssl; print(ssl.OPENSSL_VERSION)" 2>&1)
if [[ "$SSL_CHECK" == *"OpenSSL"* ]]; then
    log_message "SUCCESS" "Soporte SSL verificado: $SSL_CHECK"
else
    log_message "WARNING" "Posible problema con SSL: $SSL_CHECK"
fi

unset CFLAGS LDFLAGS LD_LIBRARY_PATH
echo ""

# === PASO 5: CREACIÓN DEL ENTORNO VIRTUAL E INSTALACIÓN DE DEPENDENCIAS ===
log_message "STEP" "═══════════════════════════════════════════════════════════"
log_message "STEP" "PASO 5: Creando entorno virtual e instalando dependencias..."
log_message "STEP" "═══════════════════════════════════════════════════════════"

cd "${BUILD_DIR}"

log_message "PROGRESS" "Creando entorno virtual..."
"${PYTHON_BIN}" -m venv build_env

if [ ! -f "build_env/bin/activate" ]; then
    log_message "ERROR" "No se pudo crear el entorno virtual."
    exit 1
fi

log_message "PROGRESS" "Activando entorno virtual..."
source build_env/bin/activate

# Verificar que estamos usando el Python correcto
VENV_PYTHON=$(which python)
log_message "INFO" "Python del entorno virtual: ${VENV_PYTHON}"

log_message "PROGRESS" "Actualizando pip y setuptools..."
python -m pip install --upgrade pip setuptools wheel

log_message "PROGRESS" "Instalando PyInstaller (versión actualizada Feb 2026)..."
# PyInstaller 6.11+ tiene mejor compatibilidad con importlib.metadata
pip install "pyinstaller>=6.11.0"

log_message "PROGRESS" "Instalando bibliotecas de cifrado para SNMPv3 PRIMERO..."
# CRÍTICO: Instalar pycryptodome ANTES de pysnmp para que lo detecte durante la instalación
pip install pycryptodome==3.23.0

log_message "PROGRESS" "Instalando pysnmpcrypto (integración crypto para pysnmp)..."
# Este paquete proporciona la integración entre pysnmp y pycryptodome
pip install pysnmpcrypto

log_message "PROGRESS" "Instalando pysnmp v7.1.22 (versión exacta probada)..."
# Usamos la versión exacta que fue probada y funciona
pip install pysnmp==7.1.22

log_message "PROGRESS" "Instalando requests..."
pip install "requests>=2.31.0"

log_message "PROGRESS" "Instalando dependencias modernas (sin pkg_resources)..."
# Versiones actualizadas que usan importlib.metadata nativo de Python 3.8+
# Estas versiones ya no dependen de pkg_resources
pip install \
    "jaraco.text>=3.14.0" \
    "jaraco.functools>=4.1.0" \
    "jaraco.context>=6.0.0" \
    "platformdirs>=4.3.0" \
    "importlib-metadata>=8.0.0" \
    "packaging>=24.0"

# Verificar que pysnmp se instaló correctamente
log_message "PROGRESS" "Verificando instalación de pysnmp..."
python -c "import pysnmp; print(f'pysnmp version: {pysnmp.__version__}')" || {
    log_message "ERROR" "pysnmp no se instaló correctamente"
    deactivate
    exit 1
}

# Verificar que las bibliotecas de cifrado están disponibles para SNMPv3
log_message "PROGRESS" "Verificando bibliotecas de cifrado para SNMPv3..."
python -c "from Crypto.Cipher import AES, DES; from Crypto.Hash import MD5, SHA; print('Crypto libraries OK')" || {
    log_message "ERROR" "pycryptodome no se instaló correctamente. Es requerido para SNMPv3."
    deactivate
    exit 1
}

# Verificar que pysnmpcrypto está instalado
log_message "PROGRESS" "Verificando pysnmpcrypto (integración pysnmp+crypto)..."
python -c "import pysnmpcrypto; print(f'pysnmpcrypto OK')" || {
    log_message "WARNING" "pysnmpcrypto no encontrado, intentando instalar de nuevo..."
    pip install pysnmpcrypto
}

# Verificar que pysnmp puede usar las funciones de cifrado
log_message "PROGRESS" "Verificando integración pysnmp + crypto..."
python -c "
from pysnmp.hlapi.v3arch.asyncio import UsmUserData, usmHMACSHAAuthProtocol, usmAesCfb128Protocol
# Crear credenciales de prueba para verificar que el cifrado funciona
user = UsmUserData('testuser', 'testauth123456', 'testpriv123456',
                   authProtocol=usmHMACSHAAuthProtocol,
                   privProtocol=usmAesCfb128Protocol)
print('pysnmp v7 crypto integration OK')
" || {
    log_message "WARNING" "pysnmp crypto integration check failed, but continuing..."
}

log_message "SUCCESS" "Todas las dependencias instaladas correctamente."
echo ""

# === PASO 6: GENERACIÓN DEL EJECUTABLE ===
log_message "STEP" "═══════════════════════════════════════════════════════════"
log_message "STEP" "PASO 6: Generando ejecutable con PyInstaller..."
log_message "STEP" "═══════════════════════════════════════════════════════════"

log_message "PROGRESS" "Ejecutando PyInstaller con hidden imports para proyecto modular v2.0..."

# Copiar hooks personalizados al directorio de trabajo
if [ -d "${ORIGINAL_DIR}/hooks" ]; then
    log_message "PROGRESS" "Copiando hooks personalizados..."
    cp -r "${ORIGINAL_DIR}/hooks" "${BUILD_DIR}/"
fi

# Verificar que los paquetes internos están disponibles en BUILD_DIR
log_message "PROGRESS" "Verificando paquetes internos en directorio de build..."
for pkg in "${INTERNAL_PACKAGES[@]}"; do
    if [ ! -d "${BUILD_DIR}/${pkg}" ]; then
        log_message "ERROR" "Paquete '${pkg}/' no encontrado en ${BUILD_DIR}. Abortando."
        deactivate
        exit 1
    fi
done
log_message "SUCCESS" "Todos los paquetes internos disponibles en BUILD_DIR."

# ═══════════════════════════════════════════════════════════════════════════
# PyInstaller v2.0 — Estructura modular multi-paquete
#
# IMPORTANTE: El proyecto v2.0 tiene esta estructura de paquetes internos:
#   ness_relay.py               → Entry point
#   core/                       → Motor, SNMP client, config, logging, updater
#   profiles/                   → Perfiles de vendor (pfSense, Cisco, etc.)
#   profiles/vendors/           → Implementaciones: pfsense, cisco, fortinet,
#                                  mikrotik, mikrotik_fw, ubnt, c_n
#   profiles/device_types/      → Tipos de dispositivo (firewall, router, etc.)
#   collectors/                 → Recolectores de datos SNMP
#   analyzers/                  → Analizadores de seguridad y rendimiento
#   exporters/                  → Exportación JSON y envío al servidor
#   utils/                      → Utilidades (crypto, conversiones, helpers)
#
# PyInstaller necesita --hidden-import para CADA módulo interno porque al
# ser un proyecto empaquetado como --onefile, no descubre automáticamente
# los imports relativos entre paquetes.
#
# NOTA: Usamos un array bash para poder documentar cada sección con
#       comentarios sin interferir con la ejecución del comando.
# ═══════════════════════════════════════════════════════════════════════════

# Construir argumentos de PyInstaller en un array
PYINSTALLER_ARGS=(
    --onefile
    --name "${OUTPUT_NAME}"
    --clean
    --noconfirm
    --noupx
    --additional-hooks-dir=./hooks

    # ─────────────────────────────────────────────────────────────────────
    # MÓDULOS INTERNOS DEL PROYECTO v2.0 (NESS Relay Multi-Vendor)
    # ─────────────────────────────────────────────────────────────────────

    # --- utils/ --- Utilidades base (crypto, conversiones, helpers)
    --hidden-import=utils
    --hidden-import=utils.crypto_init
    --hidden-import=utils.conversions
    --hidden-import=utils.helpers

    # --- core/ --- Motor principal, SNMP client, configuración
    --hidden-import=core
    --hidden-import=core.config
    --hidden-import=core.logging_setup
    --hidden-import=core.snmp_client
    --hidden-import=core.updater
    --hidden-import=core.engine

    # --- profiles/ --- Sistema de perfiles de dispositivo
    --hidden-import=profiles
    --hidden-import=profiles.base_profile
    --hidden-import=profiles.profile_loader
    --hidden-import=profiles.standard_oids

    # --- profiles.vendors/ --- Perfiles por fabricante
    --hidden-import=profiles.vendors
    --hidden-import=profiles.vendors.pfsense
    --hidden-import=profiles.vendors.cisco
    --hidden-import=profiles.vendors.fortinet
    --hidden-import=profiles.vendors.mikrotik
    --hidden-import=profiles.vendors.mikrotik_fw
    --hidden-import=profiles.vendors.ubnt
    --hidden-import=profiles.vendors.c_n

    # --- profiles.device_types/ --- Tipos de dispositivo
    --hidden-import=profiles.device_types
    --hidden-import=profiles.device_types.firewall
    --hidden-import=profiles.device_types.router
    --hidden-import=profiles.device_types.switch
    --hidden-import=profiles.device_types.access_point

    # --- collectors/ --- Recolectores de datos SNMP
    --hidden-import=collectors
    --hidden-import=collectors.system_collector
    --hidden-import=collectors.performance_collector
    --hidden-import=collectors.network_collector
    --hidden-import=collectors.security_collector
    --hidden-import=collectors.vendor_collector

    # --- analyzers/ --- Analizadores de alertas
    --hidden-import=analyzers
    --hidden-import=analyzers.security_analyzer
    --hidden-import=analyzers.performance_analyzer

    # --- exporters/ --- Exportación y envío de datos
    --hidden-import=exporters
    --hidden-import=exporters.json_exporter
    --hidden-import=exporters.server_sender

    # ─────────────────────────────────────────────────────────────────────
    # DEPENDENCIAS EXTERNAS — pysnmp y ecosistema SNMP
    # ─────────────────────────────────────────────────────────────────────

    --collect-all pysnmp
    --collect-all pyasn1
    --collect-all pysmi
    --collect-all pysnmpcrypto
    --hidden-import=pysnmpcrypto
    --hidden-import=pysnmpcrypto.aes
    --hidden-import=pysnmpcrypto.des
    --hidden-import=pysnmpcrypto.des3
    --hidden-import=pysnmp
    --hidden-import=pysnmp.hlapi
    --hidden-import=pysnmp.hlapi.v3arch
    --hidden-import=pysnmp.hlapi.v3arch.asyncio
    --hidden-import=pysnmp.hlapi.v3arch.auth
    --hidden-import=pysnmp.hlapi.v3arch.lcd
    --hidden-import=pysnmp.smi
    --hidden-import=pysnmp.smi.builder
    --hidden-import=pysnmp.carrier
    --hidden-import=pysnmp.carrier.asyncio
    --hidden-import=pysnmp.carrier.asyncio.dgram
    --hidden-import=pysnmp.carrier.asyncio.dgram.udp
    --hidden-import=pysnmp.entity
    --hidden-import=pysnmp.entity.rfc3413
    --hidden-import=pysnmp.proto
    --hidden-import=pysnmp.proto.rfc1902
    --hidden-import=pysnmp.proto.rfc1905
    --hidden-import=pysnmp.proto.api
    --hidden-import=pysnmp.proto.secmod
    --hidden-import=pysnmp.proto.secmod.rfc3414
    --hidden-import=pysnmp.proto.secmod.rfc3414.auth
    --hidden-import=pysnmp.proto.secmod.rfc3414.auth.hmacmd5
    --hidden-import=pysnmp.proto.secmod.rfc3414.auth.hmacsha
    --hidden-import=pysnmp.proto.secmod.rfc3414.priv
    --hidden-import=pysnmp.proto.secmod.rfc3414.priv.des
    --hidden-import=pysnmp.proto.secmod.rfc3826
    --hidden-import=pysnmp.proto.secmod.rfc3826.priv
    --hidden-import=pysnmp.proto.secmod.rfc3826.priv.aes
    --hidden-import=pysnmp.proto.secmod.rfc7860
    --hidden-import=pysnmp.proto.secmod.rfc7860.auth
    --hidden-import=pyasn1
    --hidden-import=pyasn1.type
    --hidden-import=pyasn1.type.univ
    --hidden-import=pyasn1.codec
    --hidden-import=pyasn1.codec.ber
    --hidden-import=pyasn1.codec.der
    --hidden-import=pysmi
    --hidden-import=pysmi.reader
    --hidden-import=pysmi.parser
    --hidden-import=pysmi.codegen

    # ─────────────────────────────────────────────────────────────────────
    # DEPENDENCIAS EXTERNAS — pycryptodome (cifrado SNMPv3)
    # ─────────────────────────────────────────────────────────────────────

    --collect-all Crypto
    --hidden-import=Crypto
    --hidden-import=Crypto.Cipher
    --hidden-import=Crypto.Cipher.AES
    --hidden-import=Crypto.Cipher.DES
    --hidden-import=Crypto.Cipher.DES3
    --hidden-import=Crypto.Cipher._mode_cfb
    --hidden-import=Crypto.Cipher._mode_cbc
    --hidden-import=Crypto.Cipher._mode_ecb
    --hidden-import=Crypto.Cipher._raw_aes
    --hidden-import=Crypto.Cipher._raw_des
    --hidden-import=Crypto.Cipher._raw_des3
    --hidden-import=Crypto.Hash
    --hidden-import=Crypto.Hash.MD5
    --hidden-import=Crypto.Hash.SHA
    --hidden-import=Crypto.Hash.SHA1
    --hidden-import=Crypto.Hash.SHA256
    --hidden-import=Crypto.Hash.SHA384
    --hidden-import=Crypto.Hash.SHA512
    --hidden-import=Crypto.Hash.HMAC
    --hidden-import=Crypto.Hash.CMAC
    --hidden-import=Crypto.Hash._SHA1
    --hidden-import=Crypto.Hash._MD5
    --hidden-import=Crypto.Random
    --hidden-import=Crypto.Random._UserFriendlyRNG
    --hidden-import=Crypto.Util
    --hidden-import=Crypto.Util._raw_api
    --hidden-import=Crypto.Util.strxor
    --hidden-import=Crypto.Util._cpuid_c
    --hidden-import=Crypto.Util.Padding
    --hidden-import=Crypto.Protocol
    --hidden-import=Crypto.Protocol.KDF

    # ─────────────────────────────────────────────────────────────────────
    # DEPENDENCIAS EXTERNAS — Utilidades de sistema
    # ─────────────────────────────────────────────────────────────────────

    --collect-all hashlib
    --collect-all hmac
    --collect-all jaraco.text
    --collect-all jaraco.functools
    --collect-all jaraco.context
    --hidden-import=jaraco
    --hidden-import=jaraco.text
    --hidden-import=jaraco.functools
    --hidden-import=jaraco.context
    --hidden-import=pkg_resources
    --hidden-import=pkg_resources.extern
    --hidden-import=setuptools
    --hidden-import=platformdirs
    --hidden-import=zipp
    --hidden-import=importlib_metadata
    --collect-all platformdirs
    --collect-all zipp
    --collect-all importlib_metadata
    --collect-all setuptools
    --collect-all pkg_resources
)

# Ejecutar PyInstaller con todos los argumentos
log_message "PROGRESS" "Ejecutando PyInstaller (esto puede tomar varios minutos)..."
pyinstaller "${PYINSTALLER_ARGS[@]}" "${RELAY_SCRIPT}"

# Desactivar entorno virtual
deactivate

log_message "SUCCESS" "PyInstaller completado."
echo ""

# === PASO 7: COPIAR EJECUTABLE AL DIRECTORIO ORIGINAL ===
log_message "STEP" "═══════════════════════════════════════════════════════════"
log_message "STEP" "PASO 7: Finalizando..."
log_message "STEP" "═══════════════════════════════════════════════════════════"

if [ -f "dist/${OUTPUT_NAME}" ]; then
    # Crear directorio dist en el directorio original si no existe
    mkdir -p "${ORIGINAL_DIR}/dist"
    
    # Copiar el ejecutable al directorio original
    cp "dist/${OUTPUT_NAME}" "${ORIGINAL_DIR}/dist/"
    chmod +x "${ORIGINAL_DIR}/dist/${OUTPUT_NAME}"
    
    # Obtener información del ejecutable
    EXEC_SIZE=$(du -h "${ORIGINAL_DIR}/dist/${OUTPUT_NAME}" | cut -f1)
    EXEC_PATH="${ORIGINAL_DIR}/dist/${OUTPUT_NAME}"
    
    echo ""
    echo -e "${GREEN}${BOLD}╔═════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}${BOLD}║                                                                 ║${NC}"
    echo -e "${GREEN}${BOLD}║     ✅ ¡ÉXITO! EJECUTABLE CREADO CORRECTAMENTE ✅              ║${NC}"
    echo -e "${GREEN}${BOLD}║                                                                 ║${NC}"
    echo -e "${GREEN}${BOLD}╚═════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${CYAN}📦 Información del ejecutable:${NC}"
    echo -e "   ${BLUE}Nombre:${NC}    ${OUTPUT_NAME}"
    echo -e "   ${BLUE}Ubicación:${NC} ${EXEC_PATH}"
    echo -e "   ${BLUE}Tamaño:${NC}    ${EXEC_SIZE}"
    echo -e "   ${BLUE}Python:${NC}    ${PYTHON_VERSION}"
    echo -e "   ${BLUE}OpenSSL:${NC}   ${OPENSSL_VERSION}"
    echo -e "   ${BLUE}Versión:${NC}   NESS Relay v2.0.0 Multi-Vendor"
    echo -e "   ${BLUE}Paquetes:${NC}  core, profiles, collectors, analyzers, exporters, utils"
    echo -e "   ${BLUE}Vendors:${NC}   pfSense, Fortinet, Cisco, MikroTik (RouterOS + Firewall), UBNT, Cambium"
    echo ""
    echo -e "${YELLOW}📋 Próximos pasos:${NC}"
    echo -e "   1. Prueba el ejecutable:"
    echo -e "      ${CYAN}${EXEC_PATH} --help${NC}"
    echo ""
    echo -e "   2. Copia el ejecutable al servidor destino:"
    echo -e "      ${CYAN}scp ${EXEC_PATH} usuario@servidor:/ruta/destino/${NC}"
    echo ""
    echo -e "   3. En el servidor destino, dale permisos de ejecución:"
    echo -e "      ${CYAN}chmod +x ${OUTPUT_NAME}${NC}"
    echo ""
    echo -e "${GREEN}🎉 ¡El ejecutable está listo para su distribución!${NC}"
    echo -e "${GREEN}   No requiere Python instalado en el sistema destino.${NC}"
    echo ""
    
    # Limpiar archivos temporales (opcional)
    log_message "PROGRESS" "Limpiando archivos temporales..."
    rm -rf "${BUILD_DIR}"
    
    # Limpiar configuración de librerías temporales
    rm -f /etc/ld.so.conf.d/python312-relay.conf
    rm -f /etc/ld.so.conf.d/custom_openssl_relay.conf
    ldconfig
    
    log_message "SUCCESS" "¡Proceso completado exitosamente!"
    
else
    echo ""
    echo -e "${RED}${BOLD}╔═════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}${BOLD}║                                                                 ║${NC}"
    echo -e "${RED}${BOLD}║     ❌ ERROR: NO SE PUDO CREAR EL EJECUTABLE ❌                ║${NC}"
    echo -e "${RED}${BOLD}║                                                                 ║${NC}"
    echo -e "${RED}${BOLD}╚═════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${YELLOW}Posibles causas:${NC}"
    echo -e "   - Error durante la compilación de PyInstaller"
    echo -e "   - Dependencias faltantes en el script Python"
    echo -e "   - Problemas de permisos"
    echo ""
    echo -e "${YELLOW}Revisa los logs en: ${BUILD_DIR}/build/${NC}"
    echo ""
    exit 1
fi