#!/bin/bash

###############################################################################
# NESS HQ - RELAY Network Monitoring System v2.0.0
# Script de instalación profesional para NESS Relay - Versión Ejecutable
#
# CARACTERÍSTICAS:
# - NO requiere instalar Python en el sistema
# - Ejecutable autocontenido optimizado
# - Monitoreo multi-fabricante (Cisco, Fortinet, pfSense, MikroTik RouterOS, MikroTik Firewalls, UBNT, Cambium, Windows, Linux)
# - Configuración múltiple de dispositivos por fabricante
# - Sistema de logs y reportes avanzado
# - Integración completa con NESS HQ Cloud
# - Programación automática cada 5 minutos
#
# Modo silencioso:
# ./install_relay.sh --silent --config-file devices.conf --token YOUR_TOKEN
#
# IMPORTANTE: Este instalador requiere el ejecutable 'ness-relay-ubuntu'
#             en el mismo directorio que este script.
###############################################################################

# Colores corporativos NESS - Nueva Paleta
WHITE='\033[1;37m'           # #FFFFFF - Color principal
GREEN='\033[38;2;40;167;69m'  # #28a745 - Mensajes de éxito
YELLOW='\033[38;2;255;193;7m' # #ffc107 - Mensajes de advertencia
RED='\033[38;2;220;53;69m'    # #dc3545 - Mensajes de error
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
BLUE='\033[0;34m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m' # No Color

# Variables por defecto
SILENT_MODE=false
FORCE_INSTALL=false
CONFIG_FILE=""
API_TOKEN=""
SERVER_ENV=3  # Por defecto usamos Public Cloud
declare -A SELECTED_VENDORS
declare -A DEVICE_CONFIGS

# Nombre del ejecutable
EXEC_NAME="ness-relay-ubuntu"

# Definir fabricantes disponibles
# mikrotik_fw: vendor oculto en el menú principal, gestionado via sub-menú del grupo MikroTik
VENDORS=("windows" "linux" "cisco" "fortinet" "pfsense" "mikrotik" "ubnt" "c_n" "mikrotik_fw")
VENDOR_NAMES=("Windows Servers" "Linux Servers" "Cisco Devices" "Fortinet Firewalls" "pfSense Firewalls" "MikroTik Devices ▶" "Ubiquiti Switches (UBNT)" "Cambium Networks APs" "")
# Vendors visibles en el menú principal (excluye hidden entries con nombre vacío)
VISIBLE_VENDOR_COUNT=8

# Banner profesional NESS
show_banner() {
    clear
    # Usar printf con color explícito en cada línea para máximo brillo
    printf "${WHITE}${BOLD}"
    cat << 'EOF'
╔══════════════════════════════════════════════════════════════════════════════╗
║                                                                              ║
║    ███╗   ██╗███████╗███████╗███████╗    ██████╗ ███████╗██╗      █████╗ ██╗   ██╗
║    ████╗  ██║██╔════╝██╔════╝██╔════╝    ██╔══██╗██╔════╝██║     ██╔══██╗╚██╗ ██╔╝
║    ██╔██╗ ██║█████╗  ███████╗███████╗    ██████╔╝█████╗  ██║     ███████║ ╚████╔╝ 
║    ██║╚██╗██║██╔══╝  ╚════██║╚════██║    ██╔══██╗██╔══╝  ██║     ██╔══██║  ╚██╔╝  
║    ██║ ╚████║███████╗███████║███████║    ██║  ██║███████╗███████╗██║  ██║   ██║   
║    ╚═╝  ╚═══╝╚══════╝╚══════╝╚══════╝    ╚═╝  ╚═╝╚══════╝╚══════╝╚═╝  ╚═╝   ╚═╝   
║                                                                              ║
║                  🌐  NETWORK RELAY MONITORING SYSTEM  🌐                     ║
║                 Professional Multi-Vendor Edition v2.0.0                     ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
EOF
    printf "${NC}\n"
    echo -e "${WHITE}${BOLD}Version:${NC} ${DIM}2.0.0 Professional Network Relay - Multi-Vendor${NC}"
    echo -e "${WHITE}${BOLD}Platform:${NC} ${DIM}Multi-Distribution Linux (Ubuntu/CentOS/Debian)${NC}"
    echo -e "${WHITE}${BOLD}Features:${NC} ${DIM}SNMP + Multi-Vendor + Cloud Integration${NC}"
    echo ""
}

