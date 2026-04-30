#!/bin/bash

###############################################################################
# NESS HQ - RELAY Network Monitoring System v2.0.0 (Rust Edition)
# Script de instalación profesional para NESS Relay — Binario estático
#
# CARACTERÍSTICAS:
# - NO requiere instalar Python, Java ni dependencias
# - Ejecutable estático compilado con musl (zero dependencies)
# - Monitoreo multi-fabricante (Cisco, Fortinet, pfSense, MikroTik RouterOS,
#   MikroTik Firewalls, UBNT, Cambium, Windows, Linux)
# - Configuración múltiple de dispositivos por fabricante
# - Sistema de logs y reportes avanzado
# - Integración completa con NESS HQ Cloud
# - Programación automática cada 5 minutos
#
# Modo interactivo (recomendado):
#   sudo ./install_relay.sh
#
# Modo silencioso:
#   sudo ./install_relay.sh --silent --config-file connection.config --token TU_TOKEN --env 3
#
# IMPORTANTE: Este instalador requiere el binario 'ness-relay'
#             en el mismo directorio (o en dist/).
###############################################################################

# Proximos cambios
# Ajustar el design de la presentacion de la instalacion para que se pueda acomodar dinamicamente a cualquier pestaña
# Utilizar el design de NESS en la version 5

# Colores corporativos NESS — Nueva Paleta
WHITE='\033[1;37m'            # #FFFFFF - Color principal
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
VERIFY_SETUP_ONLY=false
CONFIG_FILE=""
API_TOKEN=""
SERVER_ENV=3  # Por defecto usamos Public Cloud
declare -A SELECTED_VENDORS
declare -A DEVICE_CONFIGS

# Nombre del ejecutable (binario estático Rust)
EXEC_NAME="ness-relay"

# --- Funciones de Utilidad para UI Responsive ---
get_term_width() {
    local width
    width=$(tput cols 2>/dev/null) || width=80
    [[ "${width:-0}" -lt 40 ]] && width=40
    echo "$width"
}

# Retorna una línea de `char` con el ancho del terminal (sin color)
_hline() {
    local char="${1:-═}"
    local width; width=$(get_term_width)
    local line=""; local i=0
    while [[ $i -lt $width ]]; do line+="$char"; ((i++)); done
    echo "$line"
}

# Imprime una línea separadora horizontal (incluye color)
print_line() {
    local char="${1:-═}"
    local color="${2:-${WHITE}}"
    echo -e "${color}$(_hline "$char")${NC}"
}

# Centra texto según el ancho actual del terminal
center_text() {
    local text="$1"
    local width; width=$(get_term_width)
    local text_len=${#text}
    local padding=$(( (width - text_len) / 2 ))
    [[ $padding -lt 0 ]] && padding=0
    printf "%${padding}s%s\n" "" "$text"
}

# Normaliza protocolos SNMPv3 a valores canónicos que entiende el motor Rust.
normalize_snmpv3_auth_protocol() {
    local raw_value="${1:-SHA}"
    local normalized="${raw_value//[[:space:]]/}"
    normalized="${normalized^^}"

    case "$normalized" in
        MD5)
            echo "MD5"
            ;;
        SHA|SHA1|SHA-1|SHA96|HMACSHA96|HMAC-SHA1-96|HMAC-SHA-96)
            echo "SHA"
            ;;
        SHA256|SHA-256|SHA2-256|SHA256-128|HMACSHA256|HMAC-SHA2-256-128)
            echo "SHA256"
            ;;
        SHA256-192|SHA256AUTH|SHA-256-192|HMAC-SHA2-256-192)
            echo "SHA256-192"
            ;;
        SHA384|SHA-384|SHA2-384|HMAC-SHA2-384-192)
            echo "SHA384"
            ;;
        SHA512|SHA-512|SHA2-512|HMAC-SHA2-512-256)
            echo "SHA512"
            ;;
        NONE)
            echo "NONE"
            ;;
        *)
            echo "SHA"
            ;;
    esac
}

normalize_snmpv3_priv_protocol() {
    local raw_value="${1:-AES128}"
    local normalized="${raw_value//[[:space:]]/}"
    normalized="${normalized^^}"

    case "$normalized" in
        AES|AES128|AES-128|AES-128-CFB|AES128CFB)
            echo "AES128"
            ;;
        AES192|AES-192|AES-192-CFB|AES192CFB)
            echo "AES192"
            ;;
        AES256|AES-256|AES-256-CFB|AES256CFB)
            echo "AES256"
            ;;
        DES|DES-CBC|CBC-DES)
            echo "DES"
            ;;
        NONE)
            echo "NONE"
            ;;
        *)
            echo "AES128"
            ;;
    esac
}

normalize_snmpv3_device_configs() {
    local vendor count device_count config_key snmp_version auth_key priv_key auth_pass_key priv_pass_key
    for vendor in "${VENDORS[@]}"; do
        count="${DEVICE_CONFIGS[${vendor}_count]:-0}"
        [[ "$count" =~ ^[0-9]+$ ]] || continue

        for ((device_count=1; device_count<=count; device_count++)); do
            config_key="${vendor}_${device_count}"
            snmp_version="${DEVICE_CONFIGS[${config_key}_snmp_version]:-2c}"
            [[ "$snmp_version" == "3" ]] || continue

            auth_key="${config_key}_v3_auth_protocol"
            priv_key="${config_key}_v3_priv_protocol"
            auth_pass_key="${config_key}_v3_auth_password"
            priv_pass_key="${config_key}_v3_priv_password"

            DEVICE_CONFIGS["$auth_key"]="$(normalize_snmpv3_auth_protocol "${DEVICE_CONFIGS[$auth_key]:-SHA}")"
            DEVICE_CONFIGS["$priv_key"]="$(normalize_snmpv3_priv_protocol "${DEVICE_CONFIGS[$priv_key]:-AES128}")"

            if [[ "${DEVICE_CONFIGS[$auth_key]}" == "NONE" ]]; then
                if [[ "${DEVICE_CONFIGS[$priv_key]}" != "NONE" ]]; then
                    log_message "WARNING" "${config_key}: privacidad desactivada porque SNMPv3 fue configurado sin autenticación"
                    DEVICE_CONFIGS["$priv_key"]="NONE"
                fi
                DEVICE_CONFIGS["$auth_pass_key"]=""
                DEVICE_CONFIGS["$priv_pass_key"]=""
            elif [[ "${DEVICE_CONFIGS[$priv_key]}" == "NONE" ]]; then
                DEVICE_CONFIGS["$priv_pass_key"]=""
            fi
        done
    done
}

