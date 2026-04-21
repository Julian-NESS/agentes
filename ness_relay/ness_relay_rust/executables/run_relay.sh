#!/bin/bash
#
# ═══════════════════════════════════════════════════════════
# NESS RELAY — Script de Ejecución (Rust Edition)
# Generado automáticamente el mar 24 mar 2026 08:42:48 -05
# ═══════════════════════════════════════════════════════════

# Cargar variables de entorno
source /etc/profile.d/ness_relay.sh

# Cambiar al directorio de instalación
cd /opt/ness_relay

# Colores para mensajes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Detectar si estamos en un terminal interactivo o en cron
if [ -t 1 ]; then
    # Terminal interactivo: mostrar salida en tiempo real
    echo -e "${YELLOW}Ejecutando NESS Relay...${NC}"
    echo ""
    ./executables/ness-relay --config /opt/ness_relay/configs/devices.conf
    EXIT_CODE=$?
    echo ""
    if [ $EXIT_CODE -eq 0 ]; then
        echo -e "${GREEN}✓ Relay ejecutado exitosamente${NC}"
        echo "Log detallado: /opt/ness_relay/logs/ness_relay.log"
    else
        echo -e "${RED}✗ Error en la ejecución del relay (código: $EXIT_CODE)${NC}"
        echo "Revise el log: /opt/ness_relay/logs/ness_relay.log"
        echo ""
        echo "Para ver los últimos errores:"
        echo "  tail -n 50 /opt/ness_relay/logs/ness_relay.log"
    fi
    exit $EXIT_CODE
else
    # Ejecución desde cron: guardar en log
    ./executables/ness-relay --config /opt/ness_relay/configs/devices.conf >> /opt/ness_relay/logs/ness_relay.log 2>&1
    EXIT_CODE=$?
    if [ $EXIT_CODE -ne 0 ]; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: Relay falló con código $EXIT_CODE" >> /opt/ness_relay/logs/ness_relay.log
    fi
    exit $EXIT_CODE
fi