# Función para mostrar el disclaimer de seguridad y términos de uso
show_security_disclaimer() {
    clear
    echo -e "${WHITE}${BOLD}"
    cat << 'EOF'
╔══════════════════════════════════════════════════════════════════════════════╗
║                                                                              ║
║                      ⚖️  TÉRMINOS DE USO Y LICENCIA  ⚖️                      ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"
    
    echo -e "${CYAN}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}${BOLD}║                         INFORMACIÓN DEL DESARROLLADOR                        ║${NC}"
    echo -e "${CYAN}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${WHITE}Desarrollado por:${NC}    ${BOLD}NETWORK IS COLOMBIA S.A.S${NC}"
    echo -e "${WHITE}Producto:${NC}            ${BOLD}NESS RELAY - Network Monitoring System${NC}"
    echo -e "${WHITE}Versión:${NC}             ${BOLD}2.0.0 Professional Edition${NC}"
    echo -e "${WHITE}Año:${NC}                 ${BOLD}© 2026 - Todos los derechos reservados${NC}"
    echo ""
    
    echo -e "${YELLOW}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}${BOLD}║                            AVISO DE COPYRIGHT                                ║${NC}"
    echo -e "${YELLOW}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${DIM}Este software y toda su documentación asociada son propiedad exclusiva de${NC}"
    echo -e "${DIM}NETWORK IS COLOMBIA S.A.S. Está protegido por las leyes colombianas e${NC}"
    echo -e "${DIM}internacionales de derechos de autor, propiedad intelectual y tratados${NC}"
    echo -e "${DIM}internacionales.${NC}"
    echo ""
    
    echo -e "${RED}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}${BOLD}║                       RESTRICCIONES Y PROHIBICIONES                          ║${NC}"
    echo -e "${RED}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${RED}⛔${NC} ${BOLD}ESTÁ ESTRICTAMENTE PROHIBIDO:${NC}"
    echo ""
    echo -e "   ${RED}•${NC} Copiar, clonar, reproducir o distribuir este software sin autorización"
    echo -e "   ${RED}•${NC} Realizar ingeniería inversa, descompilar o desensamblar el software"
    echo -e "   ${RED}•${NC} Modificar, adaptar o crear obras derivadas del software"
    echo -e "   ${RED}•${NC} Usar el software para fines ilegales o no autorizados"
    echo -e "   ${RED}•${NC} Transferir, sublicenciar, alquilar o prestar el software"
    echo -e "   ${RED}•${NC} Eliminar o alterar avisos de copyright o propiedad intelectual"
    echo ""
    
    echo -e "${RED}${BOLD}⚠️  CONSECUENCIAS LEGALES DEL USO INDEBIDO:${NC}"
    echo ""
    echo -e "${DIM}El uso no autorizado, la copia, distribución o modificación de este software${NC}"
    echo -e "${DIM}constituye una violación de los derechos de autor y propiedad intelectual, lo${NC}"
    echo -e "${DIM}cual puede resultar en:${NC}"
    echo ""
    echo -e "   ${YELLOW}•${NC} Acciones civiles por daños y perjuicios"
    echo -e "   ${YELLOW}•${NC} Procesos penales según las leyes colombianas (Ley 23 de 1982, Ley 44 de 1993)"
    echo -e "   ${YELLOW}•${NC} Sanciones económicas y responsabilidad legal"
    echo -e "   ${YELLOW}•${NC} Cancelación inmediata de la licencia de uso"
    echo ""
    
    echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}${BOLD}║                            USO PERMITIDO                                     ║${NC}"
    echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${GREEN}✓${NC} Este software se proporciona bajo licencia para:"
    echo ""
    echo -e "   ${GREEN}•${NC} Monitoreo de redes y dispositivos autorizados"
    echo -e "   ${GREEN}•${NC} Uso exclusivo en infraestructura del licenciatario"
    echo -e "   ${GREEN}•${NC} Fines de administración y gestión de red legítimos"
    echo ""
    
    echo -e "${CYAN}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}${BOLD}║                      POLÍTICA DE PRIVACIDAD Y DATOS                          ║${NC}"
    echo -e "${CYAN}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${DIM}Este software recopila datos de rendimiento y estado de dispositivos de red${NC}"
    echo -e "${DIM}con el único propósito de monitoreo y generación de reportes. Los datos son${NC}"
    echo -e "${DIM}transmitidos de forma segura a los servidores NESS HQ. El usuario es${NC}"
    echo -e "${DIM}responsable de cumplir con las leyes de protección de datos aplicables.${NC}"
    echo ""
    
    echo -e "${BLUE}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}${BOLD}║                          LIMITACIÓN DE GARANTÍA                              ║${NC}"
    echo -e "${BLUE}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${DIM}Este software se proporciona 'TAL CUAL', sin garantías de ningún tipo, expresas${NC}"
    echo -e "${DIM}o implícitas. NETWORK IS COLOMBIA S.A.S no se hace responsable de daños${NC}"
    echo -e "${DIM}directos, indirectos, incidentales o consecuentes derivados del uso o${NC}"
    echo -e "${DIM}imposibilidad de uso de este software.${NC}"
    echo ""
    
    echo -e "${PURPLE}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${PURPLE}${BOLD}║                            SOPORTE Y CONTACTO                                ║${NC}"
    echo -e "${PURPLE}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${WHITE}Web:${NC}                 ${CYAN}https://nesshq.com${NC}"
    echo -e "${WHITE}Soporte:${NC}             ${CYAN}https://soporte.nesshq.com${NC}"
    echo ""
    
    echo -e "${WHITE}${BOLD}══════════════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${YELLOW}${BOLD}⚠️  IMPORTANTE:${NC} ${WHITE}Al continuar con la instalación, usted declara que:${NC}"
    echo ""
    echo -e "   ${WHITE}1.${NC} Ha leído y comprendido todos los términos y condiciones anteriores"
    echo -e "   ${WHITE}2.${NC} Acepta estar legalmente vinculado por estos términos"
    echo -e "   ${WHITE}3.${NC} Tiene autorización para instalar y usar este software"
    echo -e "   ${WHITE}4.${NC} Usará el software únicamente para fines legítimos y autorizados"
    echo -e "   ${WHITE}5.${NC} No intentará copiar, modificar o distribuir el software"
    echo ""
    echo -e "${WHITE}${BOLD}══════════════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    
    # Solicitar aceptación explícita
    while true; do
        echo -ne "${GREEN}${BOLD}¿Acepta los términos y condiciones? (ACEPTO/rechazo): ${NC}"
        read acceptance
        
        if [[ "$acceptance" == "ACEPTO" ]]; then
            echo ""
            log_message "SUCCESS" "Términos y condiciones aceptados por el usuario"
            echo -e "${GREEN}✓ Términos aceptados. Continuando con la instalación...${NC}"
            sleep 2
            return 0
        elif [[ "$acceptance" == "rechazo" || "$acceptance" == "RECHAZO" || "$acceptance" == "no" || "$acceptance" == "NO" ]]; then
            echo ""
            echo -e "${RED}${BOLD}✗ Instalación cancelada por el usuario.${NC}"
            echo -e "${YELLOW}No se ha aceptado los términos. El software no será instalado.${NC}"
            echo ""
            log_message "INFO" "Instalación cancelada: términos no aceptados"
            exit 0
        else
            echo -e "${RED}Respuesta no válida. Por favor escriba 'ACEPTO' para continuar o 'rechazo' para cancelar.${NC}"
            echo ""
        fi
    done
}

# Función de logging profesional
log_message() {
    local level=$1
    local message=$2
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    # Usar directorio temporal durante instalación inicial, luego /opt/ness_relay/logs
    if [[ -d "/opt/ness_relay/logs" ]]; then
        local logfile="/opt/ness_relay/logs/install.log"
    else
        local logfile="/tmp/ness_relay_install.log"
    fi
    
    case $level in
        "INFO")
            echo -e "${WHITE}ℹ️  [${timestamp}] ${message}${NC}"
            echo "[${timestamp}] [INFO] ${message}" >> "$logfile" 2>/dev/null
            ;;
        "SUCCESS")
            echo -e "${GREEN}✅ [${timestamp}] ${message}${NC}"
            echo "[${timestamp}] [SUCCESS] ${message}" >> "$logfile" 2>/dev/null
            ;;
        "WARNING")
            echo -e "${YELLOW}⚠️  [${timestamp}] ${message}${NC}"
            echo "[${timestamp}] [WARNING] ${message}" >> "$logfile" 2>/dev/null
            ;;
        "ERROR")
            echo -e "${RED}❌ [${timestamp}] ${message}${NC}"
            echo "[${timestamp}] [ERROR] ${message}" >> "$logfile" 2>/dev/null
            ;;
        "PROGRESS")
            echo -e "${WHITE}⏳ [${timestamp}] ${message}${NC}"
            echo "[${timestamp}] [PROGRESS] ${message}" >> "$logfile" 2>/dev/null
            ;;
        *)
            echo -e "${WHITE}[${timestamp}] ${message}${NC}"
            echo "[${timestamp}] ${message}" >> "$logfile" 2>/dev/null
            ;;
    esac
}

# Función para mostrar el menú de fabricantes
show_vendor_menu() {
    echo ""
    echo -e "${WHITE}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${WHITE}${BOLD}║                     SELECCIÓN DE FABRICANTES/DISPOSITIVOS                    ║${NC}"
    echo -e "${WHITE}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    for i in "${!VENDORS[@]}"; do
        vendor="${VENDORS[$i]}"
        name="${VENDOR_NAMES[$i]}"
        # Saltarse vendors ocultos (nombre vacío — gestionados via sub-menú)
        [[ -z "$name" ]] && continue
        # Estado especial para el grupo MikroTik: activo si RouterOS o Firewall están activos
        if [[ "$vendor" == "mikrotik" ]]; then
            if [[ "${SELECTED_VENDORS[mikrotik]}" == "true" || "${SELECTED_VENDORS[mikrotik_fw]}" == "true" ]]; then
                status="${GREEN}[✓ SELECCIONADO]${NC}"
            else
                status="${DIM}[   DISPONIBLE]${NC}"
            fi
        else
            if [[ "${SELECTED_VENDORS[$vendor]}" == "true" ]]; then
                status="${GREEN}[✓ SELECCIONADO]${NC}"
            else
                status="${DIM}[   DISPONIBLE]${NC}"
            fi
        fi
        printf "%d. %b %b\n" $((i+1)) "$status" "${WHITE}$name${NC}"
    done

    echo ""
    echo -e "${WHITE}${BOLD}Opciones:${NC}"
    echo -e "  ${WHITE}a.${NC} Seleccionar/Deseleccionar todos"
    echo -e "  ${GREEN}c.${NC} Continuar con la instalación"
    echo -e "  ${RED}q.${NC} Salir"
    echo ""
}