# Imprime un box de 3 líneas (╔═╗ / ║título║ / ╚═╝) adaptado al ancho del terminal
print_box() {
    local title="$1"
    local color="${2:-${WHITE}${BOLD}}"
    local width; width=$(get_term_width)
    local inner=$((width - 2))
    local title_len=${#title}
    local pad_total=$((inner - title_len))
    [[ $pad_total -lt 2 ]] && pad_total=2
    local pad_left=$(( pad_total / 2 ))
    local pad_right=$(( pad_total - pad_left ))
    local top="╔" bottom="╚" mid="║"
    local i
    for ((i=0; i<inner; i++)); do top+="═"; bottom+="═"; done
    top+="╗"; bottom+="╝"
    for ((i=0; i<pad_left; i++)); do mid+=" "; done
    mid+="${title}"
    for ((i=0; i<pad_right; i++)); do mid+=" "; done
    mid+="║"
    echo -e "${color}${top}${NC}"
    echo -e "${color}${mid}${NC}"
    echo -e "${color}${bottom}${NC}"
}

# Definir fabricantes disponibles
# mikrotik_fw: vendor oculto en el menú principal, gestionado via sub-menú del grupo MikroTik
VENDORS=("windows" "linux" "cisco" "fortinet" "pfsense" "mikrotik" "ubnt" "c_n" "mikrotik_fw")
VENDOR_NAMES=("Windows Servers" "Linux Servers" "Cisco Devices" "Fortinet Firewalls" "pfSense Firewalls" "MikroTik Devices ▶" "Ubiquiti Switches (UBNT)" "Cambium Networks APs" "")
# Vendors visibles en el menú principal (excluye hidden entries con nombre vacío)
VISIBLE_VENDOR_COUNT=8

# Banner principal NESS — Adaptativo al ancho del terminal
show_banner() {
    clear
    local width; width=$(get_term_width)
    print_line "═" "${WHITE}${BOLD}"
    if [[ $width -ge 90 ]]; then
        printf "${WHITE}${BOLD}"
        center_text "███╗   ██╗███████╗███████╗███████╗    ██████╗ ███████╗██╗      █████╗ ██╗   ██╗"
        center_text "████╗  ██║██╔════╝██╔════╝██╔════╝    ██╔══██╗██╔════╝██║     ██╔══██╗╚██╗ ██╔╝"
        center_text "██╔██╗ ██║█████╗  ███████╗███████╗    ██████╔╝█████╗  ██║     ███████║ ╚████╔╝ "
        center_text "██║╚██╗██║██╔══╝  ╚════██║╚════██║    ██╔══██╗██╔══╝  ██║     ██╔══██║  ╚██╔╝  "
        center_text "██║ ╚████║███████╗███████║███████║    ██║  ██║███████╗███████╗██║  ██║   ██║   "
        center_text "╚═╝  ╚═══╝╚══════╝╚══════╝╚══════╝    ╚═╝  ╚═╝╚══════╝╚══════╝╚═╝  ╚═╝   ╚═╝   "
        printf "${NC}\n"
    else
        printf "${WHITE}${BOLD}"; center_text "[ NESS RELAY ]"; printf "${NC}\n"
    fi
    printf "${CYAN}";      center_text "🌐  NETWORK RELAY MONITORING SYSTEM  🌐";      printf "${NC}"
    printf "${WHITE}";     center_text "Professional Multi-Vendor Edition v2.0.0  |  ⚙️  Rust Static Binary"; printf "${NC}"
    printf "${WHITE}${DIM}"; center_text "NETWORK IS COLOMBIA S.A.S  |  © 2026  Todos los derechos reservados"; printf "${NC}\n"
    print_line "═" "${WHITE}${BOLD}"
    echo ""
}

# Función para mostrar el disclaimer de seguridad y términos de uso
show_security_disclaimer() {
    echo ""
    print_box "⚖️  TÉRMINOS DE USO, LICENCIA Y PRIVACIDAD  ⚖️" "${YELLOW}${BOLD}"
    echo ""
    echo -e "${WHITE}${BOLD}Al continuar con esta instalación, usted acepta formalmente que:${NC}"
    echo ""
    echo -e "  ${YELLOW}1.${NC} ${BOLD}Propiedad Intelectual:${NC} Este software es propiedad exclusiva de ${CYAN}NETWORK IS COLOMBIA S.A.S${NC}."
    echo -e "     Queda estrictamente prohibida su copia, ingeniería inversa, redistribución o modificación"
    echo -e "     sin autorización expresa del titular. El incumplimiento implica consecuencias legales"
    echo -e "     bajo las Leyes 23 de 1982 y 44 de 1993, así como tratados internacionales."
    echo ""
    echo -e "  ${YELLOW}2.${NC} ${BOLD}Uso Autorizado:${NC} Usted garantiza poseer los permisos legales para monitorear los"
    echo -e "     dispositivos que configure. El uso indebido para espionaje, acceso no autorizado"
    echo -e "     o actividades ilegales es de exclusiva responsabilidad del usuario."
    echo ""
    echo -e "  ${YELLOW}3.${NC} ${BOLD}Privacidad de Datos:${NC} El agente recolecta métricas de rendimiento y estado de red,"
    echo -e "     transmitidas de forma cifrada a ${CYAN}NESS HQ Cloud${NC}. El usuario es responsable de"
    echo -e "     cumplir con las leyes de protección de datos aplicables en su jurisdicción."
    echo ""
    echo -e "  ${YELLOW}4.${NC} ${BOLD}Limitación de Garantía:${NC} El software se entrega '${DIM}TAL CUAL${NC}', sin garantías expresas"
    echo -e "     ni implícitas. ${CYAN}NETWORK IS COLOMBIA S.A.S${NC} no asume responsabilidad por daños"
    echo -e "     directos, indirectos o pérdida de datos derivados de su uso o configuración."
    echo ""
    echo -e "  ${YELLOW}5.${NC} ${BOLD}Soporte y Contacto:${NC} ${CYAN}https://nesshq.com${NC}"
    #echo -e "  ${YELLOW}5.${NC} ${BOLD}Soporte y Contacto:${NC} ${WHITE}https://nesshq.com${NC}  |  ${CYAN}https://soporte.nesshq.com${NC}"
    echo ""
    print_line "─" "${WHITE}"
    echo ""
    while true; do
        echo -ne "${GREEN}${BOLD}¿Acepta los términos para proceder? (ESCRIBA: ACEPTO / rechazo): ${NC}"
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
    print_box "SELECCIÓN DE FABRICANTES/DISPOSITIVOS" "${WHITE}${BOLD}"
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
        print_box "DISPOSITIVOS MIKROTIK" "${WHITE}${BOLD}"
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
    print_box "CONFIGURACIÓN: $vendor_name" "${WHITE}${BOLD}"
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
                device_count=$((device_count - 1))
                break 2
            elif [[ -n "$device_ip" ]]; then
                break
            else
                echo -e "${RED}  ❌ Error: La IP no puede estar vacía${NC}"
            fi
        done

        # Preguntar versión de SNMP
        echo ""
        echo -e "${WHITE}${BOLD}  Selecciona la versión de SNMP:${NC}"
        echo -e "    ${WHITE}1)${NC} SNMPv1  ${DIM}(Community string — protocolo legacy, sin cifrado)${NC}"
        echo -e "    ${WHITE}2)${NC} SNMPv2c ${DIM}(Community string — mejor rendimiento, sin cifrado)${NC}"
        echo -e "    ${WHITE}3)${NC} SNMPv3  ${DIM}(Usuario/Contraseña — ${WHITE}${BOLD}RECOMENDADO${NC}${DIM}: con autenticación y cifrado)${NC}"
        echo -ne "${WHITE}  Selecciona 1, 2 o 3 [default: 3]: ${NC}"
        read snmp_version_choice
        snmp_version_choice=${snmp_version_choice:-3}

        if [[ "$snmp_version_choice" == "1" ]]; then
            # Configuración SNMPv1
            snmp_version="1"
            echo -e "${YELLOW}  ⚠️  SNMPv1 seleccionado — Protocolo legacy sin seguridad${NC}"
            echo -ne "${WHITE}  🔑 Community string SNMP [default: public]: ${NC}"
            read community
            community=${community:-public}
        elif [[ "$snmp_version_choice" == "2" ]]; then
            # Configuración SNMPv2c
            snmp_version="2c"
            echo -e "${YELLOW}  ⚠️  SNMPv2c seleccionado — Sin cifrado de datos${NC}"
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
            echo -e "    ${WHITE}1)${NC} SHA ${DIM}(HMAC-SHA1-96, recomendado)${NC}"
            echo -e "    ${WHITE}2)${NC} MD5 ${DIM}(HMAC-MD5-96)${NC}"
            echo -e "    ${WHITE}3)${NC} SHA256 ${DIM}(HMAC-SHA2-256-128)${NC}"
            echo -e "    ${WHITE}4)${NC} SHA256-192 ${DIM}(HMAC-SHA2-256-192, compatibilidad)${NC}"
            echo -e "    ${WHITE}5)${NC} SHA384 ${DIM}(HMAC-SHA2-384-192)${NC}"
            echo -e "    ${WHITE}6)${NC} SHA512 ${DIM}(HMAC-SHA2-512-256)${NC}"
            echo -e "    ${WHITE}7)${NC} NONE ${DIM}(sin autenticación — no usar con privacidad)${NC}"
            echo -ne "${WHITE}  Selecciona 1-7 [default: 1]: ${NC}"
            read auth_choice
            auth_choice=${auth_choice:-1}

            case "$auth_choice" in
                1) v3_auth_protocol="SHA" ;;
                2) v3_auth_protocol="MD5" ;;
                3) v3_auth_protocol="SHA256" ;;
                4) v3_auth_protocol="SHA256-192" ;;
                5) v3_auth_protocol="SHA384" ;;
                6) v3_auth_protocol="SHA512" ;;
                7) v3_auth_protocol="NONE" ;;
                *) v3_auth_protocol="SHA" ;;
            esac
            v3_auth_protocol=$(normalize_snmpv3_auth_protocol "$v3_auth_protocol")

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
            echo -e "    ${WHITE}1)${NC} AES128 ${DIM}(AES-128-CFB, recomendado)${NC}"
            echo -e "    ${WHITE}2)${NC} AES192 ${DIM}(AES-192-CFB)${NC}"
            echo -e "    ${WHITE}3)${NC} AES256 ${DIM}(AES-256-CFB, máxima seguridad)${NC}"
            echo -e "    ${WHITE}4)${NC} DES ${DIM}(DES-CBC, obsoleto)${NC}"
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
            v3_priv_protocol=$(normalize_snmpv3_priv_protocol "$v3_priv_protocol")

            if [[ "$v3_auth_protocol" == "NONE" && "$v3_priv_protocol" != "NONE" ]]; then
                echo -e "${YELLOW}  ⚠️  La privacidad SNMPv3 requiere autenticación; se desactivará la privacidad para mantener una configuración funcional.${NC}"
                v3_priv_protocol="NONE"
            fi

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
            echo -e "${YELLOW}  ⚠️  Recordatorio: SNMPv1 no proporciona seguridad — considere actualizar${NC}"
        elif [[ "$snmp_version" == "2c" ]]; then
            DEVICE_CONFIGS["${config_key}_community"]="$community"
            echo -e "${GREEN}  ✅ Dispositivo SNMPv2c configurado: ${BOLD}$device_ip${NC} ${DIM}($description)${NC}"
            echo -e "${YELLOW}  ⚠️  Recordatorio: SNMPv2c no cifra datos — considere SNMPv3${NC}"
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
                    [[ -z "$name" ]] && continue
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
                    for vendor in "${VENDORS[@]}"; do
                        SELECTED_VENDORS["$vendor"]="false"
                    done
                    log_message "INFO" "Todos los fabricantes deseleccionados"
                else
                    for i in "${!VENDORS[@]}"; do
                        vendor="${VENDORS[$i]}"
                        vendor_name="${VENDOR_NAMES[$i]}"
                        [[ -z "$vendor_name" ]] && continue
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
        if [[ "$key" =~ ^([a-z_]+)_([0-9]+)_(.+)$ ]]; then
            vendor="${BASH_REMATCH[1]}"
            field="${BASH_REMATCH[3]}"

            case "$field" in
                v3_auth_protocol)
                    value="$(normalize_snmpv3_auth_protocol "$value")"
                    ;;
                v3_priv_protocol)
                    value="$(normalize_snmpv3_priv_protocol "$value")"
                    ;;
            esac

            DEVICE_CONFIGS["$key"]="$value"
            SELECTED_VENDORS["$vendor"]="true"
        elif [[ "$key" =~ ^([a-z_]+)_count$ ]]; then
            DEVICE_CONFIGS["$key"]="$value"
        fi
    done < "$config_file"
}

