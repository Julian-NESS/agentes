#!/bin/bash
#
# ═══════════════════════════════════════════════════════════
# NESS RELAY — Visor Seguro de Configuración
# Este script protege el acceso al archivo devices.conf
# ═══════════════════════════════════════════════════════════

CONFIG_FILE='/opt/ness_relay/configs/devices.conf'
ENV_FILE='/etc/profile.d/ness_relay.sh'

# Colores
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m'

# Verificar que el archivo de configuración existe
if [[ ! -f "$CONFIG_FILE" ]]; then
    echo -e "${RED}❌ Error: Archivo de configuración no encontrado.${NC}"
    exit 1
fi

# Banner de acceso
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║       🔐  NESS RELAY — Acceso Protegido              ║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${YELLOW}Este archivo contiene información sensible de los dispositivos.${NC}"
echo -e "${YELLOW}Se requiere autenticación para acceder.${NC}"
echo ""

# Solicitar contraseña
read -sp 'Ingrese la contraseña de acceso: ' INPUT_PASSWORD
echo ""
echo ""

# Obtener el token real del sistema
if [[ -f "$ENV_FILE" ]]; then
    source "$ENV_FILE"
    STORED_TOKEN="$NESS_API_TOKEN"
else
    echo -e "${RED}❌ Error: No se pudo verificar las credenciales.${NC}"
    exit 1
fi

# Verificar contraseña
if [[ "$INPUT_PASSWORD" == "$STORED_TOKEN" ]]; then
    echo -e "${GREEN}✓ Contraseña correcta. Acceso concedido.${NC}"
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════${NC}"
    cat "$CONFIG_FILE"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════${NC}"
else
    echo -e "${RED}❌ Contraseña incorrecta. Acceso denegado.${NC}"
    exit 1
fi