# Sub-menú de dispositivos MikroTik (RouterOS vs Firewall)
show_mikrotik_submenu() {
    while true; do
        clear
        show_banner
        echo ""
        echo -e "${WHITE}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${WHITE}${BOLD}║                       DISPOSITIVOS MIKROTIK                                  ║${NC}"
        echo -e "${WHITE}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
        echo ""
        echo -e "${DIM}  Seleccione el tipo de dispositivo MikroTik a monitorear.${NC}"
        echo -e "${DIM}  Ambos perfiles usan la misma MIKROTIK-MIB (mismo OID enterprise).${NC}"
        echo -e "${DIM}  La diferencia está en las funcionalidades de monitoreo activas.${NC}"
        echo ""

        # Estado RouterOS
        if [[ "${SELECTED_VENDORS[mikrotik]}" == "true" ]]; then
            routeros_status="${GREEN}[✓ SELECCIONADO]${NC}"
        else
            routeros_status="${DIM}[   DISPONIBLE]${NC}"
        fi

        # Estado Firewall
        if [[ "${SELECTED_VENDORS[mikrotik_fw]}" == "true" ]]; then
            fw_status="${GREEN}[✓ SELECCIONADO]${NC}"
        else
            fw_status="${DIM}[   DISPONIBLE]${NC}"
        fi

        printf "  1. %b %b\n" "$routeros_status" "${WHITE}RouterOS — Routers y Switches${NC}"
        echo -e "     ${DIM}Modelos: CHR, CCR, RB series (como router/switch).${NC}"
        echo -e "     ${DIM}Monitoreo: CPU, memoria, disco, health (temp/voltaje), wireless.${NC}"
        echo ""
        printf "  2. %b %b\n" "$fw_status" "${WHITE}Firewall — Gateway y Perimetral (CHR/CCR/RB)${NC}"
        echo -e "     ${DIM}Modelos: CHR, CCR2004/2116/1036, RB4011, RB3011, RB1100, L009.${NC}"
        echo -e "     ${DIM}Monitoreo: + Netwatch (ISP probes), interfaces WAN, canales de${NC}"
        echo -e "     ${DIM}Internet (ETB/Tigo/Claro), Queue Simple (ancho de banda por canal).${NC}"
        echo ""
        echo -e "  ${RED}b.${NC} Volver al menú principal"
        echo ""
        echo -ne "${BOLD}Seleccione sub-tipo MikroTik [1/2/b]: ${NC}"
        read sub_choice

        case "$sub_choice" in
            "1")
                if [[ "${SELECTED_VENDORS[mikrotik]}" == "true" ]]; then
                    SELECTED_VENDORS["mikrotik"]="false"
                    log_message "INFO" "MikroTik RouterOS deseleccionado"
                else
                    SELECTED_VENDORS["mikrotik"]="true"
                    log_message "SUCCESS" "MikroTik RouterOS seleccionado"
                    configure_vendor_devices "mikrotik" "MikroTik RouterOS"
                fi
                echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                read
                ;;
            "2")
                if [[ "${SELECTED_VENDORS[mikrotik_fw]}" == "true" ]]; then
                    SELECTED_VENDORS["mikrotik_fw"]="false"
                    log_message "INFO" "MikroTik Firewalls deseleccionado"
                else
                    SELECTED_VENDORS["mikrotik_fw"]="true"
                    log_message "SUCCESS" "MikroTik Firewalls (CHR/CCR/RB) seleccionado"
                    configure_vendor_devices "mikrotik_fw" "MikroTik Firewalls (CHR/CCR/RB)"
                fi
                echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                read
                ;;
            "b"|"B")
                return
                ;;
            *)
                log_message "WARNING" "Opción inválida. Use 1, 2 o b"
                echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                read
                ;;
        esac
    done
}