# Función para generar archivo de configuración
generate_config_file() {
    local config_file="$INSTALL_DIR/configs/connection.config"

    {
        echo "# ═══════════════════════════════════════════════════════════"
        echo "# NESS RELAY — Configuración de Dispositivos"
        echo "# Generado automáticamente el $(date)"
        echo "# ═══════════════════════════════════════════════════════════"
        echo "# "
        echo "# Soporta SNMPv1, SNMPv2c y SNMPv3"
        echo "# "
        echo "# Para SNMPv1 (Legacy — sin seguridad):"
        echo "#   <vendor>_<num>_snmp_version=1"
        echo "#   <vendor>_<num>_community=<string>"
        echo "# "
        echo "# Para SNMPv2c (Sin cifrado):"
        echo "#   <vendor>_<num>_snmp_version=2c"
        echo "#   <vendor>_<num>_community=<string>"
        echo "# "
        echo "# Para SNMPv3 (RECOMENDADO — con autenticación y cifrado):"
        echo "#   <vendor>_<num>_snmp_version=3"
        echo "#   <vendor>_<num>_v3_user=<username>"
        echo "#   <vendor>_<num>_v3_auth_protocol=SHA|MD5|SHA256|SHA256-192|SHA384|SHA512|NONE"
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
                        # SNMPv1 o SNMPv2c — solo requieren community string
                        echo "${config_key}_community=${DEVICE_CONFIGS[${config_key}_community]}"
                    else
                        # SNMPv3 — requiere credenciales completas
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

###############################################################################
# PROCESAMIENTO DE ARGUMENTOS
###############################################################################
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
        --verify-setup)
            VERIFY_SETUP_ONLY=true
            shift
            ;;
        --help)
            echo "Uso: sudo ./install_relay.sh [opciones]"
            echo ""
            echo "Opciones:"
            echo "  --silent               Instalar en modo silencioso (sin menús)"
            echo "  --config-file FILE     Usar archivo de configuración existente"
            echo "  --force                Forzar instalación sobre existente"
            echo "  --token TOKEN          Token de API de NESS HQ"
            echo "  --env ENV_ID           ID del servidor (1=On-premise, 2=Testing, 3=Cloud)"
            echo "  --verify-setup         Ejecutar solo Smart Tester y salir"
            echo "  --help                 Mostrar esta ayuda"
            echo ""
            echo "Modo interactivo (recomendado):"
            echo "  sudo ./install_relay.sh"
            echo ""
            echo "Modo silencioso:"
            echo "  sudo ./install_relay.sh --silent --config-file connection.config --token TU_TOKEN --env 3"
            echo ""
            echo "Ejemplo de archivo de configuración:"
            echo "  pfsense_count=1"
            echo "  pfsense_1_ip=192.168.1.1"
            echo "  pfsense_1_community=public"
            echo "  pfsense_1_snmp_version=2c"
            echo "  pfsense_1_port=161"
            echo "  pfsense_1_vendor=pfsense"
            echo "  pfsense_1_description=Firewall Principal"
            exit 0
            ;;
        *)
            echo "Opción desconocida: $1"
            echo "Use --help para ver opciones disponibles"
            exit 1
            ;;
    esac