# Función para configurar dispositivos de un fabricante
configure_vendor_devices() {
    local vendor="$1"
    local vendor_name="$2"
    
    echo ""
    echo -e "${WHITE}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${WHITE}${BOLD}║                       CONFIGURACIÓN: $vendor_name${NC}                       ${WHITE}${BOLD}║${NC}"
    echo -e "${WHITE}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    local device_count=0
    
    while true; do
        device_count=$((device_count + 1))
        
        echo -e "${WHITE}${BOLD}📡 Dispositivo #$device_count para $vendor_name:${NC}"
        
        # Solicitar IP
        while true; do
            echo -ne "${WHITE}  🌐 IP/Host del dispositivo (o 'fin' para terminar): ${NC}"
            read device_ip
            if [[ "$device_ip" == "fin" ]]; then
                return
            elif [[ -n "$device_ip" ]]; then
                break
            else
                echo -e "${RED}  ❌ Error: La IP no puede estar vacía${NC}"
            fi
        done
        
        # Preguntar versión de SNMP
        echo ""
        echo -e "${WHITE}${BOLD}  Selecciona la versión de SNMP:${NC}"
        echo -e "    ${WHITE}1)${NC} SNMPv1  ${DIM}(Community string - protocolo legacy, sin cifrado)${NC}"
        echo -e "    ${WHITE}2)${NC} SNMPv2c ${DIM}(Community string - mejor rendimiento, sin cifrado)${NC}"
        echo -e "    ${WHITE}3)${NC} SNMPv3  ${DIM}(Usuario/Contraseña - ${GREEN}RECOMENDADO${NC}${DIM}: con autenticación y cifrado)${NC}"
        echo -ne "${WHITE}  Selecciona 1, 2 o 3 [default: 3]: ${NC}"
        read snmp_version_choice
        snmp_version_choice=${snmp_version_choice:-3}
        
        if [[ "$snmp_version_choice" == "1" ]]; then
            # Configuración SNMPv1
            snmp_version="1"
            echo -e "${YELLOW}  ⚠️  SNMPv1 seleccionado - Protocolo legacy sin seguridad${NC}"
            echo -ne "${WHITE}  🔑 Community string SNMP [default: public]: ${NC}"
            read community
            community=${community:-public}
        elif [[ "$snmp_version_choice" == "2" ]]; then
            # Configuración SNMPv2c
            snmp_version="2c"
            echo -e "${YELLOW}  ⚠️  SNMPv2c seleccionado - Sin cifrado de datos${NC}"
            echo -ne "${WHITE}  🔑 Community string SNMP [default: public]: ${NC}"
            read community
            community=${community:-public}
        else
            # Configuración SNMPv3
            snmp_version="3"
            echo ""
            echo -e "${CYAN}${BOLD}  ═══ Configuración SNMPv3 ═══${NC}"
            
            # Usuario SNMPv3
            while true; do
                echo -ne "${WHITE}  👤 Usuario SNMPv3: ${NC}"
                read v3_user
                if [[ -n "$v3_user" ]]; then
                    break
                else
                    echo -e "${RED}  ❌ Error: El usuario no puede estar vacío${NC}"
                fi
            done
            
            # Protocolo de autenticación
            echo ""
            echo -e "${WHITE}  Protocolo de Autenticación:${NC}"
            echo -e "    ${WHITE}1)${NC} SHA ${DIM}(recomendado)${NC}"
            echo -e "    ${WHITE}2)${NC} MD5"
            echo -e "    ${WHITE}3)${NC} NONE ${DIM}(sin autenticación - no recomendado)${NC}"
            echo -ne "${WHITE}  Selecciona 1, 2 o 3 [default: 1]: ${NC}"
            read auth_choice
            auth_choice=${auth_choice:-1}
            
            case "$auth_choice" in
                1) v3_auth_protocol="SHA" ;;
                2) v3_auth_protocol="MD5" ;;
                3) v3_auth_protocol="NONE" ;;
                *) v3_auth_protocol="SHA" ;;
            esac
            
            # Contraseña de autenticación
            if [[ "$v3_auth_protocol" != "NONE" ]]; then
                while true; do
                    echo -ne "${WHITE}  🔐 Contraseña de Autenticación (mín. 8 caracteres): ${NC}"
                    read -s v3_auth_password
                    echo ""
                    if [[ ${#v3_auth_password} -ge 8 ]]; then
                        break
                    else
                        echo -e "${RED}  ❌ Error: La contraseña debe tener al menos 8 caracteres${NC}"
                    fi
                done
            else
                v3_auth_password=""
            fi
            
            # Protocolo de privacidad (encriptación)
            echo ""
            echo -e "${WHITE}  Protocolo de Privacidad (Encriptación):${NC}"
            echo -e "    ${WHITE}1)${NC} AES128 ${DIM}(recomendado)${NC}"
            echo -e "    ${WHITE}2)${NC} AES192"
            echo -e "    ${WHITE}3)${NC} AES256 ${DIM}(máxima seguridad)${NC}"
            echo -e "    ${WHITE}4)${NC} DES ${DIM}(obsoleto)${NC}"
            echo -e "    ${WHITE}5)${NC} NONE ${DIM}(sin encriptación)${NC}"
            echo -ne "${WHITE}  Selecciona 1-5 [default: 1]: ${NC}"
            read priv_choice
            priv_choice=${priv_choice:-1}
            
            case "$priv_choice" in
                1) v3_priv_protocol="AES128" ;;
                2) v3_priv_protocol="AES192" ;;
                3) v3_priv_protocol="AES256" ;;
                4) v3_priv_protocol="DES" ;;
                5) v3_priv_protocol="NONE" ;;
                *) v3_priv_protocol="AES128" ;;
            esac
            
            # Contraseña de privacidad
            if [[ "$v3_priv_protocol" != "NONE" && "$v3_auth_protocol" != "NONE" ]]; then
                while true; do
                    echo -ne "${WHITE}  🔐 Contraseña de Privacidad (mín. 8 caracteres): ${NC}"
                    read -s v3_priv_password
                    echo ""
                    if [[ ${#v3_priv_password} -ge 8 ]]; then
                        break
                    else
                        echo -e "${RED}  ❌ Error: La contraseña debe tener al menos 8 caracteres${NC}"
                    fi
                done
            else
                v3_priv_password=""
            fi
        fi
        
        # Solicitar puerto
        echo -ne "${WHITE}  🔌 Puerto SNMP [default: 161]: ${NC}"
        read port
        port=${port:-161}
        
        # Solicitar descripción opcional
        echo -ne "${WHITE}  📝 Descripción del dispositivo [opcional]: ${NC}"
        read description
        
        # Guardar configuración
        local config_key="${vendor}_${device_count}"
        DEVICE_CONFIGS["${config_key}_ip"]="$device_ip"
        DEVICE_CONFIGS["${config_key}_port"]="$port"
        DEVICE_CONFIGS["${config_key}_description"]="$description"
        DEVICE_CONFIGS["${config_key}_vendor"]="$vendor"
        DEVICE_CONFIGS["${config_key}_snmp_version"]="$snmp_version"
        
        if [[ "$snmp_version" == "1" ]]; then
            DEVICE_CONFIGS["${config_key}_community"]="$community"
            echo -e "${GREEN}  ✅ Dispositivo SNMPv1 configurado: ${BOLD}$device_ip${NC} ${DIM}($description)${NC}"
            echo -e "${YELLOW}  ⚠️  Recordatorio: SNMPv1 no proporciona seguridad - considere actualizar${NC}"
        elif [[ "$snmp_version" == "2c" ]]; then
            DEVICE_CONFIGS["${config_key}_community"]="$community"
            echo -e "${GREEN}  ✅ Dispositivo SNMPv2c configurado: ${BOLD}$device_ip${NC} ${DIM}($description)${NC}"
            echo -e "${YELLOW}  ⚠️  Recordatorio: SNMPv2c no cifra datos - considere SNMPv3${NC}"
        else
            DEVICE_CONFIGS["${config_key}_v3_user"]="$v3_user"
            DEVICE_CONFIGS["${config_key}_v3_auth_protocol"]="$v3_auth_protocol"
            DEVICE_CONFIGS["${config_key}_v3_auth_password"]="$v3_auth_password"
            DEVICE_CONFIGS["${config_key}_v3_priv_protocol"]="$v3_priv_protocol"
            DEVICE_CONFIGS["${config_key}_v3_priv_password"]="$v3_priv_password"
            echo -e "${GREEN}  ✅ Dispositivo SNMPv3 configurado: ${BOLD}$device_ip${NC} ${DIM}(usuario: $v3_user)${NC}"
        fi
        echo ""
        
        echo -ne "${YELLOW}¿Agregar otro dispositivo $vendor_name? (y/N): ${NC}"
        read add_another
        if [[ "$add_another" != "y" && "$add_another" != "Y" ]]; then
            break
        fi
    done
    
    # Actualizar contador de dispositivos para este vendor
    DEVICE_CONFIGS["${vendor}_count"]="$device_count"
}

# Función para mostrar el menú interactivo
interactive_vendor_selection() {
    while true; do
        clear
        show_banner
        show_vendor_menu
        
        echo -ne "${BOLD}Seleccione una opción: ${NC}"
        read choice
        
        case "$choice" in
            [0-9]*)
                # Validar usando VISIBLE_VENDOR_COUNT (excluye vendors ocultos como mikrotik_fw)
                if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= VISIBLE_VENDOR_COUNT )); then
                    vendor_index=$((choice - 1))
                    vendor="${VENDORS[$vendor_index]}"
                    vendor_name="${VENDOR_NAMES[$vendor_index]}"

                    # Grupo MikroTik ▶ abre el sub-menú (RouterOS o Firewall)
                    if [[ "$vendor" == "mikrotik" ]]; then
                        show_mikrotik_submenu
                        echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                        read
                        continue
                    fi

                    if [[ "${SELECTED_VENDORS[$vendor]}" == "true" ]]; then
                        SELECTED_VENDORS["$vendor"]="false"
                        log_message "INFO" "$vendor_name deseleccionado"
                    else
                        SELECTED_VENDORS["$vendor"]="true"
                        log_message "SUCCESS" "$vendor_name seleccionado"
                        configure_vendor_devices "$vendor" "$vendor_name"
                    fi
                    
                    echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                    read
                else
                    log_message "WARNING" "Opción inválida. Seleccione un número del 1 al $VISIBLE_VENDOR_COUNT"
                    echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                    read
                fi
                ;;
            "a"|"A")
                # Seleccionar/Deseleccionar todos (solo vendors visibles del menú)
                all_selected=true
                for i in "${!VENDORS[@]}"; do
                    vendor="${VENDORS[$i]}"
                    name="${VENDOR_NAMES[$i]}"
                    # Omitir vendors ocultos del chequeo
                    [[ -z "$name" ]] && continue
                    # Para el grupo MikroTik: activo si alguno de sus sub-tipos está seleccionado
                    if [[ "$vendor" == "mikrotik" ]]; then
                        if [[ "${SELECTED_VENDORS[mikrotik]}" != "true" && "${SELECTED_VENDORS[mikrotik_fw]}" != "true" ]]; then
                            all_selected=false
                            break
                        fi
                    else
                        if [[ "${SELECTED_VENDORS[$vendor]}" != "true" ]]; then
                            all_selected=false
                            break
                        fi
                    fi
                done
                
                if [[ "$all_selected" == "true" ]]; then
                    # Deseleccionar todos (incluyendo vendors ocultos)
                    for vendor in "${VENDORS[@]}"; do
                        SELECTED_VENDORS["$vendor"]="false"
                    done
                    log_message "INFO" "Todos los fabricantes deseleccionados"
                else
                    # Seleccionar y configurar solo vendors visibles
                    for i in "${!VENDORS[@]}"; do
                        vendor="${VENDORS[$i]}"
                        vendor_name="${VENDOR_NAMES[$i]}"
                        # Omitir vendors ocultos (mikrotik_fw se gestiona via sub-menú)
                        [[ -z "$vendor_name" ]] && continue
                        # Para el grupo MikroTik en "select all": configura RouterOS por defecto
                        # (para Firewalls usar el sub-menú manualmente)
                        local display_name="$vendor_name"
                        if [[ "$vendor" == "mikrotik" ]]; then
                            display_name="MikroTik RouterOS"
                        fi
                        SELECTED_VENDORS["$vendor"]="true"
                        configure_vendor_devices "$vendor" "$display_name"
                    done
                    log_message "SUCCESS" "Todos los fabricantes visibles seleccionados y configurados"
                fi
                
                echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                read
                ;;
            "c"|"C")
                # Verificar que al menos un fabricante esté seleccionado
                selected_count=0
                for vendor in "${VENDORS[@]}"; do
                    if [[ "${SELECTED_VENDORS[$vendor]}" == "true" ]]; then
                        selected_count=$((selected_count + 1))
                    fi
                done
                
                if [[ $selected_count -eq 0 ]]; then
                    log_message "ERROR" "Debe seleccionar al menos un fabricante"
                    echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                    read
                else
                    break
                fi
                ;;
            "q"|"Q")
                log_message "WARNING" "Instalación cancelada por el usuario"
                exit 0
                ;;
            *)
                log_message "WARNING" "Opción inválida"
                echo -ne "\n${DIM}Presione Enter para continuar...${NC}"
                read
                ;;
        esac
    done
}

# Función para cargar configuración desde archivo
load_config_file() {
    local config_file="$1"
    
    if [[ ! -f "$config_file" ]]; then
        log_message "ERROR" "Archivo de configuración no encontrado: $config_file"
        exit 1
    fi
    
    log_message "PROGRESS" "Cargando configuración desde: $config_file"
    
    # Leer configuración del archivo
    while IFS='=' read -r key value; do
        # Ignorar líneas vacías y comentarios
        [[ -z "$key" || "$key" =~ ^[[:space:]]*# ]] && continue
        
        # Remover espacios en blanco
        key=$(echo "$key" | tr -d '[:space:]')
        value=$(echo "$value" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        
        # Procesar configuración
        if [[ "$key" =~ ^([a-z]+)_([0-9]+)_(.+)$ ]]; then
            DEVICE_CONFIGS["$key"]="$value"
            vendor="${BASH_REMATCH[1]}"
            SELECTED_VENDORS["$vendor"]="true"
        elif [[ "$key" =~ ^([a-z]+)_count$ ]]; then
            DEVICE_CONFIGS["$key"]="$value"
        fi
    done < "$config_file"
}

# Función para generar archivo de configuración
generate_config_file() {
    local config_file="$INSTALL_DIR/configs/devices.conf"
    
    {
        echo "# ═══════════════════════════════════════════════════════════"
        echo "# NESS RELAY - Configuración de Dispositivos"
        echo "# Generado automáticamente el $(date)"
        echo "# ═══════════════════════════════════════════════════════════"
        echo "# "
        echo "# Soporta SNMPv1, SNMPv2c y SNMPv3"
        echo "# "
        echo "# Para SNMPv1 (Legacy - sin seguridad):"
        echo "#   <vendor>_<num>_snmp_version=1"
        echo "#   <vendor>_<num>_community=<string>"
        echo "# "
        echo "# Para SNMPv2c (Sin cifrado):"
        echo "#   <vendor>_<num>_snmp_version=2c"
        echo "#   <vendor>_<num>_community=<string>"
        echo "# "
        echo "# Para SNMPv3 (RECOMENDADO - con autenticación y cifrado):"
        echo "#   <vendor>_<num>_snmp_version=3"
        echo "#   <vendor>_<num>_v3_user=<username>"
        echo "#   <vendor>_<num>_v3_auth_protocol=SHA|MD5|NONE"
        echo "#   <vendor>_<num>_v3_auth_password=<password>"
        echo "#   <vendor>_<num>_v3_priv_protocol=AES128|AES192|AES256|DES|NONE"
        echo "#   <vendor>_<num>_v3_priv_password=<password>"
        echo "# "
        echo "# ═══════════════════════════════════════════════════════════"
        echo ""
        
        for vendor in "${VENDORS[@]}"; do
            if [[ "${SELECTED_VENDORS[$vendor]}" == "true" ]]; then
                local count="${DEVICE_CONFIGS[${vendor}_count]}"
                echo "# ─────────────────────────────────────────────────────────"
                echo "# Dispositivos $vendor"
                echo "# ─────────────────────────────────────────────────────────"
                echo "${vendor}_count=$count"
                
                for ((i=1; i<=count; i++)); do
                    local config_key="${vendor}_${i}"
                    local snmp_ver="${DEVICE_CONFIGS[${config_key}_snmp_version]}"
                    
                    echo ""
                    echo "# Dispositivo $i: ${DEVICE_CONFIGS[${config_key}_description]}"
                    echo "${config_key}_ip=${DEVICE_CONFIGS[${config_key}_ip]}"
                    echo "${config_key}_port=${DEVICE_CONFIGS[${config_key}_port]}"
                    echo "${config_key}_vendor=${DEVICE_CONFIGS[${config_key}_vendor]}"
                    echo "${config_key}_description=${DEVICE_CONFIGS[${config_key}_description]}"
                    echo "${config_key}_snmp_version=$snmp_ver"
                    
                    if [[ "$snmp_ver" == "1" ]] || [[ "$snmp_ver" == "2c" ]]; then
                        # SNMPv1 o SNMPv2c - solo requieren community string
                        echo "${config_key}_community=${DEVICE_CONFIGS[${config_key}_community]}"
                    else
                        # SNMPv3 - requiere credenciales completas
                        echo "${config_key}_v3_user=${DEVICE_CONFIGS[${config_key}_v3_user]}"
                        echo "${config_key}_v3_auth_protocol=${DEVICE_CONFIGS[${config_key}_v3_auth_protocol]}"
                        echo "${config_key}_v3_auth_password=${DEVICE_CONFIGS[${config_key}_v3_auth_password]}"
                        echo "${config_key}_v3_priv_protocol=${DEVICE_CONFIGS[${config_key}_v3_priv_protocol]}"
                        echo "${config_key}_v3_priv_password=${DEVICE_CONFIGS[${config_key}_v3_priv_password]}"
                    fi
                done
                echo ""
            fi
        done
    } > "$config_file"
    
    log_message "SUCCESS" "Configuración guardada en: $config_file"
}

# Procesar argumentos
while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        --silent)
            SILENT_MODE=true
            shift
            ;;
        --config-file)
            CONFIG_FILE="$2"
            shift 2
            ;;
        --force)
            FORCE_INSTALL=true
            shift
            ;;
        --token)
            API_TOKEN="$2"
            shift 2
            ;;
        --env)
            SERVER_ENV="$2"
            shift 2
            ;;
        --help)
            echo "Uso: ./install_relay.sh [opciones]"
            echo "Opciones:"
            echo "  --silent               Instalar en modo silencioso"
            echo "  --config-file FILE     Usar archivo de configuración"
            echo "  --force                Forzar instalación si existe"
            echo ""
            echo "Ejemplo de archivo de configuración:"
            echo "cisco_count=2"
            echo "cisco_1_ip=192.168.1.1"
            echo "cisco_1_community=public"
            echo "cisco_1_port=161"
            echo "cisco_1_description=Router Principal"
            echo "cisco_1_vendor=cisco"
            echo "fortinet_count=1"
            echo "fortinet_1_ip=192.168.1.254"
            echo "fortinet_1_community=private"
            echo "fortinet_1_port=161"
            echo "fortinet_1_description=Firewall Principal"
            echo "fortinet_1_vendor=fortinet"
            exit 0
            ;;
        *)
            echo "Opción desconocida: $1"
            echo "Use --help para ver opciones disponibles"
            exit 1
            ;;
    esac
done

# Mostrar banner inicial
show_banner

# Mostrar disclaimer de seguridad y solicitar aceptación (OBLIGATORIO)
if [[ "$SILENT_MODE" != "true" ]]; then
    show_security_disclaimer
fi

# Verificar root
if [ "$EUID" -ne 0 ]; then
    log_message "ERROR" "Este script debe ejecutarse como root (sudo)"
    echo ""
    echo -e "${YELLOW}${BOLD}Uso correcto:${NC}"
    echo -e "  ${WHITE}sudo ./install_relay.sh${NC}"
    echo ""
    exit 1
fi

log_message "SUCCESS" "Permisos de root verificados"

# Verificar que el ejecutable existe en el directorio actual
log_message "PROGRESS" "Verificando ejecutable..."
if [ ! -f "./$EXEC_NAME" ]; then
    log_message "ERROR" "No se encuentra el archivo '$EXEC_NAME' en este directorio"
    echo ""
    echo -e "${YELLOW}${BOLD}Asegúrese de:${NC}"
    echo -e "  ${WHITE}1.${NC} Haber compilado el agente con ${CYAN}build_relay_executable.sh${NC}"
    echo -e "  ${WHITE}2.${NC} Copiar el ejecutable desde ${CYAN}dist/$EXEC_NAME${NC} a este directorio"
    echo ""
    echo -e "${YELLOW}${BOLD}Ejemplo:${NC}"
    echo -e "  ${WHITE}cp dist/$EXEC_NAME .${NC}"
    echo ""
    exit 1