done

###############################################################################
# INICIO DE LA INSTALACIÓN
###############################################################################

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

# Verificar que el ejecutable existe en el directorio actual o en dist/
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
log_message "PROGRESS" "Verificando ejecutable..."

HOST_ARCH_RAW="$(uname -m 2>/dev/null || echo unknown)"
case "$HOST_ARCH_RAW" in
    x86_64|amd64)
        HOST_ARCH_SUFFIX="x86_64"
        ;;
    aarch64|arm64)
        HOST_ARCH_SUFFIX="aarch64"
        ;;
    *)
        HOST_ARCH_SUFFIX=""
        ;;
esac

declare -a BINARY_CANDIDATES
if [[ -n "$HOST_ARCH_SUFFIX" ]]; then
    BINARY_CANDIDATES+=("${EXEC_NAME}-${HOST_ARCH_SUFFIX}")
fi
BINARY_CANDIDATES+=("${EXEC_NAME}" "${EXEC_NAME}-x86_64" "${EXEC_NAME}-aarch64")

BINARY_SOURCE=""
BINARY_NAME_SELECTED=""

# Buscar primero en dist/ y luego en el directorio del script.
for candidate in "${BINARY_CANDIDATES[@]}"; do
    if [[ -f "${SCRIPT_DIR}/dist/${candidate}" ]]; then
        BINARY_SOURCE="${SCRIPT_DIR}/dist/${candidate}"
        BINARY_NAME_SELECTED="$candidate"
        break
    fi

    if [[ -f "${SCRIPT_DIR}/${candidate}" ]]; then
        BINARY_SOURCE="${SCRIPT_DIR}/${candidate}"
        BINARY_NAME_SELECTED="$candidate"
        break
    fi
done

if [[ -z "$BINARY_SOURCE" ]]; then
    log_message "ERROR" "No se encuentra un binario compatible en este directorio ni en dist/"
    echo ""
    echo -e "${YELLOW}${BOLD}Asegúrese de:${NC}"
    echo -e "  ${WHITE}1.${NC} Haber compilado el agente con ${CYAN}build_relay.sh${NC}"
    echo -e "  ${WHITE}2.${NC} El binario compilado debe estar en ${CYAN}dist/${EXEC_NAME}${NC}, ${CYAN}dist/${EXEC_NAME}-x86_64${NC}, ${CYAN}dist/${EXEC_NAME}-aarch64${NC} o en este directorio"
    echo ""
    echo -e "${YELLOW}${BOLD}Ejemplo de compilación:${NC}"
    echo -e "  ${WHITE}./build_relay.sh --arch x86_64 --release${NC}"
    echo -e "  ${WHITE}./build_relay.sh --arch aarch64 --release${NC}"
    echo ""
    exit 1
fi

log_message "SUCCESS" "Ejecutable '${BINARY_NAME_SELECTED}' encontrado: ${BINARY_SOURCE}"
echo ""

###############################################################################
# MODO SMART TESTER DIRECTO (SIN INSTALACIÓN)
###############################################################################
if [[ "$VERIFY_SETUP_ONLY" == "true" ]]; then
    log_message "PROGRESS" "Modo --verify-setup detectado: ejecutando Smart Tester y saliendo..."

    VERIFY_CMD=("$BINARY_SOURCE" "--verify-setup")
    if [[ -n "$CONFIG_FILE" ]]; then
        VERIFY_CMD+=("--config" "$CONFIG_FILE")
    fi

    # Si se ejecuta como root desde el instalador, permitimos auto-fix y modo no interactivo.
    if [[ "$EUID" -eq 0 ]]; then
        VERIFY_CMD+=("--verify-auto-fix" "--verify-assume-yes")
    fi

    "${VERIFY_CMD[@]}"
    EXIT_CODE=$?
    if [[ $EXIT_CODE -eq 0 ]]; then
        log_message "SUCCESS" "Smart Tester finalizado correctamente"
    else
        log_message "WARNING" "Smart Tester finalizó con advertencias o errores (código $EXIT_CODE)"
    fi
    exit $EXIT_CODE
fi

###############################################################################
# SMART TESTER PRE-FLIGHT (POST-TÉRMINOS)
###############################################################################
AUTOCOMPLETE_FILE="/tmp/ness_smart_tester_autocomplete.conf"
AUTOCOMPLETE_USED=false

if [[ "$SILENT_MODE" != "true" ]]; then
    print_box "SMART TESTER — PRE-FLIGHT" "${CYAN}${BOLD}"
    echo ""
    echo -e "${WHITE}Recomendado: ejecutar diagnóstico inteligente antes de continuar.${NC}"
    echo -e "${DIM}Validará readiness del sistema, cron y salud base de red.${NC}"
    echo ""
    echo -e "${WHITE}[Fase A]${NC} System Readiness"
    echo -e "${WHITE}[Fase B]${NC} Network Health"
    echo -e "${WHITE}[Fase C]${NC} Deep SNMP Validation"
    echo -e "${WHITE}[Fase D]${NC} Local Firewall Checker"
    echo ""
    echo -ne "${YELLOW}¿Desea ejecutar Smart Tester ahora? (Y/n): ${NC}"
    read -r RUN_SMART_TESTER_PREFLIGHT

    if [[ "$RUN_SMART_TESTER_PREFLIGHT" =~ ^[Yy]$ ]] || [[ -z "$RUN_SMART_TESTER_PREFLIGHT" ]]; then
        log_message "PROGRESS" "Ejecutando Smart Tester pre-flight..."
        "$BINARY_SOURCE" --verify-setup --verify-auto-fix || true
        echo ""
        log_message "SUCCESS" "Smart Tester pre-flight finalizado"
    else
        log_message "INFO" "Smart Tester pre-flight omitido por el usuario"
    fi
else
    log_message "PROGRESS" "Modo silencioso: ejecutando Smart Tester pre-flight no interactivo..."
    "$BINARY_SOURCE" --verify-setup --verify-auto-fix --verify-assume-yes || true
fi