fi

log_message "SUCCESS" "Ejecutable '$EXEC_NAME' encontrado"
echo ""

# Selección de fabricantes
if [[ "$SILENT_MODE" == "true" && -n "$CONFIG_FILE" ]]; then
    load_config_file "$CONFIG_FILE"
elif [[ "$SILENT_MODE" != "true" ]]; then
    # Configurar URL del servidor
    echo -e "${WHITE}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${WHITE}${BOLD}║                          CONFIGURACIÓN DEL SERVIDOR                          ║${NC}"
    echo -e "${WHITE}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${WHITE}Selecciona el entorno del servidor:${NC}"
    echo -e "  ${WHITE}1)${NC} On-premise: 172.206.0.217"
    echo -e "  ${WHITE}2)${NC} Testing: testing.nesshq.com"
    echo -e "  ${WHITE}3)${NC} Public Cloud: cloud.nesshq.com"
    echo ""
    echo -ne "${BOLD}Ingresa 1, 2 o 3 [default: 3]: ${NC}"
    read ENV_SELECTION
    ENV_SELECTION=${ENV_SELECTION:-3}
    SERVER_ENV=$ENV_SELECTION

    echo ""
    # Solicitar token
    while [[ -z "$API_TOKEN" ]]; do
        echo -ne "${BOLD}🔑 Ingresa tu NESS_API_TOKEN: ${NC}"
        read API_TOKEN
        if [[ -z "$API_TOKEN" ]]; then
            log_message "ERROR" "El token es obligatorio"
        fi
    done

    log_message "SUCCESS" "Token de API configurado"
    echo ""
    interactive_vendor_selection
else
    log_message "ERROR" "En modo silencioso debe especificar --config-file"
    exit 1
fi

# Guardar solo el ID del servidor (el ejecutable tiene las URLs hardcodeadas)
# Esto mejora la seguridad al no exponer las rutas de los endpoints
case "$SERVER_ENV" in
    "1")
        SERVER_ID="1"
        log_message "SUCCESS" "Servidor On-premise seleccionado"
        ;;
    "2")
        SERVER_ID="2"
        log_message "SUCCESS" "Servidor de Producción seleccionado"
        ;;
    "3")
        SERVER_ID="3"
        log_message "SUCCESS" "Servidor Public Cloud seleccionado"
        ;;
    *)
        SERVER_ID="3"
        log_message "SUCCESS" "Servidor Public Cloud seleccionado (default)"
        ;;
esac

# Verificar token en modo silencioso
if [[ -z "$API_TOKEN" ]]; then
    log_message "ERROR" "No se proporcionó un token de API. Use --token YOUR_TOKEN"
    exit 1
fi

# Mostrar resumen de configuración
echo ""
echo -e "${WHITE}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${WHITE}${BOLD}║                          RESUMEN DE CONFIGURACIÓN                            ║${NC}"
echo -e "${WHITE}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
echo ""
total_devices=0
for vendor in "${VENDORS[@]}"; do
    if [[ "${SELECTED_VENDORS[$vendor]}" == "true" ]]; then
        count="${DEVICE_CONFIGS[${vendor}_count]}"
        echo -e "${GREEN}✓${NC} ${WHITE}$vendor:${NC} ${DIM}$count dispositivo(s)${NC}"
        total_devices=$((total_devices + count))
    fi
done
echo ""
echo -e "${WHITE}${BOLD}Total de dispositivos a monitorear: ${GREEN}$total_devices${NC}"
echo ""

if [[ "$SILENT_MODE" != "true" ]]; then
    echo -ne "${YELLOW}${BOLD}¿Continuar con la instalación? (Y/n): ${NC}"
    read continue_install
    if [[ "$continue_install" == "n" || "$continue_install" == "N" ]]; then
        log_message "WARNING" "Instalación cancelada por el usuario"
        exit 0
    fi
fi

# Crear directorio de instalación
INSTALL_DIR="/opt/ness_relay"
if [[ -d "$INSTALL_DIR" && "$FORCE_INSTALL" != "true" ]]; then
    # El directorio ya existe, ofrecer opciones al usuario
    echo ""
    echo -e "${YELLOW}${BOLD}⚠️  INSTALACIÓN EXISTENTE DETECTADA${NC}"
    echo -e "${WHITE}El directorio $INSTALL_DIR ya existe.${NC}"
    echo ""
    echo -e "${WHITE}${BOLD}Selecciona una opción:${NC}"
    echo -e "  ${WHITE}1)${NC} ${GREEN}Reinstalar completamente${NC} ${DIM}(elimina todo y crea una instalación nueva)${NC}"
    echo -e "  ${WHITE}2)${NC} ${YELLOW}Actualizar configuración${NC} ${DIM}(mantiene estructura, actualiza configuraciones)${NC}"
    echo -e "  ${WHITE}3)${NC} ${RED}Cancelar instalación${NC}"
    echo ""
    echo -ne "${BOLD}Selecciona una opción (1-3): ${NC}"
    read reinstall_option
    
    case "$reinstall_option" in
        1)
            # Opción 1: Reinstalación completa
            log_message "WARNING" "Reinstalación completa seleccionada"
            
            # Crear backup con fecha
            BACKUP_DATE=$(date '+%Y%m%d_%H%M%S')
            BACKUP_DIR="/opt/ness_relay_backup_${BACKUP_DATE}"
            
            log_message "PROGRESS" "Creando backup de la instalación existente..."
            cp -r "$INSTALL_DIR" "$BACKUP_DIR" 2>/dev/null
            
            if [[ -d "$BACKUP_DIR" ]]; then
                log_message "SUCCESS" "Backup creado en: $BACKUP_DIR"
                echo ""
                echo -e "${GREEN}${BOLD}✅ Backup completado${NC}"
                echo -e "${WHITE}  • Ubicación: ${BOLD}$BACKUP_DIR${NC}"
                echo -e "${WHITE}  • Contenido: Configuraciones, logs y ejecutable anterior${NC}"
                echo ""
            else
                log_message "WARNING" "No se pudo crear el backup completo"
            fi
            
            # Eliminar instalación anterior
            log_message "PROGRESS" "Eliminando instalación anterior..."
            rm -rf "$INSTALL_DIR"
            log_message "SUCCESS" "Instalación anterior eliminada"
            ;;
            
        2)
            # Opción 2: Actualización de configuración
            log_message "WARNING" "Actualización de configuración seleccionada"
            
            # Crear backup con fecha
            BACKUP_DATE=$(date '+%Y%m%d_%H%M%S')
            BACKUP_DIR="/opt/ness_relay_backup_${BACKUP_DATE}"
            
            log_message "PROGRESS" "Creando backup de configuraciones existentes..."
            mkdir -p "$BACKUP_DIR"
            
            # Backup de archivos críticos (soporta ambas estructuras: legacy y nueva)
            [[ -f "$INSTALL_DIR/devices.conf" ]] && cp "$INSTALL_DIR/devices.conf" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/configs/devices.conf" ]] && cp "$INSTALL_DIR/configs/devices.conf" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/install.log" ]] && cp "$INSTALL_DIR/install.log" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/logs/install.log" ]] && cp "$INSTALL_DIR/logs/install.log" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/relay.log" ]] && cp "$INSTALL_DIR/relay.log" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/logs/relay.log" ]] && cp "$INSTALL_DIR/logs/relay.log" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "/etc/profile.d/ness_relay.sh" ]] && cp "/etc/profile.d/ness_relay.sh" "$BACKUP_DIR/" 2>/dev/null
            
            log_message "SUCCESS" "Backup de configuraciones creado en: $BACKUP_DIR"
            echo ""
            echo -e "${GREEN}${BOLD}✅ Backup completado${NC}"
            echo -e "${WHITE}  • Ubicación: ${BOLD}$BACKUP_DIR${NC}"
            echo -e "${WHITE}  • Contenido: devices.conf, logs y variables de entorno${NC}"
            echo ""
            
            log_message "INFO" "Modo actualización: se sobrescribirán configuraciones"
            ;;
            
        3)
            log_message "WARNING" "Instalación cancelada por el usuario"
            exit 0
            ;;
            
        *)
            log_message "ERROR" "Opción inválida. Instalación cancelada"
            exit 1
            ;;
    esac