###############################################################################
# AUTOCOMPLETADO CON DATOS DEL SMART TESTER
###############################################################################
# Si el Smart Tester validó un dispositivo manualmente, ofrecer autocompletado
if [[ "$SILENT_MODE" != "true" && -f "$AUTOCOMPLETE_FILE" ]]; then
    echo ""
    print_box "AUTOCOMPLETADO DISPONIBLE" "${GREEN}${BOLD}"
    echo ""

    # Leer datos del archivo de autocompletado
    AC_IP=""
    AC_PORT=""
    AC_SNMP_VERSION=""
    AC_COMMUNITY=""
    AC_V3_USER=""
    AC_V3_AUTH_PROTOCOL=""
    AC_V3_AUTH_PASSWORD=""
    AC_V3_PRIV_PROTOCOL=""
    AC_V3_PRIV_PASSWORD=""
    while IFS='=' read -r ac_key ac_value; do
        [[ -z "$ac_key" || "$ac_key" =~ ^[[:space:]]*# ]] && continue
        ac_key=$(echo "$ac_key" | tr -d '[:space:]')
        case "$ac_key" in
            ip)                 AC_IP="$ac_value" ;;
            port)               AC_PORT="$ac_value" ;;
            snmp_version)       AC_SNMP_VERSION="$ac_value" ;;
            community)          AC_COMMUNITY="$ac_value" ;;
            v3_user)            AC_V3_USER="$ac_value" ;;
            v3_auth_protocol)   AC_V3_AUTH_PROTOCOL="$ac_value" ;;
            v3_auth_password)   AC_V3_AUTH_PASSWORD="$ac_value" ;;
            v3_priv_protocol)   AC_V3_PRIV_PROTOCOL="$ac_value" ;;
            v3_priv_password)   AC_V3_PRIV_PASSWORD="$ac_value" ;;
        esac
    done < "$AUTOCOMPLETE_FILE"

    echo -e "${WHITE}El Smart Tester validó correctamente la conexión SNMP con:${NC}"
    echo -e "  ${WHITE}IP:${NC} ${BOLD}$AC_IP${NC}"
    echo -e "  ${WHITE}Puerto:${NC} $AC_PORT"
    echo -e "  ${WHITE}SNMP:${NC} v$AC_SNMP_VERSION"
    if [[ "$AC_SNMP_VERSION" == "3" ]]; then
        echo -e "  ${WHITE}Usuario:${NC} $AC_V3_USER"
        echo -e "  ${WHITE}Auth:${NC} $AC_V3_AUTH_PROTOCOL"
        echo -e "  ${WHITE}Priv:${NC} $AC_V3_PRIV_PROTOCOL"
    fi
    echo ""
    echo -e "${WHITE}Puede reutilizar estos datos para ahorrar tiempo en la configuración.${NC}"
    echo -e "${DIM}Solo necesitará seleccionar: servidor, token API y tipo de dispositivo.${NC}"
    echo ""
    echo -ne "${YELLOW}${BOLD}¿Desea usar el autocompletado con los datos del Smart Tester? (Y/n): ${NC}"
    read -r USE_AUTOCOMPLETE

    if [[ "$USE_AUTOCOMPLETE" =~ ^[Yy]$ ]] || [[ -z "$USE_AUTOCOMPLETE" ]]; then
        AUTOCOMPLETE_USED=true
        log_message "SUCCESS" "Autocompletado activado con datos del Smart Tester"

        # --- Paso 1: Seleccionar servidor ---
        echo ""
        print_box "CONFIGURACIÓN DEL SERVIDOR" "${WHITE}${BOLD}"
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

        # --- Paso 2: Token API ---
        echo ""
        while [[ -z "$API_TOKEN" ]]; do
            echo -ne "${BOLD}🔑 Ingresa tu NESS_API_TOKEN: ${NC}"
            read API_TOKEN
            if [[ -z "$API_TOKEN" ]]; then
                log_message "ERROR" "El token es obligatorio"
            fi
        done
        log_message "SUCCESS" "Token de API configurado"

        # --- Paso 3: Seleccionar tipo de dispositivo (vendor) ---
        echo ""
        print_box "SELECCIÓN DE DISPOSITIVO" "${WHITE}${BOLD}"
        echo ""
        echo -e "${WHITE}Selecciona el tipo de dispositivo para ${BOLD}$AC_IP${NC}${WHITE}:${NC}"
        echo ""
        echo -e "  ${WHITE}1)${NC} Windows Servers"
        echo -e "  ${WHITE}2)${NC} Linux Servers"
        echo -e "  ${WHITE}3)${NC} Cisco Devices"
        echo -e "  ${WHITE}4)${NC} Fortinet Firewalls"
        echo -e "  ${WHITE}5)${NC} pfSense Firewalls"
        echo -e "  ${WHITE}6)${NC} MikroTik Devices ${WHITE}▶${NC}"
        echo -e "  ${WHITE}7)${NC} Ubiquiti Switches (UBNT)"
        echo -e "  ${WHITE}8)${NC} Cambium Networks APs"
        echo ""
        echo -ne "${BOLD}Selecciona 1-8: ${NC}"
        read VENDOR_CHOICE
        case "$VENDOR_CHOICE" in
            1) AC_VENDOR="windows" ;;
            2) AC_VENDOR="linux" ;;
            3) AC_VENDOR="cisco" ;;
            4) AC_VENDOR="fortinet" ;;
            5) AC_VENDOR="pfsense" ;;
            6)
                echo ""
                echo -e "${WHITE}Selecciona el tipo de MikroTik:${NC}"
                echo -e "  ${WHITE}a)${NC} MikroTik RouterOS"
                echo -e "  ${WHITE}b)${NC} MikroTik Firewalls"
                echo ""
                echo -ne "${BOLD}Selecciona a o b [default: a]: ${NC}"
                read MK_CHOICE
                case "$MK_CHOICE" in
                    b|B) AC_VENDOR="mikrotik_fw" ;;
                    *)   AC_VENDOR="mikrotik" ;;
                esac
                ;;
            7) AC_VENDOR="ubnt" ;;
            8) AC_VENDOR="c_n" ;;
            *) AC_VENDOR="generic"; echo -e "${YELLOW}Opción no reconocida, usando 'generic'.${NC}" ;;
        esac

        # Solicitar descripción opcional
        echo ""
        echo -ne "${WHITE}  📝 Descripción del dispositivo [opcional]: ${NC}"
        read AC_DESCRIPTION

        log_message "SUCCESS" "Dispositivo configurado: $AC_VENDOR ($AC_IP)"

        # --- Poblar DEVICE_CONFIGS con datos del autocompletado ---
        AC_CONFIG_KEY="${AC_VENDOR}_1"
        SELECTED_VENDORS["$AC_VENDOR"]="true"
        DEVICE_CONFIGS["${AC_VENDOR}_count"]="1"
        DEVICE_CONFIGS["${AC_CONFIG_KEY}_ip"]="$AC_IP"
        DEVICE_CONFIGS["${AC_CONFIG_KEY}_port"]="${AC_PORT:-161}"
        DEVICE_CONFIGS["${AC_CONFIG_KEY}_description"]="${AC_DESCRIPTION}"
        DEVICE_CONFIGS["${AC_CONFIG_KEY}_vendor"]="$AC_VENDOR"
        DEVICE_CONFIGS["${AC_CONFIG_KEY}_snmp_version"]="$AC_SNMP_VERSION"

        if [[ "$AC_SNMP_VERSION" == "1" || "$AC_SNMP_VERSION" == "2c" ]]; then
            DEVICE_CONFIGS["${AC_CONFIG_KEY}_community"]="${AC_COMMUNITY:-public}"
        elif [[ "$AC_SNMP_VERSION" == "3" ]]; then
            DEVICE_CONFIGS["${AC_CONFIG_KEY}_v3_user"]="$AC_V3_USER"
            DEVICE_CONFIGS["${AC_CONFIG_KEY}_v3_auth_protocol"]="$AC_V3_AUTH_PROTOCOL"
            DEVICE_CONFIGS["${AC_CONFIG_KEY}_v3_auth_password"]="$AC_V3_AUTH_PASSWORD"
            DEVICE_CONFIGS["${AC_CONFIG_KEY}_v3_priv_protocol"]="$AC_V3_PRIV_PROTOCOL"
            DEVICE_CONFIGS["${AC_CONFIG_KEY}_v3_priv_password"]="$AC_V3_PRIV_PASSWORD"
        fi
    fi

    # Limpiar archivo de autocompletado (contiene contraseñas)
    rm -f "$AUTOCOMPLETE_FILE"