fi

log_message "PROGRESS" "Creando estructura de directorios organizada..."
mkdir -p "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR/configs"
mkdir -p "$INSTALL_DIR/devices"
mkdir -p "$INSTALL_DIR/executables"
mkdir -p "$INSTALL_DIR/logs"
mkdir -p "$INSTALL_DIR/output"

log_message "SUCCESS" "Estructura de directorios creada:"
echo -e "${WHITE}  ├── configs/     ${DIM}(Archivos de configuración)${NC}"
echo -e "${WHITE}  ├── devices/     ${DIM}(Datos de dispositivos monitoreados)${NC}"
echo -e "${WHITE}  ├── executables/ ${DIM}(Binarios y scripts de ejecución)${NC}"
echo -e "${WHITE}  ├── logs/        ${DIM}(Logs de instalación y operación)${NC}"
echo -e "${WHITE}  └── output/      ${DIM}(Datos JSON exportados)${NC}"
echo ""

# Mover el log temporal al directorio de logs
if [[ -f "/tmp/ness_relay_install.log" ]]; then
    mv "/tmp/ness_relay_install.log" "$INSTALL_DIR/logs/install.log" 2>/dev/null
fi

# Copiar el ejecutable al directorio de executables
log_message "PROGRESS" "Copiando ejecutable..."
cp "./$EXEC_NAME" "$INSTALL_DIR/executables/"
chmod +x "$INSTALL_DIR/executables/$EXEC_NAME"

log_message "SUCCESS" "Ejecutable instalado en: $INSTALL_DIR/executables/$EXEC_NAME"

# Configurar variables de entorno
ENV_FILE="/etc/profile.d/ness_relay.sh"
log_message "PROGRESS" "Configurando variables de entorno..."
{
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# NESS RELAY - Variables de Entorno"
    echo "# Generado automáticamente el $(date)"
    echo "# NOTA: SERVER_ID es un identificador interno (1=On-premise, 2=Producción, 3=Cloud)"
    echo "# Las URLs reales están protegidas dentro del ejecutable compilado"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""
    echo "export NESS_SERVER_ID=\"$SERVER_ID\""
    echo "export NESS_API_TOKEN=\"$API_TOKEN\""
    echo "export NESS_INSTALL_DIR=\"$INSTALL_DIR\""
} > "$ENV_FILE"
chmod +x "$ENV_FILE"
source "$ENV_FILE"
log_message "SUCCESS" "Variables de entorno configuradas en: $ENV_FILE"

# Generar archivo de configuración
generate_config_file

# Crear script de protección para devices.conf
VIEW_CONFIG_SCRIPT="$INSTALL_DIR/executables/view_config.sh"
log_message "PROGRESS" "Creando script de protección para devices.conf..."
{
    echo "#!/bin/bash"
    echo "#"
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# NESS RELAY - Visor Seguro de Configuración"
    echo "# Este script protege el acceso al archivo devices.conf"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""
    echo "CONFIG_FILE='/opt/ness_relay/configs/devices.conf'"
    echo "ENV_FILE='/etc/profile.d/ness_relay.sh'"
    echo ""
    echo "# Colores"
    echo "GREEN='\\033[0;32m'"
    echo "RED='\\033[0;31m'"
    echo "YELLOW='\\033[1;33m'"
    echo "CYAN='\\033[0;36m'"
    echo "WHITE='\\033[1;37m'"
    echo "NC='\\033[0m'"
    echo ""
    echo "# Verificar que el archivo de configuración existe"
    echo "if [[ ! -f \"\$CONFIG_FILE\" ]]; then"
    echo "    echo -e \"\${RED}❌ Error: Archivo de configuración no encontrado.\${NC}\""
    echo "    exit 1"
    echo "fi"
    echo ""
    echo "# Banner de acceso"
    echo "echo -e \"\${CYAN}╔═══════════════════════════════════════════════════════════╗\${NC}\""
    echo "echo -e \"\${CYAN}║       🔐  NESS RELAY - Acceso Protegido              ║\${NC}\""
    echo "echo -e \"\${CYAN}╚═══════════════════════════════════════════════════════════╝\${NC}\""
    echo "echo \"\""
    echo "echo -e \"\${YELLOW}Este archivo contiene información sensible de los dispositivos.\${NC}\""
    echo "echo -e \"\${YELLOW}Se requiere autenticación para acceder.\${NC}\""
    echo "echo \"\""
    echo ""
    echo "# Solicitar contraseña"
    echo "read -sp 'Ingrese la contraseña de acceso: ' INPUT_PASSWORD"
    echo "echo \"\""
    echo "echo \"\""
    echo ""
    echo "# Obtener el token real del sistema"
    echo "if [[ -f \"\$ENV_FILE\" ]]; then"
    echo "    source \"\$ENV_FILE\""
    echo "    STORED_TOKEN=\"\$NESS_API_TOKEN\""
    echo "else"
    echo "    echo -e \"\${RED}❌ Error: No se pudo verificar las credenciales.\${NC}\""
    echo "    exit 1"
    echo "fi"
    echo ""
    echo "# Verificar contraseña"
    echo "if [[ \"\$INPUT_PASSWORD\" == \"\$STORED_TOKEN\" ]]; then"
    echo "    echo -e \"\${GREEN}✓ Contraseña correcta. Acceso concedido.\${NC}\""
    echo "    echo \"\""
    echo "    echo -e \"\${WHITE}═══════════════════════════════════════════════════════════\${NC}\""
    echo "    cat \"\$CONFIG_FILE\""
    echo "    echo -e \"\${WHITE}═══════════════════════════════════════════════════════════\${NC}\""
    echo "else"
    echo "    echo -e \"\${RED}❌ Contraseña incorrecta. Acceso denegado.\${NC}\""
    echo "    exit 1"
    echo "fi"
} > "$VIEW_CONFIG_SCRIPT"

chmod +x "$VIEW_CONFIG_SCRIPT"
log_message "SUCCESS" "Script de protección creado: $VIEW_CONFIG_SCRIPT"

# Proteger el archivo devices.conf con permisos restrictivos
chmod 600 "$INSTALL_DIR/configs/devices.conf"
log_message "SUCCESS" "Permisos de seguridad aplicados a devices.conf (600 - solo root)"

# Crear script de ejecución mejorado con diagnósticos
RUN_SCRIPT="$INSTALL_DIR/executables/run_relay.sh"
log_message "PROGRESS" "Creando script de ejecución..."
{
    echo "#!/bin/bash"
    echo "#"
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# NESS RELAY - Script de Ejecución"
    echo "# Generado automáticamente el $(date)"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""
    echo "# Colores para mensajes"
    echo "GREEN='\033[0;32m'"
    echo "RED='\033[0;31m'"
    echo "YELLOW='\033[1;33m'"
    echo "NC='\033[0m' # No Color"
    echo ""
    echo "# Suprimir warnings de APIs obsoletas (pkg_resources)"
    echo "# Evita mensajes de deprecación de dependencias internas"
    echo "export PYTHONWARNINGS='ignore::DeprecationWarning,ignore::UserWarning'"
    echo ""
    echo "# Cargar variables de entorno"
    echo "source $ENV_FILE"
    echo ""
    echo "# Cambiar al directorio de instalación"
    echo "cd $INSTALL_DIR"
    echo ""
    echo "# Detectar si estamos en un terminal interactivo o en cron"
    echo "if [ -t 1 ]; then"
    echo "    # Terminal interactivo: mostrar salida en tiempo real"
    echo "    echo -e \"\${YELLOW}Ejecutando NESS Relay...\${NC}\""
    echo "    echo \"\""
    echo "    ./executables/$EXEC_NAME --config $INSTALL_DIR/configs/devices.conf"
    echo "    EXIT_CODE=\$?"
    echo "    echo \"\""
    echo "    if [ \$EXIT_CODE -eq 0 ]; then"
    echo "        echo -e \"\${GREEN}✓ Relay ejecutado exitosamente\${NC}\""
    echo "        echo \"Log detallado: $INSTALL_DIR/logs/ness_relay.log\""
    echo "    else"
    echo "        echo -e \"\${RED}✗ Error en la ejecución del relay (código: \$EXIT_CODE)\${NC}\""
    echo "        echo \"Revise el log: $INSTALL_DIR/logs/ness_relay.log\""
    echo "        echo \"\""
    echo "        echo \"Para ver los últimos errores:\""
    echo "        echo \"  tail -n 50 $INSTALL_DIR/logs/ness_relay.log\""
    echo "    fi"
    echo "    exit \$EXIT_CODE"
    echo "else"
    echo "    # Ejecución desde cron: guardar en log"
    echo "    ./executables/$EXEC_NAME --config $INSTALL_DIR/configs/devices.conf >> $INSTALL_DIR/logs/ness_relay.log 2>&1"
    echo "    EXIT_CODE=\$?"
    echo "    if [ \$EXIT_CODE -ne 0 ]; then"
    echo "        echo \"[\$(date '+%Y-%m-%d %H:%M:%S')] ERROR: Relay falló con código \$EXIT_CODE\" >> $INSTALL_DIR/logs/ness_relay.log"
    echo "    fi"
    echo "    exit \$EXIT_CODE"
    echo "fi"
} > "$RUN_SCRIPT"

chmod +x "$RUN_SCRIPT"

log_message "SUCCESS" "Script de ejecución creado: $RUN_SCRIPT"

# Configurar cron para ejecutar cada 5 minutos
log_message "PROGRESS" "Configurando tarea programada (cron)..."

# Eliminar entradas existentes del relay
(crontab -l 2>/dev/null | grep -v "$RUN_SCRIPT") | crontab -

# Añadir nueva entrada de cron
(crontab -l 2>/dev/null; echo "*/5 * * * * $RUN_SCRIPT") | crontab -

log_message "SUCCESS" "Tarea programada configurada (cada 5 minutos)"

# Ofrecer ejecutar una prueba inmediata
echo ""
echo -e "${CYAN}${BOLD}╔══════════════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}${BOLD}║                          PRUEBA DE CONFIGURACIÓN                             ║${NC}"
echo -e "${CYAN}${BOLD}╚══════════════════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${WHITE}La instalación se ha completado. Ahora puede:${NC}"
echo ""
echo -e "${WHITE}  1. Ejecutar una prueba AHORA para verificar la configuración${NC}"
echo -e "${WHITE}  2. Omitir la prueba y dejar que el cron lo ejecute cada 5 minutos${NC}"
echo ""

if [[ "$SILENT_MODE" != "true" ]]; then
    echo -ne "${YELLOW}¿Desea ejecutar el relay por primera vez ahora para verificar? (Y/n): ${NC}"
    read -r RUN_TEST
    
    if [[ "$RUN_TEST" =~ ^[Yy]$ ]] || [[ -z "$RUN_TEST" ]]; then
        echo ""
        echo -e "${GREEN}${BOLD}Ejecutando relay por primera vez...${NC}"
        echo ""
        
        # Ejecutar el script que ahora mostrará diagnósticos en tiempo real
        $RUN_SCRIPT
        TEST_EXIT_CODE=$?
        
        echo ""
        if [ $TEST_EXIT_CODE -eq 0 ]; then
            echo -e "${GREEN}${BOLD}✓ Prueba exitosa! El relay está funcionando correctamente.${NC}"
        else
            echo -e "${RED}${BOLD}✗ La prueba falló. Por favor revise los errores mostrados arriba.${NC}"
            echo ""
            echo -e "${YELLOW}Sugerencias:${NC}"
            echo -e "${WHITE}  • Verifique las credenciales SNMP en: ${DIM}$INSTALL_DIR/configs/devices.conf${NC}"
            echo -e "${WHITE}  • Verifique que el dispositivo sea alcanzable${NC}"
            echo -e "${WHITE}  • Revise el log detallado: ${DIM}tail -f $INSTALL_DIR/logs/ness_relay.log${NC}"
            echo ""
            echo -e "${YELLOW}Para editar la configuración de forma segura:${NC}"
            echo -e "${DIM}  sudo $INSTALL_DIR/executables/view_config.sh${NC}"
        fi
    else
        echo ""
        echo -e "${WHITE}Prueba omitida. El relay se ejecutará automáticamente cada 5 minutos vía cron.${NC}"
    fi
else
    echo -e "${WHITE}Modo silencioso: omitiendo prueba interactiva.${NC}"
fi

# Mensaje final
echo ""
echo -e "${GREEN}${BOLD}"
cat << 'EOF'
╔══════════════════════════════════════════════════════════════════════════════╗
║                                                                              ║
║                    ✅  INSTALACIÓN COMPLETADA EXITOSAMENTE                   ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
EOF
echo -e "${NC}"

echo -e "${WHITE}${BOLD}📁 DETALLES DE LA INSTALACIÓN:${NC}"
echo -e "${WHITE}  • Directorio de instalación:${NC} ${BOLD}$INSTALL_DIR${NC}"
echo -e "${WHITE}  • Ejecutable:${NC}               ${DIM}$INSTALL_DIR/executables/$EXEC_NAME${NC}"
echo -e "${WHITE}  • Configuración:${NC}            ${DIM}$INSTALL_DIR/configs/devices.conf${NC}"
echo -e "${WHITE}  • Script de ejecución:${NC}      ${DIM}$RUN_SCRIPT${NC}"
echo -e "${WHITE}  • Log de ejecución:${NC}         ${DIM}$INSTALL_DIR/logs/ness_relay.log${NC}"
echo -e "${WHITE}  • Programación:${NC}             ${GREEN}Cada 5 minutos via cron${NC}"
echo ""
echo -e "${WHITE}${BOLD}🔒 SEGURIDAD:${NC}"
echo -e "${WHITE}  • Ver configuración protegida:${NC}  ${DIM}sudo $INSTALL_DIR/executables/view_config.sh${NC}"
echo -e "${WHITE}  • Contraseña requerida:${NC}        ${DIM}Use su NESS_API_TOKEN${NC}"
echo ""
echo -e "${WHITE}${BOLD}📋 COMANDOS ÚTILES:${NC}"
echo -e "${WHITE}  • ${BOLD}Ejecutar con diagnósticos:${NC} ${GREEN}sudo $RUN_SCRIPT${NC}"
echo -e "${WHITE}  • Ver configuración cron:${NC}     ${DIM}crontab -l | grep ness_relay${NC}"
echo -e "${WHITE}  • Ver logs en tiempo real:${NC}    ${DIM}tail -f $INSTALL_DIR/logs/ness_relay.log${NC}"
echo -e "${WHITE}  • Ver últimos errores:${NC}        ${DIM}tail -n 100 $INSTALL_DIR/logs/ness_relay.log | grep -i error${NC}"
echo -e "${WHITE}  • Ver estructura:${NC}             ${DIM}tree -L 2 $INSTALL_DIR${NC}"
echo ""

echo ""
echo -e "${GREEN}${BOLD}🎉 ¡INSTALACIÓN FINALIZADA EXITOSAMENTE!${NC}"
echo -e "${WHITE}   El relay está programado para ejecutarse cada 5 minutos.${NC}"
echo -e "${WHITE}   Para ver diagnósticos en tiempo real, ejecute: ${GREEN}sudo $RUN_SCRIPT${NC}"
echo -e "${DIM}   Gracias por usar NESS HQ Network Relay System${NC}"
echo ""