fi

# Selección de fabricantes (flujo normal, solo si no se usó autocompletado)
if [[ "$AUTOCOMPLETE_USED" == "true" ]]; then
    log_message "INFO" "Configuración completada vía autocompletado del Smart Tester"
elif [[ "$SILENT_MODE" == "true" && -n "$CONFIG_FILE" ]]; then
    load_config_file "$CONFIG_FILE"
elif [[ "$SILENT_MODE" != "true" ]]; then
    # Configurar servidor NESS
    print_box "CONFIGURACIÓN DEL SERVIDOR" "${WHITE}${BOLD}"
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

normalize_snmpv3_device_configs

# Guardar solo el ID del servidor (el ejecutable tiene las URLs hardcodeadas)
# Esto mejora la seguridad al no exponer las rutas de los endpoints
case "$SERVER_ENV" in
    "1")
        SERVER_ID="1"
        log_message "SUCCESS" "Servidor On-premise seleccionado"
        ;;
    "2")
        SERVER_ID="2"
        log_message "SUCCESS" "Servidor de Testing seleccionado"
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
print_box "RESUMEN DE CONFIGURACIÓN" "${WHITE}${BOLD}"
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

###############################################################################
# GESTIÓN DE INSTALACIÓN EXISTENTE
###############################################################################
INSTALL_DIR="/opt/ness_relay"
if [[ -d "$INSTALL_DIR" && "$FORCE_INSTALL" != "true" ]]; then
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
            log_message "WARNING" "Reinstalación completa seleccionada"

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

            log_message "PROGRESS" "Eliminando instalación anterior..."
            rm -rf "$INSTALL_DIR"
            log_message "SUCCESS" "Instalación anterior eliminada"
            ;;

        2)
            log_message "WARNING" "Actualización de configuración seleccionada"

            BACKUP_DATE=$(date '+%Y%m%d_%H%M%S')
            BACKUP_DIR="/opt/ness_relay_backup_${BACKUP_DATE}"

            log_message "PROGRESS" "Creando backup de configuraciones existentes..."
            mkdir -p "$BACKUP_DIR"

            # Backup de archivos críticos (soporta estructura legacy y nueva)
            [[ -f "$INSTALL_DIR/connection.config" ]] && cp "$INSTALL_DIR/connection.config" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/configs/connection.config" ]] && cp "$INSTALL_DIR/configs/connection.config" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/install.log" ]] && cp "$INSTALL_DIR/install.log" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/logs/install.log" ]] && cp "$INSTALL_DIR/logs/install.log" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/relay.log" ]] && cp "$INSTALL_DIR/relay.log" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "$INSTALL_DIR/logs/relay.log" ]] && cp "$INSTALL_DIR/logs/relay.log" "$BACKUP_DIR/" 2>/dev/null
            [[ -f "/etc/profile.d/ness_relay.sh" ]] && cp "/etc/profile.d/ness_relay.sh" "$BACKUP_DIR/" 2>/dev/null

            log_message "SUCCESS" "Backup de configuraciones creado en: $BACKUP_DIR"
            echo ""
            echo -e "${GREEN}${BOLD}✅ Backup completado${NC}"
            echo -e "${WHITE}  • Ubicación: ${BOLD}$BACKUP_DIR${NC}"
            echo -e "${WHITE}  • Contenido: connection.config, logs y variables de entorno${NC}"
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

###############################################################################
# CREAR ESTRUCTURA DE DIRECTORIOS
###############################################################################
log_message "PROGRESS" "Creando estructura de directorios organizada..."
mkdir -p "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR/configs"
mkdir -p "$INSTALL_DIR/devices"
mkdir -p "$INSTALL_DIR/executables"
mkdir -p "$INSTALL_DIR/logs"
mkdir -p "$INSTALL_DIR/data"

log_message "SUCCESS" "Estructura de directorios creada:"
echo -e "${WHITE}  ├── configs/     ${DIM}(Archivos de configuración)${NC}"
echo -e "${WHITE}  ├── devices/     ${DIM}(Datos JSON por vendor: devices/<tipo>_<vendor>/output/)${NC}"
echo -e "${WHITE}  ├── executables/ ${DIM}(Binario y scripts de ejecución)${NC}"
echo -e "${WHITE}  └── logs/        ${DIM}(Logs de instalación y operación)${NC}"
echo ""

# Mover el log temporal al directorio de logs
if [[ -f "/tmp/ness_relay_install.log" ]]; then
    mv "/tmp/ness_relay_install.log" "$INSTALL_DIR/logs/install.log" 2>/dev/null
fi

###############################################################################
# INSTALAR BINARIO
###############################################################################
log_message "PROGRESS" "Copiando ejecutable..."
cp "${BINARY_SOURCE}" "$INSTALL_DIR/executables/$EXEC_NAME"
chmod +x "$INSTALL_DIR/executables/$EXEC_NAME"
log_message "SUCCESS" "Ejecutable instalado en: $INSTALL_DIR/executables/$EXEC_NAME"

###############################################################################
# CONFIGURAR VARIABLES DE ENTORNO
###############################################################################
ENV_FILE="/etc/profile.d/ness_relay.sh"
log_message "PROGRESS" "Configurando variables de entorno..."
{
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# NESS RELAY — Variables de Entorno (Rust Edition)"
    echo "# Generado automáticamente el $(date)"
    echo "# NOTA: SERVER_ID es un identificador interno (1=On-premise, 2=Testing, 3=Cloud)"
    echo "# Las URLs reales están protegidas dentro del ejecutable compilado"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""
    echo "export NESS_SERVER_ID=\"$SERVER_ID\""
    echo "export NESS_API_TOKEN=\"$API_TOKEN\""
    echo "export NESS_INSTALL_DIR=\"$INSTALL_DIR\""
    echo "export NESS_DEVICES_FILE=\"$INSTALL_DIR/configs/connection.config\""
    echo "export NESS_OUTPUT_DIR=\"$INSTALL_DIR/output\""
    echo "export NESS_LOG_DIR \"$INSTALL_DIR/logs\""
    # License key for MaxMind GeoLite2 (optional). Set NESS_MAXMIND_LICENSE_KEY env to enable automatic download.
    echo "export NESS_MAXMIND_LICENSE_KEY \"${NESS_MAXMIND_LICENSE_KEY:-}\""
} > "$ENV_FILE"
chmod +x "$ENV_FILE"
source "$ENV_FILE"
log_message "SUCCESS" "Variables de entorno configuradas en: $ENV_FILE"

###############################################################################
# GENERAR ARCHIVO DE CONFIGURACIÓN DE DISPOSITIVOS
###############################################################################
generate_config_file

# Descargar GeoLite2 DBs (City + ASN) si hay license key disponible
download_geolite2() {
    local dest="$INSTALL_DIR/data"
    mkdir -p "$dest"

    local key="${NESS_MAXMIND_LICENSE_KEY:-}"
    if [[ -z "$key" ]]; then
        if [[ "$SILENT_MODE" == "true" ]]; then
            log_message "INFO" "NESS_MAXMIND_LICENSE_KEY no está definido; omitiendo descarga de GeoLite2"
            return 0
        fi
        echo ""
        echo -ne "Introduce la MaxMind License Key para descargar GeoLite2 (ENTER para omitir): "
        read -r input_key
        if [[ -z "$input_key" ]]; then
            log_message "INFO" "Descarga de GeoLite2 omitida por el usuario"
            return 0
        fi
        key="$input_key"
    fi

    for edition in "GeoLite2-City" "GeoLite2-ASN"; do
        local url="https://download.maxmind.com/app/geoip_download?edition_id=${edition}&license_key=${key}&suffix=tar.gz"
        log_message "PROGRESS" "Descargando ${edition} desde MaxMind..."
        tmpdir=$(mktemp -d)
        if command -v curl >/dev/null 2>&1; then
            curl -fsSL -o "$tmpdir/${edition}.tar.gz" "$url" || { log_message "ERROR" "Error descargando ${edition}"; rm -rf "$tmpdir"; continue; }
        elif command -v wget >/dev/null 2>&1; then
            wget -qO "$tmpdir/${edition}.tar.gz" "$url" || { log_message "ERROR" "Error descargando ${edition}"; rm -rf "$tmpdir"; continue; }
        else
            log_message "ERROR" "curl o wget no están disponibles; no se puede descargar GeoLite2"
            rm -rf "$tmpdir"
            return 1
        fi

        tar -xzf "$tmpdir/${edition}.tar.gz" -C "$tmpdir" || { log_message "ERROR" "Error extrayendo ${edition}"; rm -rf "$tmpdir"; continue; }
        mmdb_file=$(find "$tmpdir" -type f -name "*.mmdb" | head -n 1)
        if [[ -n "$mmdb_file" ]]; then
            cp "$mmdb_file" "$dest/${edition}.mmdb" || { log_message "ERROR" "No se pudo copiar ${edition} al destino"; }
            log_message "SUCCESS" "Base ${edition} instalada en: $dest/${edition}.mmdb"
        else
            log_message "ERROR" "No se encontró archivo .mmdb dentro del paquete ${edition}"
        fi
        rm -rf "$tmpdir"
    done
}

# Intentar descargar GeoLite2 ahora (si se desea)
download_geolite2

###############################################################################
# SMART TESTER DEEP VALIDATION (CON CONNECTION.CONFIG)
###############################################################################
log_message "PROGRESS" "Ejecutando Smart Tester Deep Validation sobre connection.config..."
if [[ "$SILENT_MODE" == "true" ]]; then
    "$INSTALL_DIR/executables/$EXEC_NAME" \
        --verify-setup \
        --verify-auto-fix \
        --verify-assume-yes \
        --config "$INSTALL_DIR/configs/connection.config" || true
else
    "$INSTALL_DIR/executables/$EXEC_NAME" \
        --verify-setup \
        --verify-auto-fix \
        --config "$INSTALL_DIR/configs/connection.config" || true
fi
log_message "SUCCESS" "Smart Tester Deep Validation completado"

###############################################################################
# CREAR SCRIPT DE PROTECCIÓN PARA connection.config
###############################################################################
VIEW_CONFIG_SCRIPT="$INSTALL_DIR/executables/view_config.sh"
log_message "PROGRESS" "Creando script de protección para connection.config..."
{
    echo "#!/bin/bash"
    echo "#"
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# NESS RELAY — Visor Seguro de Configuración"
    echo "# Este script protege el acceso al archivo connection.config"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""
    echo "CONFIG_FILE='/opt/ness_relay/configs/connection.config'"
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
    echo "echo -e \"\${CYAN}║       🔐  NESS RELAY — Acceso Protegido              ║\${NC}\""
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

# Proteger el archivo connection.config con permisos restrictivos
chmod 600 "$INSTALL_DIR/configs/connection.config"
log_message "SUCCESS" "Permisos de seguridad aplicados a connection.config (600 — solo root)"

###############################################################################
# CREAR SCRIPT DE EJECUCIÓN (run_relay.sh)
###############################################################################
RUN_SCRIPT="$INSTALL_DIR/executables/run_relay.sh"
log_message "PROGRESS" "Creando script de ejecución..."
{
    echo "#!/bin/bash"
    echo "#"
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# NESS RELAY — Script de Ejecución (Rust Edition)"
    echo "# Generado automáticamente el $(date)"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""
    echo "# Cargar variables de entorno"
    echo "source $ENV_FILE"
    echo ""
    echo "# Cambiar al directorio de instalación"
    echo "cd $INSTALL_DIR"
    echo ""
    echo "# Colores para mensajes"
    echo "GREEN='\\033[0;32m'"
    echo "RED='\\033[0;31m'"
    echo "YELLOW='\\033[1;33m'"
    echo "NC='\\033[0m'"
    echo ""
    echo "# Detectar si estamos en un terminal interactivo o en cron"
    echo "if [ -t 1 ]; then"
    echo "    # Terminal interactivo: mostrar salida en tiempo real"
    echo "    echo -e \"\${YELLOW}Ejecutando NESS Relay...\${NC}\""
    echo "    echo \"\""
    echo "    ./executables/$EXEC_NAME --config $INSTALL_DIR/configs/connection.config"
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
    echo "    # Ejecución desde cron: modo silencioso (solo escribe al log interno)"
    echo "    ./executables/$EXEC_NAME --silent --config $INSTALL_DIR/configs/connection.config"
    echo "    EXIT_CODE=\$?"
    echo "    if [ \$EXIT_CODE -ne 0 ]; then"
    echo "        echo \"[\$(date '+%Y-%m-%d %H:%M:%S')] ERROR: Relay falló con código \$EXIT_CODE\" >> $INSTALL_DIR/logs/ness_relay.log"
    echo "    fi"
    echo "    exit \$EXIT_CODE"
    echo "fi"
} > "$RUN_SCRIPT"

chmod +x "$RUN_SCRIPT"
log_message "SUCCESS" "Script de ejecución creado: $RUN_SCRIPT"

###############################################################################
# CONFIGURAR CRON (CADA 5 MINUTOS)
###############################################################################
log_message "PROGRESS" "Configurando tarea programada (cron)..."

# Verificar/instalar cron según la distro
if ! command -v crontab &>/dev/null; then
    log_message "PROGRESS" "Instalando cron..."
    if command -v apt-get &>/dev/null; then
        apt-get install -y cron >/dev/null 2>&1
        systemctl enable cron 2>/dev/null || true
        systemctl start cron 2>/dev/null || true
    elif command -v dnf &>/dev/null; then
        dnf install -y cronie >/dev/null 2>&1
        systemctl enable crond 2>/dev/null || true
        systemctl start crond 2>/dev/null || true
    elif command -v yum &>/dev/null; then
        yum install -y cronie >/dev/null 2>&1
        systemctl enable crond 2>/dev/null || true
        systemctl start crond 2>/dev/null || true
    fi
fi

# Eliminar entradas existentes del relay
(crontab -l 2>/dev/null | grep -v "$RUN_SCRIPT" | grep -v "ness.relay" | grep -v "ness_relay") | crontab -

# Añadir nueva entrada de cron
(crontab -l 2>/dev/null; echo "*/5 * * * * $RUN_SCRIPT") | crontab -
log_message "SUCCESS" "Tarea programada configurada (cada 5 minutos)"

###############################################################################
# PRUEBA OPCIONAL
###############################################################################
echo ""
print_box "PRUEBA DE CONFIGURACIÓN" "${CYAN}${BOLD}"
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

        # Ejecutar directamente (no vía run_relay.sh) para que --silent no interfiera
        # run_relay.sh usa "[ -t 1 ]" que falla con pipes, causando modo silencioso
        TEST_OUTPUT_FILE=$(mktemp /tmp/ness_relay_test_XXXXXX.log)
        source "$ENV_FILE"
        cd "$INSTALL_DIR"
        ./executables/$EXEC_NAME --config "$INSTALL_DIR/configs/connection.config" 2>&1 | tee "$TEST_OUTPUT_FILE"
        TEST_EXIT_CODE=${PIPESTATUS[0]}

        echo ""
        if [ $TEST_EXIT_CODE -eq 0 ]; then
            INSTALL_STATUS="success"
            echo -e "${GREEN}${BOLD}✓ Prueba exitosa! El relay está funcionando correctamente.${NC}"
        else
            # Analizar el tipo de error para dar mensaje específico
            if grep -qi "HTTP 401\|Unauthorized\|Token.*inválido\|token.*invalid" "$TEST_OUTPUT_FILE" 2>/dev/null; then
                INSTALL_STATUS="auth_error"
                echo -e "${RED}${BOLD}✗ Error de autenticación: el servidor rechazó el token de API.${NC}"
                echo ""
                echo -e "${YELLOW}Sugerencias:${NC}"
                echo -e "${WHITE}  • Verifique que NESS_API_TOKEN sea correcto en: ${DIM}$INSTALL_DIR/configs/.env${NC}"
                echo -e "${WHITE}  • Confirme el token en la plataforma NESS HQ${NC}"
            elif grep -qi "HTTP 400\|Datos inválidos\|Expected a dictionary" "$TEST_OUTPUT_FILE" 2>/dev/null; then
                INSTALL_STATUS="data_error"
                echo -e "${RED}${BOLD}✗ Error de datos: el servidor rechazó el formato de los datos enviados.${NC}"
                echo ""
                echo -e "${YELLOW}Sugerencias:${NC}"
                echo -e "${WHITE}  • Verifique que la versión del relay sea compatible con el servidor${NC}"
                echo -e "${WHITE}  • Revise el log detallado: ${DIM}tail -f $INSTALL_DIR/logs/ness_relay.log${NC}"
            elif grep -qi "timeout\|timed out\|Conectividad SNMP falló\|No se recibió respuesta" "$TEST_OUTPUT_FILE" 2>/dev/null; then
                INSTALL_STATUS="snmp_error"
                echo -e "${RED}${BOLD}✗ Error de conectividad SNMP: no se pudo contactar el dispositivo.${NC}"
                echo ""
                echo -e "${YELLOW}Sugerencias:${NC}"
                echo -e "${WHITE}  • Verifique que el dispositivo sea alcanzable: ${DIM}ping <IP_DISPOSITIVO>${NC}"
                echo -e "${WHITE}  • Verifique las credenciales SNMP en: ${DIM}$INSTALL_DIR/configs/connection.config${NC}"
                echo -e "${WHITE}  • Verifique que el puerto SNMP (161) esté abierto${NC}"
            elif grep -qi "HTTP 5[0-9][0-9]\|Internal Server Error\|Bad Gateway\|Service Unavailable" "$TEST_OUTPUT_FILE" 2>/dev/null; then
                INSTALL_STATUS="server_error"
                echo -e "${RED}${BOLD}✗ Error del servidor: el servidor NESS no pudo procesar la solicitud.${NC}"
                echo ""
                echo -e "${YELLOW}Sugerencias:${NC}"
                echo -e "${WHITE}  • Verifique que el servidor NESS esté en línea${NC}"
                echo -e "${WHITE}  • Contacte al administrador del servidor${NC}"
            elif grep -qi "Connection refused\|No route to host\|Network is unreachable\|Could not resolve" "$TEST_OUTPUT_FILE" 2>/dev/null; then
                INSTALL_STATUS="network_error"
                echo -e "${RED}${BOLD}✗ Error de red: no se pudo conectar al servidor NESS.${NC}"
                echo ""
                echo -e "${YELLOW}Sugerencias:${NC}"
                echo -e "${WHITE}  • Verifique la conectividad de red hacia el servidor${NC}"
                echo -e "${WHITE}  • Verifique la URL del servidor en: ${DIM}$INSTALL_DIR/configs/.env${NC}"
            else
                INSTALL_STATUS="unknown_error"
                echo -e "${RED}${BOLD}✗ La prueba falló con un error desconocido.${NC}"
                echo ""
                echo -e "${YELLOW}Sugerencias:${NC}"
                echo -e "${WHITE}  • Revise el log detallado: ${DIM}tail -f $INSTALL_DIR/logs/ness_relay.log${NC}"
            fi
            echo ""
            echo -e "${WHITE}  • Para ver la configuración de forma segura:${NC}"
            echo -e "${DIM}    sudo $INSTALL_DIR/executables/view_config.sh${NC}"
        fi

        # Limpiar archivo temporal
        rm -f "$TEST_OUTPUT_FILE"
    else
        INSTALL_STATUS="skipped"
        echo ""
        echo -e "${WHITE}Prueba omitida. El relay se ejecutará automáticamente cada 5 minutos vía cron.${NC}"
    fi
else
    INSTALL_STATUS="skipped"
    echo -e "${WHITE}Modo silencioso: omitiendo prueba interactiva.${NC}"
fi

###############################################################################
# MENSAJE FINAL
###############################################################################
echo ""

if [[ "$INSTALL_STATUS" == "success" ]] || [[ "$INSTALL_STATUS" == "skipped" ]]; then
    echo ""
    print_box "✅  INSTALACIÓN COMPLETADA EXITOSAMENTE" "${GREEN}${BOLD}"
    echo ""
else
    echo ""
    print_box "⚠️  INSTALACIÓN COMPLETADA CON ADVERTENCIAS" "${YELLOW}${BOLD}"
    echo -e "${YELLOW}${DIM}   Los archivos se instalaron correctamente, pero la prueba de ejecución${NC}"
    echo -e "${YELLOW}${DIM}   detectó errores. Revise las sugerencias arriba antes de continuar.${NC}"
    echo ""
fi

echo -e "${WHITE}${BOLD}📁 DETALLES DE LA INSTALACIÓN:${NC}"
echo -e "${WHITE}  • Directorio de instalación:${NC} ${BOLD}$INSTALL_DIR${NC}"
echo -e "${WHITE}  • Ejecutable:${NC}               ${DIM}$INSTALL_DIR/executables/$EXEC_NAME${NC}"
echo -e "${WHITE}  • Configuración:${NC}            ${DIM}$INSTALL_DIR/configs/connection.config${NC}"
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
if [[ "$INSTALL_STATUS" == "success" ]] || [[ "$INSTALL_STATUS" == "skipped" ]]; then
    echo -e "${GREEN}${BOLD}🎉 ¡INSTALACIÓN FINALIZADA EXITOSAMENTE!${NC}"
    echo -e "${WHITE}   El relay está programado para ejecutarse cada 5 minutos.${NC}"
    echo -e "${WHITE}   Para ver diagnósticos en tiempo real, ejecute: ${GREEN}sudo $RUN_SCRIPT${NC}"
else
    echo -e "${YELLOW}${BOLD}⚠️  INSTALACIÓN FINALIZADA CON ADVERTENCIAS${NC}"
    echo -e "${WHITE}   Los archivos se instalaron y el cron está configurado.${NC}"
    echo -e "${WHITE}   Corrija los errores reportados y ejecute manualmente: ${GREEN}sudo $RUN_SCRIPT${NC}"
fi
echo -e "${DIM}   Gracias por usar NESS HQ Network Relay System${NC}"
echo ""
