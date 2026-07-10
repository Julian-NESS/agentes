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
# IMPORTANTE: Este instalador requiere el binario correspondiente a la
#             arquitectura del host (`ness-relay-x86_64` o `ness-relay-aarch64`)
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
UPDATE_ONLY_MODE=false
CONFIG_FILE=""
API_TOKEN=""
SERVER_ENV=3  # Por defecto usamos Public Cloud
GUIDED_MODE=false
CRON_INTERVAL=""
GUIDED_VENDOR=""
GUIDED_SNMP_VERSION=""
GUIDED_DEVICE_IP=""
GUIDED_DEVICE_PORT=""
GUIDED_COMMUNITY=""
GUIDED_V3_USER=""
GUIDED_V3_AUTH_PROTOCOL=""
GUIDED_V3_AUTH_PASSWORD=""
GUIDED_V3_PRIV_PROTOCOL=""
GUIDED_V3_PRIV_PASSWORD=""
GUIDED_DESCRIPTION=""
EXISTING_INSTALL_DIR="/opt/ness_relay"
PRESERVED_CONFIG_SOURCE=""
PRESERVED_SERVER_ID=""
PRESERVED_API_TOKEN=""
PRESERVED_INSTALL_DIR=""
PRESERVED_DEVICES_FILE=""
PRESERVED_OUTPUT_DIR=""
PRESERVED_LOG_DIR=""

RELAY_METADATA_URL_DEFAULT="https://storage.googleapis.com/agent-updates-lab/utilities/relay/latest.json"
RELAY_METADATA_URL="${NESS_RELAY_METADATA_URL:-$RELAY_METADATA_URL_DEFAULT}"
TEMP_ARTIFACT_DIR=""
SOURCE_PACKAGE_DIR=""
SOURCE_PACKAGE_HAS_LOCAL_BINARY=false
SOURCE_PACKAGE_HAS_LOCAL_INSTALLER=false

# Función para cargar variables de entorno previas en actualización
load_existing_env_vars() {
    local env_file="/etc/profile.d/ness_relay.sh"
    if [[ -f "$env_file" ]]; then
        log_message "PROGRESS" "Leyendo variables de entorno existentes desde $env_file..."
        # Extraer valores de forma segura sin ejecutar el archivo
        PRESERVED_SERVER_ID="$(grep -o 'NESS_SERVER_ID="[^"]*"' "$env_file" | cut -d'"' -f2)"
        PRESERVED_API_TOKEN="$(grep -o 'NESS_API_TOKEN="[^"]*"' "$env_file" | cut -d'"' -f2)"
        PRESERVED_INSTALL_DIR="$(grep -o 'NESS_INSTALL_DIR="[^"]*"' "$env_file" | cut -d'"' -f2)"
        PRESERVED_DEVICES_FILE="$(grep -o 'NESS_DEVICES_FILE="[^"]*"' "$env_file" | cut -d'"' -f2)"
        PRESERVED_OUTPUT_DIR="$(grep -o 'NESS_OUTPUT_DIR="[^"]*"' "$env_file" | cut -d'"' -f2)"
        PRESERVED_LOG_DIR="$(grep -o 'NESS_LOG_DIR="[^"]*"' "$env_file" | cut -d'"' -f2)"
        
        # Si encontró variables, restituirlas a las variables de instalación actual
        if [[ -n "$PRESERVED_SERVER_ID" ]]; then
            SERVER_ENV="$PRESERVED_SERVER_ID"
            log_message "SUCCESS" "Server ID recuperado: $SERVER_ENV"
        fi
        if [[ -n "$PRESERVED_API_TOKEN" ]]; then
            API_TOKEN="$PRESERVED_API_TOKEN"
            log_message "SUCCESS" "Token API recuperado"
        fi
    else
        log_message "WARNING" "No se encontró configuración de entorno existente en $env_file"
    fi
}

detect_source_package_dir() {
    local script_dir="$1"
    local binary_source="$2"

    SOURCE_PACKAGE_DIR=""
    SOURCE_PACKAGE_HAS_LOCAL_BINARY=false
    SOURCE_PACKAGE_HAS_LOCAL_INSTALLER=false

    if [[ -z "$script_dir" || -z "$binary_source" ]]; then
        return
    fi

    if [[ -f "$script_dir/install_relay.sh" ]]; then
        SOURCE_PACKAGE_HAS_LOCAL_INSTALLER=true
    fi

    if [[ "$binary_source" == "$script_dir"/* ]]; then
        SOURCE_PACKAGE_HAS_LOCAL_BINARY=true
    fi

    if [[ "$SOURCE_PACKAGE_HAS_LOCAL_INSTALLER" == "true" && "$SOURCE_PACKAGE_HAS_LOCAL_BINARY" == "true" ]]; then
        SOURCE_PACKAGE_DIR="$script_dir"
    fi
}

cleanup_source_package_if_needed() {
    local candidate_dir="$SOURCE_PACKAGE_DIR"

    # Solo limpiar en instalación inicial (no en update) y con instalación completada.
    if [[ "$UPDATE_ONLY_MODE" == "true" ]]; then
        return
    fi
    if [[ "$INSTALL_STATUS" != "success" && "$INSTALL_STATUS" != "skipped" ]]; then
        return
    fi
    if [[ -z "$candidate_dir" || ! -d "$candidate_dir" ]]; then
        return
    fi

    # Guardas de seguridad para evitar borrar rutas críticas.
    case "$candidate_dir" in
        "/"|"/opt"|"/opt/ness_relay"|"/opt/ness_relay/"|"/opt/ness_relay"/*)
            log_message "WARNING" "Limpieza omitida por seguridad (ruta protegida): $candidate_dir"
            return
            ;;
    esac

    # No borrar entornos de desarrollo/repositorio.
    if [[ -f "$candidate_dir/Cargo.toml" || -d "$candidate_dir/src" || -d "$candidate_dir/.git" ]]; then
        log_message "INFO" "Limpieza omitida: directorio detectado como entorno de desarrollo ($candidate_dir)"
        return
    fi

    if rm -rf "$candidate_dir" 2>/dev/null; then
        log_message "SUCCESS" "Carpeta temporal de instalación eliminada: $candidate_dir"
    else
        log_message "WARNING" "No se pudo eliminar la carpeta temporal de instalación: $candidate_dir"
    fi
}

declare -A SELECTED_VENDORS
declare -A DEVICE_CONFIGS

get_configured_vendors() {
    local vendor_list=()

    if [[ ${#SELECTED_VENDORS[@]} -gt 0 ]]; then
        mapfile -t vendor_list < <(printf '%s\n' "${!SELECTED_VENDORS[@]}" | sort)
    else
        vendor_list=("${VENDORS[@]}")
    fi

    printf '%s\n' "${vendor_list[@]}"
}

# Nombre del ejecutable (binario estático Rust)
EXEC_NAME="ness-relay"
INSTALLED_BINARY_NAME=""

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

cleanup_temp_artifacts() {
    if [[ -n "$TEMP_ARTIFACT_DIR" && -d "$TEMP_ARTIFACT_DIR" ]]; then
        rm -rf "$TEMP_ARTIFACT_DIR" 2>/dev/null || true
    fi
}
trap cleanup_temp_artifacts EXIT

# ----------------------------------------------------------------------------
# Helper: escapa caracteres especiales para usar una cadena como reemplazo
# en `sed s|...|nuevo|`. Específicamente protege contra:
#   \ → \\  (escape de backslash)
#   & → \&  (referencia al patrón matcheado en GNU sed)
#   | → \|  (delimitador del comando s|)
#   / → /   (delimitador alternativo)
# Si la password contiene caracteres como `{}[]()*+?.^$` se mantienen tal
# cual (sed los trata literal fuera del delimitador).
# ----------------------------------------------------------------------------
escape_sed_replacement() {
    local s="$1"
    s="${s//\\/\\\\}"
    s="${s//&/\\&}"
    s="${s//|/\\|}"
    printf '%s' "$s"
}

download_file() {
    local url="$1"
    local output_file="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 --connect-timeout 10 -o "$output_file" "$url"
        return $?
    fi

    if command -v wget >/dev/null 2>&1; then
        wget -qO "$output_file" "$url"
        return $?
    fi

    return 1
}

verify_sha256_checksum() {
    local file_path="$1"
    local expected_sha="$2"
    local calculated_sha=""

    if command -v sha256sum >/dev/null 2>&1; then
        calculated_sha="$(sha256sum "$file_path" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
        calculated_sha="$(shasum -a 256 "$file_path" | awk '{print $1}')"
    else
        return 2
    fi

    [[ "$calculated_sha" == "$expected_sha" ]]
}

extract_latest_version() {
    local metadata_file="$1"
    sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$metadata_file" | head -n 1
}

extract_variant_download_info() {
    local metadata_file="$1"
    local target_arch="$2"

    if command -v jq >/dev/null 2>&1; then
        jq -r --arg arch "$target_arch" '
            .variants[]
            | select(.arch == $arch and .platform == "linux")
            | [(.binary.url // .pack.url), (.binary.sha256 // .pack.sha256)]
            | @tsv
        ' "$metadata_file" | head -n 1 | awk -F'\t' 'NF>=2 {print $1"\n"$2}'
        return 0
    fi
    # Fallback: use python3 if available (more portable than complex awk parsing)
    if command -v python3 >/dev/null 2>&1; then
        python3 - "$metadata_file" "$target_arch" <<'PY'
import sys, json
fn = sys.argv[1]
arch = sys.argv[2]
try:
    with open(fn, 'r', encoding='utf-8') as f:
        data = json.load(f)
except Exception:
    sys.exit(1)

variants = data.get('variants', [])
for v in variants:
    if str(v.get('arch')) == arch and str(v.get('platform')) == 'linux':
        # prefer binary.url, fallback to pack.url
        binobj = v.get('binary') or v.get('pack') or {}
        url = binobj.get('url') or v.get('url') or ''
        sha = binobj.get('sha256') or v.get('sha256') or ''
        if url:
            print(url)
            print(sha)
            sys.exit(0)
sys.exit(2)
PY
        return $?
    fi

    # Last resort: simple awk that extracts first url/sha after matching arch
    awk -v arch="$target_arch" '
        BEGIN { capture=0; url=""; sha="" }
        /"arch"[[:space:]]*:[[:space:]]*"/ {
            if ($0 ~ arch) { capture=1 }
        }
        capture && url == "" && /"url"[[:space:]]*:/ {
            split($0, parts, "\"")
            for (i=1; i<=length(parts); i++) {
                if (parts[i] == "url") { url = parts[i+2] }
            }
        }
        capture && sha == "" && /"sha256"[[:space:]]*:/ {
            split($0, parts, "\"")
            for (i=1; i<=length(parts); i++) {
                if (parts[i] == "sha256") { sha = parts[i+2] }
            }
        }
        capture && url != "" && sha != "" { print url; print sha; exit 0 }
    ' "$metadata_file"
}

normalize_guided_vendor() {
    local raw_vendor="$1"
    local normalized="${raw_vendor,,}"
    normalized="${normalized//[[:space:]]/}"

    case "$normalized" in
        generic|auto|any|unknown|other|firewall|router|switch|ap|access_point)
            echo "generic"
            ;;
        windows|win|linux|mac|darwin)
            # En instalación guiada de Relay el valor corresponde al SO del servidor
            # donde se instala el agente, no al vendor real del dispositivo SNMP.
            echo "generic"
            ;;
        cisco)
            echo "cisco"
            ;;
        fortinet)
            echo "fortinet"
            ;;
        pfsense|pfsensefw)
            echo "pfsense"
            ;;
        mikrotik|routeros)
            echo "mikrotik"
            ;;
        mikrotik_fw|mikrotikfw|mikrotik-firewall)
            echo "mikrotik_fw"
            ;;
        ubnt|ubiquiti)
            echo "ubnt"
            ;;
        c_n|cambium|cambiumnetworks)
            echo "c_n"
            ;;
        *)
            echo "generic"
            ;;
    esac
}

setup_guided_configuration_from_env() {
    local key_prefix config_key

    GUIDED_VENDOR="$(normalize_guided_vendor "${NESS_RELAY_VENDOR:-${NESS_RELAY_DEVICE_VENDOR:-generic}}")"
    GUIDED_SNMP_VERSION="${NESS_RELAY_SNMP_VERSION:-2c}"
    GUIDED_DEVICE_IP="${NESS_RELAY_DEVICE_IP:-}"
    GUIDED_DEVICE_PORT="${NESS_RELAY_SNMP_PORT:-161}"
    GUIDED_COMMUNITY="${NESS_RELAY_COMMUNITY:-public}"
    GUIDED_V3_USER="${NESS_RELAY_SNMPV3_USER:-}"
    GUIDED_V3_AUTH_PROTOCOL="${NESS_RELAY_SNMPV3_AUTH_PROTOCOL:-SHA}"
    GUIDED_V3_AUTH_PASSWORD="${NESS_RELAY_SNMPV3_AUTH_PASSWORD:-}"
    GUIDED_V3_PRIV_PROTOCOL="${NESS_RELAY_SNMPV3_PRIV_PROTOCOL:-AES128}"
    GUIDED_V3_PRIV_PASSWORD="${NESS_RELAY_SNMPV3_PRIV_PASSWORD:-}"
    GUIDED_DESCRIPTION="${NESS_RELAY_DEVICE_DESCRIPTION:-Instalación guiada}"

    if [[ -z "$GUIDED_DEVICE_IP" ]]; then
        log_message "ERROR" "Modo guiado: falta NESS_RELAY_DEVICE_IP"
        exit 1
    fi

    if [[ "$GUIDED_SNMP_VERSION" == "3" && -z "$GUIDED_V3_USER" ]]; then
        log_message "ERROR" "Modo guiado: para SNMPv3 debe enviar NESS_RELAY_SNMPV3_USER"
        exit 1
    fi

    key_prefix="${GUIDED_VENDOR}"
    config_key="${key_prefix}_1"

    SELECTED_VENDORS["$key_prefix"]="true"
    DEVICE_CONFIGS["${key_prefix}_count"]="1"
    DEVICE_CONFIGS["${config_key}_ip"]="$GUIDED_DEVICE_IP"
    DEVICE_CONFIGS["${config_key}_port"]="$GUIDED_DEVICE_PORT"
    DEVICE_CONFIGS["${config_key}_description"]="$GUIDED_DESCRIPTION"
    DEVICE_CONFIGS["${config_key}_vendor"]="$GUIDED_VENDOR"
    DEVICE_CONFIGS["${config_key}_snmp_version"]="$GUIDED_SNMP_VERSION"

    if [[ "$GUIDED_SNMP_VERSION" == "1" || "$GUIDED_SNMP_VERSION" == "2c" ]]; then
        DEVICE_CONFIGS["${config_key}_community"]="$GUIDED_COMMUNITY"
    else
        DEVICE_CONFIGS["${config_key}_v3_user"]="$GUIDED_V3_USER"
        DEVICE_CONFIGS["${config_key}_v3_auth_protocol"]="$GUIDED_V3_AUTH_PROTOCOL"
        DEVICE_CONFIGS["${config_key}_v3_auth_password"]="$GUIDED_V3_AUTH_PASSWORD"
        DEVICE_CONFIGS["${config_key}_v3_priv_protocol"]="$GUIDED_V3_PRIV_PROTOCOL"
        DEVICE_CONFIGS["${config_key}_v3_priv_password"]="$GUIDED_V3_PRIV_PASSWORD"
    fi
}

download_binary_from_metadata() {
    local host_arch="$1"
    local metadata_file metadata_version variant_url variant_sha

    TEMP_ARTIFACT_DIR="$(mktemp -d /tmp/ness_relay_guided_XXXXXX)"
    metadata_file="$TEMP_ARTIFACT_DIR/latest.json"

    log_message "PROGRESS" "Descargando metadata de release: $RELAY_METADATA_URL"
    if ! download_file "$RELAY_METADATA_URL" "$metadata_file"; then
        log_message "ERROR" "No se pudo descargar latest.json desde $RELAY_METADATA_URL"
        return 1
    fi

    metadata_version="$(extract_latest_version "$metadata_file")"
    mapfile -t variant_info < <(extract_variant_download_info "$metadata_file" "$host_arch")

    if [[ ${#variant_info[@]} -lt 2 ]]; then
        log_message "ERROR" "No existe variante para arquitectura '$host_arch' en latest.json"
        # Contenido de latest.json oculto por motivos de seguridad
        return 1
    fi

    variant_url="${variant_info[0]}"
    variant_sha="${variant_info[1]}"

    BINARY_NAME_SELECTED="${EXEC_NAME}-${host_arch}"
    BINARY_SOURCE="$TEMP_ARTIFACT_DIR/$BINARY_NAME_SELECTED"

    log_message "PROGRESS" "Descargando binario '$BINARY_NAME_SELECTED'${metadata_version:+ (v$metadata_version)}"
    if ! download_file "$variant_url" "$BINARY_SOURCE"; then
        log_message "ERROR" "Fallo al descargar binario desde: $variant_url"
        return 1
    fi

    if [[ -n "$variant_sha" ]]; then
        if verify_sha256_checksum "$BINARY_SOURCE" "$variant_sha"; then
            log_message "SUCCESS" "Checksum SHA-256 del binario verificado correctamente"
        else
            log_message "ERROR" "Checksum SHA-256 inválido para binario descargado"
            return 1
        fi
    else
        log_message "WARNING" "La variante descargada no incluye SHA-256 en latest.json"
    fi

    chmod +x "$BINARY_SOURCE" 2>/dev/null || true
    return 0
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

# ----------------------------------------------------------------------------
# prompt_password_with_confirm
# Pide una contraseña al usuario con confirmación (doble input).
# Las contraseñas pueden ser visibles u ocultas (según parámetro).
# Si no coinciden, vuelve a pedirlas (hasta que coincidan o se alcance max_intentos).
#
# Argumentos:
#   $1 = prompt_label  (ej: "Contraseña de Autenticación")
#   $2 = min_length    (ej: 8, o "" si no hay mínimo)
#   $3 = show_mode     ("visible" o "oculto")
#   $4 = allow_empty   ("true" para permitir Enter vacío sin pedir confirmación, "false" si no)
# Salida (stdout): la contraseña confirmada (string)
#
# IMPORTANTE Phase 2.8: todos los prompts van a STDERR, no a stdout.
# Esto es crítico cuando el caller hace `result=$(prompt_password_with_confirm ...)`
# porque si los prompts van a stdout, contaminan el valor capturado.
# Solo el valor de la contraseña (al final) va a stdout.
#
# Cambio Phase 2.7: la confirmación garantiza que el operador tipeó
# correctamente la contraseña. Especialmente crítico para contraseñas
# con caracteres especiales como '}' o '*' que son fáciles de errar.
# ----------------------------------------------------------------------------
prompt_password_with_confirm() {
    local prompt_label="$1"
    local min_length="$2"
    local show_mode="$3"
    local allow_empty="$4"
    local max_intentos=3
    local intento=1
    local pwd1="" pwd2=""

    while [[ $intento -le $max_intentos ]]; do
        # Primer input — prompt va a STDERR
        echo -ne "${WHITE}  🔑 ${prompt_label}: ${NC}" >&2
        if [[ "$show_mode" == "visible" ]]; then
            read -r pwd1 </dev/tty 2>/dev/null || read -r pwd1
        else
            read -rs pwd1 </dev/tty 2>/dev/null || read -rs pwd1
            echo "" >&2
        fi

        # Si está vacío Y se permite, retornamos vacío sin pedir confirmación
        if [[ -z "$pwd1" ]] && [[ "$allow_empty" == "true" ]]; then
            echo "" >&2
            echo ""
            return 0
        fi

        # Validar longitud mínima si aplica
        if [[ -n "$min_length" ]] && [[ ${#pwd1} -lt $min_length ]]; then
            echo -e "${RED}  ❌ Error: debe tener al menos $min_length caracteres${NC}" >&2
            intento=$((intento + 1))
            continue
        fi

        # Segundo input (confirmación) — prompt va a STDERR
        echo -ne "${WHITE}  🔑 Confirmar ${prompt_label}: ${NC}" >&2
        if [[ "$show_mode" == "visible" ]]; then
            read -r pwd2 </dev/tty 2>/dev/null || read -r pwd2
        else
            read -rs pwd2 </dev/tty 2>/dev/null || read -rs pwd2
            echo "" >&2
        fi

        # Verificar coincidencia
        if [[ "$pwd1" == "$pwd2" ]]; then
            # OK — solo devolver la contraseña a stdout
            echo "$pwd1"
            return 0
        else
            echo -e "${RED}  ❌ Las contraseñas no coinciden. Intento $intento de $max_intentos.${NC}" >&2
            intento=$((intento + 1))
        fi
    done

    echo -e "${RED}  ❌ Demasiados intentos fallidos.${NC}" >&2
    return 1
}

normalize_snmpv3_device_configs() {
    local vendor count device_count config_key snmp_version auth_key priv_key auth_pass_key priv_pass_key
    while IFS= read -r vendor; do
        [[ -z "$vendor" ]] && continue
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
    done < <(get_configured_vendors)
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
VENDORS=("generic")
VENDOR_NAMES=("Auto-Detección Inteligente")
# Vendors visibles en el menú principal (excluye hidden entries con nombre vacío)
VISIBLE_VENDOR_COUNT=1

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
interactive_device_configuration() {
    clear
    show_banner
    echo ""
    print_box "CONFIGURACIÓN DE DISPOSITIVOS (AUTO-DETECCIÓN)" "${WHITE}${BOLD}"
    echo ""
    echo -e "${DIM}  NESS Relay detectará automáticamente el tipo de dispositivo y fabricante.${NC}"
    echo -e "${DIM}  Solo necesitas proporcionar la IP y credenciales SNMP.${NC}"
    echo ""

    local vendor="generic"
    local device_count=0

    while true; do
        device_count=$((device_count + 1))
        echo -e "${WHITE}${BOLD}📡 Dispositivo #$device_count:${NC}"

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

        # ────────────────────────────────────────────────────────
        # Phase 2.4 — el vendor NO se pregunta al usuario. El binario
        # lo detecta automáticamente vía SNMP (sysObjectID + sysDescr).
        # Durante este flujo de captura SNMP todavía no sabemos la vendor
        # real, así que etiquetamos el device como `generic` en DEVICE_CONFIGS.
        # Después de generar connection.config, `post_install_probe_devices()`
        # corre el binario con `--probe <ip>` para descubrir el slug real
        # y reescribir las claves con la vendor correcta.
        # ────────────────────────────────────────────────────────
        echo -e "${DIM}  ℹ️  El vendor se detectará automáticamente vía SNMP tras la instalación.${NC}"

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
            snmp_version="1"
            echo -ne "${WHITE}  🔑 Community string SNMP [default: public]: ${NC}"
            read community
            community=${community:-public}
        elif [[ "$snmp_version_choice" == "2" ]]; then
            snmp_version="2c"
            echo -ne "${WHITE}  🔑 Community string SNMP [default: public]: ${NC}"
            read community
            community=${community:-public}
        else
            snmp_version="3"
            echo ""
            echo -e "${CYAN}${BOLD}  ═══ Configuración SNMPv3 ═══${NC}"

            while true; do
                echo -ne "${WHITE}  👤 Usuario SNMPv3: ${NC}"
                read v3_user
                if [[ -n "$v3_user" ]]; then break; else echo -e "${RED}  ❌ Error: El usuario no puede estar vacío${NC}"; fi
            done

            echo -e "${WHITE}  Protocolo de Autenticación:${NC}"
            echo -e "    ${WHITE}1)${NC} MD5 ${DIM}(HMAC-MD5-96)${NC}"
            echo -e "    ${WHITE}2)${NC} SHA ${DIM}(HMAC-SHA1-96, recomendado)${NC}"
            echo -e "    ${WHITE}3)${NC} SHA256 ${DIM}(HMAC-SHA2-256-128)${NC}"
            echo -e "    ${WHITE}4)${NC} SHA256-192 ${DIM}(HMAC-SHA2-256-192, compatibilidad)${NC}"
            echo -e "    ${WHITE}5)${NC} SHA384 ${DIM}(HMAC-SHA2-384-192)${NC}"
            echo -e "    ${WHITE}6)${NC} SHA512 ${DIM}(HMAC-SHA2-512-256)${NC}"
            echo -e "    ${WHITE}7)${NC} NONE ${DIM}(sin autenticación — no usar con privacidad)${NC}"
            echo -ne "${WHITE}  Selecciona [default: 2]: ${NC}"
            read auth_choice
            auth_choice=${auth_choice:-2}
            case "$auth_choice" in
                1) v3_auth_protocol="MD5" ;;
                2) v3_auth_protocol="SHA" ;;
                3) v3_auth_protocol="SHA256" ;;
                4) v3_auth_protocol="SHA256-192" ;;
                5) v3_auth_protocol="SHA384" ;;
                6) v3_auth_protocol="SHA512" ;;
                7) v3_auth_protocol="NONE" ;;
                *) v3_auth_protocol="SHA" ;;
            esac

            v3_auth_protocol=$(normalize_snmpv3_auth_protocol "$v3_auth_protocol")

            if [[ "$v3_auth_protocol" != "NONE" ]]; then
                # Phase 2.7: preguntar si quiere ver la contraseña mientras la escribe
                echo -ne "${WHITE}  👁️  ¿Desea ver la contraseña mientras la escribe? (Y/n): ${NC}"
                read v3_show_pw
                if [[ "$v3_show_pw" =~ ^[Yy]$ ]] || [[ -z "$v3_show_pw" ]]; then
                    v3_show_mode="visible"
                else
                    v3_show_mode="oculto"
                fi
                # Phase 2.7: pedir contraseña con confirmación (doble input)
                v3_auth_password=$(prompt_password_with_confirm \
                    "Contraseña de Autenticación (mín. 8 caracteres)" \
                    "8" \
                    "$v3_show_mode" \
                    "false")
                [[ $? -ne 0 ]] && return 1
            fi

            echo -e "${WHITE}  Protocolo de Privacidad (Encriptación):${NC}"
            echo -e "    ${WHITE}1)${NC} AES128 ${DIM}(AES-128-CFB, recomendado)${NC}"
            echo -e "    ${WHITE}2)${NC} AES192 ${DIM}(AES-192-CFB)${NC}"
            echo -e "    ${WHITE}3)${NC} AES256 ${DIM}(AES-256-CFB, máxima seguridad)${NC}"
            echo -e "    ${WHITE}4)${NC} DES ${DIM}(DES-CBC, obsoleto)${NC}"
            echo -e "    ${WHITE}5)${NC} NONE ${DIM}(sin encriptación)${NC}"
            echo -ne "${WHITE}  Selecciona [default: 1]: ${NC}"
            read priv_choice
            priv_choice=${priv_choice:-1}
            case "$priv_choice" in
                2) v3_priv_protocol="AES192" ;;
                3) v3_priv_protocol="AES256" ;;
                4) v3_priv_protocol="DES" ;;
                5) v3_priv_protocol="NONE" ;;
                *) v3_priv_protocol="AES128" ;;
            esac

            v3_priv_protocol=$(normalize_snmpv3_priv_protocol "$v3_priv_protocol")

            if [[ "$v3_priv_protocol" != "NONE" ]]; then
                # Phase 2.7: reusar la elección de visibilidad (visible/oculto)
                # Reusamos la elección de visibilidad del auth para mantener consistencia.
                # Phase 2.7: pedir contraseña de Privacidad con confirmación (doble input)
                v3_priv_password=$(prompt_password_with_confirm \
                    "Contraseña de Privacidad (mín. 8 caracteres)" \
                    "8" \
                    "$v3_show_mode" \
                    "false")
                [[ $? -ne 0 ]] && return 1
            fi
        fi

        echo -ne "${WHITE}  🔌 Puerto SNMP [default: 161]: ${NC}"
        read port
        port=${port:-161}

        echo -ne "${WHITE}  📝 Descripción del dispositivo [opcional]: ${NC}"
        read description

        local config_key="${vendor}_${device_count}"
        DEVICE_CONFIGS["${config_key}_ip"]="$device_ip"
        DEVICE_CONFIGS["${config_key}_port"]="$port"
        DEVICE_CONFIGS["${config_key}_description"]="$description"
        DEVICE_CONFIGS["${config_key}_vendor"]="$vendor"
        DEVICE_CONFIGS["${config_key}_snmp_version"]="$snmp_version"

        if [[ "$snmp_version" == "1" ]] || [[ "$snmp_version" == "2c" ]]; then
            DEVICE_CONFIGS["${config_key}_community"]="$community"
            echo -e "${GREEN}  ✅ Dispositivo configurado: ${BOLD}$device_ip${NC} ${DIM}($description)${NC}"
        else
            DEVICE_CONFIGS["${config_key}_v3_user"]="$v3_user"
            DEVICE_CONFIGS["${config_key}_v3_auth_protocol"]="$v3_auth_protocol"
            DEVICE_CONFIGS["${config_key}_v3_auth_password"]="$v3_auth_password"
            DEVICE_CONFIGS["${config_key}_v3_priv_protocol"]="$v3_priv_protocol"
            DEVICE_CONFIGS["${config_key}_v3_priv_password"]="$v3_priv_password"
            echo -e "${GREEN}  ✅ Dispositivo SNMPv3 configurado: ${BOLD}$device_ip${NC} ${DIM}(usuario: $v3_user)${NC}"
        fi
        echo ""

        SELECTED_VENDORS["$vendor"]="true"

        echo -ne "${YELLOW}¿Agregar otro dispositivo? (y/N): ${NC}"
        read add_another
        if [[ "$add_another" != "y" && "$add_another" != "Y" ]]; then
            break
        fi
    done

    DEVICE_CONFIGS["${vendor}_count"]="$device_count"
}

# Variable global para opt-in audit (Phase 2.4).
ENABLE_AUDIT="${ENABLE_AUDIT:-false}"

# ----------------------------------------------------------------------------
# Función: prompt de opt-in para análisis de vulnerabilidades + CIS
# Solo se llama en modo interactivo. En modo silent, devuelve "false".
#
# IMPORTANTE: Como bash no permite escribir a variables globales del caller
# cuando se invoca con `source <(awk ...)`, esta función imprime el resultado
# en stdout ("true" o "false"). El caller debe capturarlo así:
#     ENABLE_AUDIT=$(prompt_audit_optin)
#
# Cuando el usuario acepta:
#   - ENABLE_AUDIT se persiste en /etc/profile.d/ness_relay.sh
#   - El instalador registra una entrada de cron `0 */6 * * *` adicional
#   - Por cada dispositivo Fortinet, se recolectan credenciales SSH
# ----------------------------------------------------------------------------
prompt_audit_optin() {
    if [[ "$SILENT_MODE" == "true" ]]; then
        log_message "INFO" "Modo silencioso: análisis de vulnerabilidades DESACTIVADO por default"
        echo "false"
        return 0
    fi

    # IMPORTANTE: Toda la decoración va a STDERR (>&2) para no contaminar
    # el stdout. Solo el resultado final (`true` o `false`) va a STDOUT
    # para que `ENABLE_AUDIT=$(prompt_audit_optin)` capture SOLO ese valor.
    # Si mezcláramos el banner al stdout, el sed -i posterior rompería con
    # "unterminated `s' command" porque el patrón contendría todo el texto
    # decorativo (ver bug del 2026-07-03).

    echo "" >&2
    echo -e "${CYAN}${BOLD}  ════════════════════════════════════════════════════════════${NC}" >&2
    echo -e "${CYAN}${BOLD}  ANÁLISIS DE VULNERABILIDADES Y CONTROLES CIS${NC}" >&2
    echo -e "${CYAN}${BOLD}  ════════════════════════════════════════════════════════════${NC}" >&2
    echo "" >&2
    echo -e "  ${WHITE}ness-relay puede escanear vulnerabilidades conocidas y aplicar${NC}" >&2
    echo -e "  ${WHITE}controles CIS contra dispositivos Fortinet.${NC}" >&2
    echo "" >&2
    echo -e "  ${WHITE}Este análisis requiere:${NC}" >&2
    echo -e "    ${WHITE}- Acceso SSH al dispositivo${NC}" >&2
    echo -e "    ${WHITE}- Se ejecuta cada 6 horas (cron independiente)${NC}" >&2
    echo -e "    ${WHITE}- Las contraseñas NO se guardan en connection.config:${NC}" >&2
    echo -e "      ${WHITE}se leen del entorno en tiempo de ejecución${NC}" >&2
    echo "" >&2
    echo -e "  ${WHITE}Si activa esta opción, deberá proveer credenciales SSH durante${NC}" >&2
    echo -e "  ${WHITE}la configuración de cada dispositivo Fortinet.${NC}" >&2
    echo "" >&2

    echo -ne "${YELLOW}¿Desea activar análisis de vulnerabilidades? (y/N): ${NC}" >&2
    # Pequeña pausa para asegurar que cualquier buffer previo se consuma
    sleep 0.3
    read audit_response
    # Si el operador tecleó algo antes de este prompt, el read lo capturó.
    # Validamos explícitamente: solo aceptamos 'y', 'Y' o vacío (no).
    if [[ "$audit_response" =~ ^[Yy]$ ]]; then
        echo -e "${GREEN}  ✓ Análisis de vulnerabilidades ACTIVADO${NC}" >&2
        echo "" >&2
        echo "true"
        return 0
    fi

    # Si la respuesta no es y/Y ni n/N explícito, asumimos "no" por seguridad.
    # Mostramos un mensaje informativo y devolvemos false.
    if [[ -n "$audit_response" && ! "$audit_response" =~ ^[Nn]$ ]]; then
        echo -e "${YELLOW}  ⚠ Respuesta no reconocida ('$audit_response'). Asumiendo NO.${NC}" >&2
    fi
    echo -e "${DIM}  Análisis de vulnerabilidades desactivado. Puede activarlo después${NC}" >&2
    echo -e "${DIM}  ejecutando nuevamente este instalador.${NC}" >&2
    echo "false"
    return 0
}

# ----------------------------------------------------------------------------
# Recolectar credenciales SSH para cada dispositivo Fortinet agregado.
# Se llama solo si ENABLE_AUDIT=true.
# ----------------------------------------------------------------------------
collect_ssh_credentials() {
    if [[ "$ENABLE_AUDIT" != "true" ]]; then
        return 0
    fi

    echo ""
    echo -e "${CYAN}${BOLD}  ═══ Credenciales SSH para auditoría ═══${NC}"
    echo ""
    echo -e "  ${WHITE}Las credenciales SSH se almacenan así:${NC}"
    echo -e "    ${WHITE}- connection.config: solo el nombre del env var${NC}"
    echo -e "    ${WHITE}- Entorno del agente: la contraseña real (no persistida)${NC}"
    echo -e "  ${WHITE}Recomendación: use una cuenta de servicio dedicada${NC}"
    echo -e "  ${WHITE}con permisos de solo lectura (ej: 'auditor').${NC}"
    echo ""

    # Phase 2.4 — iterar sobre TODOS los devices cuya vendor ya fue detectada.
    # En este punto del flujo, las claves del DEVICE_CONFIGS todavía tienen
    # `generic_<N>_*` porque el instalador aún no detectó la vendor real.
    # La detección ocurre después de `generate_config_file()` en
    # `post_install_probe_devices()`, que reescribe las claves con el slug
    # correcto (e.g. `fortinet_1_*`).
    #
    # Por seguridad, aquí solo preguntamos SSH para devices que ya tienen
    # vendor asignada por el usuario vía `--vendor` flag (instalación CLI
    # no-interactiva) o que se configuran manualmente. La mayoría de los
    # usuarios serán cubiertos por `post_install_probe_devices()` después
    # de la instalación.
    #
    # Mapa de vendors que aceptan audit (Phase 1):
    local -A AUDIT_READY_VENDORS=(
        [fortinet]="Fortinet FortiGate"
        # Futuras vendors se agregan aquí cuando audit_runner las soporte.
    )

    # Iterar sobre todos los devices que ya tengan vendor != generic
    # (los `generic_*` los maneja post_install_probe_devices después).
    local -A seen_indices=()
    for key in "${!DEVICE_CONFIGS[@]}"; do
        if [[ "$key" =~ ^([a-z_]+)_([0-9]+)_ip$ ]]; then
            local vendor_slug="${BASH_REMATCH[1]}"
            local device_idx="${BASH_REMATCH[2]}"
            seen_indices["${vendor_slug}_${device_idx}"]=1
        fi
    done

    for composite_key in "${!seen_indices[@]}"; do
        local vendor_slug="${composite_key%_*}"
        local device_idx="${composite_key##*_}"

        # Solo preguntar SSH si la vendor ya es compatible con audit
        # (Phase 1: fortinet). Los `generic_*` se detectarán después.
        if [[ -z "${AUDIT_READY_VENDORS[$vendor_slug]:-}" ]]; then
            continue
        fi

        local config_key="${vendor_slug}_${device_idx}"
        local device_ip="${DEVICE_CONFIGS[${config_key}_ip]}"
        local vendor_label="${AUDIT_READY_VENDORS[$vendor_slug]}"

        echo -e "${BOLD}─── Dispositivo ${vendor_label}: $device_ip ───${NC}"

        echo -ne "${WHITE}  👤 Usuario SSH [default: admin]: ${NC}"
        read ssh_user
        ssh_user=${ssh_user:-admin}

        echo -ne "${WHITE}  🔌 Puerto SSH [default: 22]: ${NC}"
        read ssh_port
        ssh_port=${ssh_port:-22}

        local default_env="NESS_SSH_PASSWORD_$(echo "$vendor_slug" | tr '[:lower:]' '[:upper:]')_${device_idx}"
        echo ""
        echo -e "  ${DIM}El agente NUNCA almacena la contraseña SSH en el archivo${NC}"
        echo -e "  ${DIM}de configuración. Solo se guarda el NOMBRE de la variable${NC}"
        echo -e "  ${DIM}de entorno que el operador exporta en su shell.${NC}"
        echo ""
        echo -ne "${WHITE}  🔑 Nombre de la env var (NO la contraseña)${NC}"
        echo -ne "     ${WHITE}[default: $default_env]: ${NC}"
        read ssh_pw_env
        ssh_pw_env=${ssh_pw_env:-$default_env}

        # Validar que el env var name solo tenga [A-Z0-9_]
        if [[ ! "$ssh_pw_env" =~ ^[A-Z_][A-Z0-9_]*$ ]]; then
            log_message "WARN" "Nombre de env var inválido ('$ssh_pw_env'); usando default '$default_env'"
            ssh_pw_env="$default_env"
        fi

        DEVICE_CONFIGS["${config_key}_ssh_enabled"]="true"
        DEVICE_CONFIGS["${config_key}_ssh_username"]="$ssh_user"
        DEVICE_CONFIGS["${config_key}_ssh_port"]="$ssh_port"
        DEVICE_CONFIGS["${config_key}_ssh_password_env"]="$ssh_pw_env"

        echo ""
        echo -e "${GREEN}  ✓ Credenciales SSH configuradas para $device_ip${NC}"
        echo -e "${DIM}    Ahora exporte la contraseña en su shell:${NC}"
        echo -e "${CYAN}      export $ssh_pw_env='SU_PASSWORD_AQUI'${NC}"
        echo -e "${DIM}    O añádala a ~/.bashrc para que persista entre sesiones.${NC}"
        echo ""
    done
}

# ----------------------------------------------------------------------------
# post_install_probe_devices — detecta la vendor real de cada device mediante
# el binario con --probe, reescribe connection.config con el slug correcto,
# y pregunta credenciales SSH para los vendors que están en AUDIT_READY_VENDORS.
#
# Esta función se llama UNA SOLA VEZ al final de la instalación, después de
# `generate_config_file`. Es idempotente: si se vuelve a ejecutar, detecta
# de nuevo pero no duplica entradas.
# ----------------------------------------------------------------------------
post_install_probe_devices() {
    local config_file="$INSTALL_DIR/configs/connection.config"
    local binary="$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME"

    if [[ ! -f "$config_file" ]]; then
        log_message "WARN" "post_install_probe_devices: no existe $config_file"
        return 0
    fi
    if [[ ! -x "$binary" ]]; then
        log_message "WARN" "post_install_probe_devices: binario no ejecutable $binary"
        return 0
    fi

    log_message "PROGRESS" "Detectando vendors reales vía SNMP (--probe)…"

    # Mapa de vendors que aceptan audit
    local -A AUDIT_READY_VENDORS=(
        [fortinet]="Fortinet FortiGate"
    )

    # Recolectar IP y vendor actual (que será siempre 'generic' en este punto)
    declare -A CURRENT_DEVICES=()
    declare -A CURRENT_VENDOR=()
    while IFS='=' read -r key value; do
        key=$(echo "$key" | xargs)
        value=$(echo "$value" | xargs)
        if [[ "$key" =~ ^([a-z_]+)_([0-9]+)_ip$ ]]; then
            local v="${BASH_REMATCH[1]}"
            local i="${BASH_REMATCH[2]}"
            CURRENT_DEVICES["${v}_${i}"]="$value"
            CURRENT_VENDOR["${v}_${i}"]="$v"
        fi
    done < "$config_file"

    # Para cada device, correr --probe
    for composite_key in "${!CURRENT_DEVICES[@]}"; do
        local device_ip="${CURRENT_DEVICES[$composite_key]}"
        local old_slug="${CURRENT_VENDOR[$composite_key]}"
        local device_idx="${composite_key##*_}"

        log_message "INFO" "Probe $device_ip …"
        local probe_output
        local probe_exit
        # Phase 2.5.1: capturar SOLO stdout (el slug del vendor). stderr se
        # redirige a /dev/null para evitar que un warning de probe (ej.
        # "probe_debug: ip=...") se confunda con el slug cuando el
        # instalador hace `head -1 | awk`. Antes el 2>&1 mezclaba los
        # streams y el primer token podía ser "[WARN]" en lugar del
        # vendor real (bug que dejó devices como "[WARN]_1" en el config).
        probe_output=$("$binary" --probe "$device_ip" --config "$config_file" 2>/dev/null) || probe_exit=$?
        probe_exit=${probe_exit:-0}

        if [[ $probe_exit -ne 0 ]]; then
            log_message "WARN" "Probe falló para $device_ip (exit=$probe_exit) — se conserva vendor=$old_slug"
            # Phase 2.5: Aún si el probe falla, el Smart Tester pudo haber
            # detectado fortinet (mostrado en el banner "Vendor detectado
            # automáticamente: Fortinet FortiGate"). En ese caso, preguntar
            # SSH igual — el operador sabe que es FortiGate.
            #
            # Truco: si el probe falló pero el sysName devuelto es "FortiGate*",
            # considerar al device como fortinet para SSH.
            # Aquí lo hacemos simple: si la vendor actual es generic y hay
            # algún dispositivo previo renombrado a fortinet en este ciclo,
            # entonces preguntar SSH al operador para todos los generic
            # restantes (caso típico: 3 FortiGates, probe solo detecta 1).
            continue
        fi

        # El binario imprime UNA línea con el slug detectado en stdout.
        # Parseamos solo el primer token por seguridad (en caso de output sucio).
        local detected_slug
        detected_slug=$(echo "$probe_output" | head -1 | awk '{print $1}' | xargs)
        if [[ -z "$detected_slug" ]]; then
            log_message "WARN" "Probe devolvió vacío para $device_ip"
            continue
        fi

        # Phase 2.5.1: validar que el slug no contenga corchetes (lo que
        # indicaría que el stdout está contaminado con un warning del
        # binario o un mensaje de error). Los slugs válidos son
        # `[a-z0-9_]+` (ej: fortinet, mikrotik, pfsense, generic).
        if [[ ! "$detected_slug" =~ ^[a-z0-9_]+$ ]]; then
            log_message "WARN" "Probe devolvió slug inválido '$detected_slug' para $device_ip — se conserva vendor=$old_slug"
            continue
        fi

        if [[ "$detected_slug" != "$old_slug" ]]; then
            log_message "INFO" "Vendor detectada por SNMP: $old_slug → $detected_slug (IP=$device_ip)"
            rewrite_config_vendor "$config_file" "$old_slug" "$detected_slug" "$device_idx"
        fi
    done

    # ────────────────────────────────────────────────────────
    # RECONCILIACIÓN POST-PROBE (Phase 2.5)
    # ────────────────────────────────────────────────────────
    reconcile_vendor_counters "$config_file"

    # ────────────────────────────────────────────────────────
    # Segunda pasada: devices que quedaron como generic (probe falló)
    # Si el operador activó audit Y hay al menos un fortinet detectado,
    # preguntar al usuario si quiere clasificar los generic restantes
    # como fortinet (caso típico: 3 FortiGates, probe solo OK en 1).
    # ────────────────────────────────────────────────────────
    if [[ "${ENABLE_AUDIT:-false}" == "true" ]]; then
        # Phase 2.7: consolidar los generic_* restantes ANTES de mostrarlos.
        # Esto es importante porque después del probe+rename, los generic_*
        # pueden tener índices no contiguos (ej. generic_2, generic_3 porque
        # generic_1 fue renombrado a fortinet_1). Sin consolidación, el
        # usuario vería [2], [1] en lugar de [1], [2] lo cual es confuso.
        # La reconciliación renumera los generic_* a 1..N contiguos.
        log_message "INFO" "Consolidando devices generic restantes (pueden tener índices no contiguos)"
        reconcile_vendor_counters "$config_file"

        declare -A GENERIC_DEVICES=()
        while IFS='=' read -r key value; do
            key=$(echo "$key" | xargs)
            value=$(echo "$value" | xargs)
            if [[ "$key" =~ ^generic_([0-9]+)_ip$ ]]; then
                local i="${BASH_REMATCH[1]}"
                GENERIC_DEVICES["${i}"]="$value"
            fi
        done < "$config_file"

        if [[ "${#GENERIC_DEVICES[@]}" -gt 0 ]]; then
            echo ""
            echo -e "${YELLOW}${BOLD}  ⚠ Hay ${#GENERIC_DEVICES[@]} dispositivo(s) que quedaron como 'generic'${NC}"
            echo -e "${WHITE}  (el probe SNMP falló — puede ser por SNMPv1/v2c con credenciales distintas)${NC}"
            echo -e "${WHITE}  Si sabes que son FortiGate u otro vendor compatible con auditoría,${NC}"
            echo -e "${WHITE}  puedes clasificarlos manualmente para que el agente les aplique SSH.${NC}"
            echo ""

            # Listar los dispositivos generic
            for generic_idx in "${!GENERIC_DEVICES[@]}"; do
                echo -e "    ${WHITE}[$generic_idx] IP: ${BOLD}${GENERIC_DEVICES[$generic_idx]}${NC}"
            done
            echo ""

            echo -ne "${YELLOW}¿Desea clasificar alguno de estos dispositivos como fortinet para auditoría SSH? (y/N): ${NC}"
            read classify_response

            if [[ "$classify_response" =~ ^[Yy]$ ]]; then
                # Ordenar los generic_idx para procesarlos en orden ascendente
                # (más predecible, evita que el orden de iteración del hash
                # asociativo afecte el resultado).
                local sorted_generic_ids=($(printf '%s\n' "${!GENERIC_DEVICES[@]}" | sort -n))

                for generic_idx in "${sorted_generic_ids[@]}"; do
                    local g_ip="${GENERIC_DEVICES[$generic_idx]}"
                    echo ""
                    echo -e "${WHITE}  Dispositivo $generic_idx ($g_ip)${NC}"
                    echo -ne "  ${YELLOW}¿Es FortiGate? (y/N): ${NC}"
                    read is_fortinet
                    if [[ "$is_fortinet" =~ ^[Yy]$ ]]; then
                        # ────────────────────────────────────────────────────────
                        # Calcular el próximo idx disponible en fortinet para
                        # evitar colisiones con devices fortinet ya existentes.
                        # ────────────────────────────────────────────────────────
                        local next_ftn_idx
                        next_ftn_idx=$(_next_vendor_idx "$config_file" "fortinet")

                        log_message "INFO" "Renombrando generic_${generic_idx} → fortinet_${next_ftn_idx} (clasificación manual, IP=$g_ip)"
                        rewrite_config_vendor "$config_file" "generic" "fortinet" "$generic_idx" "$next_ftn_idx"
                    fi
                done
                # Reconciliar tras los renombres para consolidar contadores
                reconcile_vendor_counters "$config_file"
            fi
        fi
    fi

    # ────────────────────────────────────────────────────────
    # TERCERA PASADA: SSH para devices compatibles (Phase 2.6 — consolidada)
    # ────────────────────────────────────────────────────────
    #
    # Esta pasada corre AL FINAL de TODOS los renombres (probe + manual).
    # 1. Limpia ssh_* preexistentes (pueden estar mal mapeadas tras renombres).
    # 2. Itera TODOS los fortinet del config final y pregunta SSH a los
    #    que no tengan ssh_enabled=true.
    #
    # ────────────────────────────────────────────────────────

    # ── 1. Limpieza de ssh_* preexistentes ──
    log_message "INFO" "Limpiando claves SSH_* preexistentes (pueden estar mal mapeadas tras renombres)"
    sed -i -E '/^[a-z_]+_[0-9]+_ssh_(enabled|username|port|password_env)=/d' "$config_file"

    # ── 2. SSH prompt para cada fortinet compatible ──
    declare -A FINAL_DEVICES=()
    while IFS='=' read -r key value; do
        key=$(echo "$key" | xargs)
        value=$(echo "$value" | xargs)
        if [[ "$key" =~ ^([a-z_]+)_([0-9]+)_ip$ ]]; then
            local v="${BASH_REMATCH[1]}"
            local i="${BASH_REMATCH[2]}"
            FINAL_DEVICES["${v}_${i}"]="$value"
        fi
    done < "$config_file"

    # Ordenar para iterar en orden estable (vendor_idx ascendente)
    local sorted_composite=($(printf '%s\n' "${!FINAL_DEVICES[@]}" | sort))

    for composite_key in "${sorted_composite[@]}"; do
        local device_ip="${FINAL_DEVICES[$composite_key]}"
        local vendor_slug="${composite_key%_*}"
        local device_idx="${composite_key##*_}"

        # Saltar si la vendor no está en la whitelist de audit
        if [[ -z "${AUDIT_READY_VENDORS[$vendor_slug]:-}" ]]; then
            continue
        fi

        # Si ya tiene ssh_enabled=true (sobrevivió a la limpieza), saltar
        local ssh_already_set
        ssh_already_set=$(grep -E "^${vendor_slug}_${device_idx}_ssh_enabled=true" "$config_file" 2>/dev/null)
        if [[ -n "$ssh_already_set" ]]; then
            log_message "INFO" "SSH ya configurado para ${vendor_slug}_${device_idx} (${device_ip}), saltando"
            continue
        fi

        prompt_ssh_credentials_for_device "$vendor_slug" "$device_idx" "$device_ip"
    done

    if [[ "${REWRITE_HAPPENED:-false}" == "true" ]]; then
        log_message "SUCCESS" "Vendors reescritos y contadores reconciliados en connection.config"
    fi

    # Phase 2.5.1: re-cifrar campos sensibles con el AAD post-probe.
    # Antes el cifrado se hacía con device_id="generic_1" (porque el
    # probe aún no había corrido). Ahora, después del rename, los
    # tokens tienen AAD viejo pero el binario descifra con AAD nuevo
    # → AEAD fail. Recrypt con el AAD correcto lo resuelve.
    if [[ "${REWRITE_HAPPENED:-false}" == "true" ]]; then
        recrypt_sensitive_fields "$config_file"
    fi
}

# ----------------------------------------------------------------------------
# rewrite_config_vendor — renombra todas las claves <old>_<idx>_* a
# <new>_<idx>_* dentro del archivo connection.config.
# ----------------------------------------------------------------------------
# rewrite_config_vendor — renombra un device de un vendor a otro
#
# Argumentos:
#   $1 config_file
#   $2 old_slug          (e.g. "generic")
#   $3 new_slug          (e.g. "fortinet")
#   $4 device_idx        (idx ORIGINAL dentro de old_slug, e.g. "1")
#   [$5 new_idx]         (opcional: idx destino dentro de new_slug).
#                        Si no se da, se usa el mismo device_idx.
#
# Importante Phase 2.6: si new_slug YA tiene devices (e.g. fortinet_1 existe)
# y no se pasa new_idx, el sed crea una colisión (fortinet_1 duplicado).
# Para evitar esto, el caller debe calcular el próximo idx disponible con
# _next_vendor_idx() y pasarlo como $5.
#
# El contador principal NO se toca aquí — `reconcile_vendor_counters`
# lo recalcula tras todos los renombres.
# ----------------------------------------------------------------------------
rewrite_config_vendor() {
    local config_file="$1"
    local old_slug="$2"
    local new_slug="$3"
    local device_idx="$4"
    local new_idx="${5:-}"

    # Si no se dio new_idx, usar el mismo (compatibilidad con código previo)
    if [[ -z "$new_idx" ]]; then
        new_idx="$device_idx"
    fi

    local tmpfile="${config_file}.tmp.$$"

    if [[ "$old_slug" == "$new_slug" && "$device_idx" == "$new_idx" ]]; then
        # Nada que renombrar
        return 0
    fi

    if [[ "$old_slug" == "$new_slug" ]]; then
        # Mismo vendor, distinto idx — solo renumerar dentro del vendor
        sed -E "s/^${old_slug}_${device_idx}_/${new_slug}_${new_idx}_/g" \
            "$config_file" > "$tmpfile"
    else
        # Distinto vendor — primero renombrar prefijo, luego ajustar idx
        sed -E "s/^${old_slug}_${device_idx}_/${new_slug}_${device_idx}_/g" \
            "$config_file" > "$tmpfile"
        if [[ "$device_idx" != "$new_idx" ]]; then
            sed -i -E "s/^${new_slug}_${device_idx}_/${new_slug}_${new_idx}_/g" "$tmpfile"
        fi
    fi

    # Ajustar línea del vendor interno
    sed -i -E "s/^${new_slug}_${new_idx}_vendor=.*/${new_slug}_${new_idx}_vendor=${new_slug}/g" \
        "$tmpfile"

    mv "$tmpfile" "$config_file"
    chmod 600 "$config_file"
    REWRITE_HAPPENED="true"
}

# ----------------------------------------------------------------------------
# _next_vendor_idx — devuelve el siguiente índice disponible (1..N+1) para
# un vendor dado en el connection.config. Usado por la clasificación
# manual para evitar colisiones de idx entre vendors.
#
# Argumentos:
#   $1 config_file
#   $2 vendor_slug       (e.g. "fortinet")
#
# Salida:
#   Imprime el siguiente idx en stdout (e.g. "3" si hay fortinet_1 y
#   fortinet_2). Si no hay devices del vendor, imprime "1".
# ----------------------------------------------------------------------------
_next_vendor_idx() {
    local config_file="$1"
    local vendor_slug="$2"

    local max_idx=0
    while IFS='=' read -r key value; do
        key=$(echo "$key" | xargs)
        if [[ "$key" =~ ^${vendor_slug}_([0-9]+)_ip$ ]]; then
            local idx="${BASH_REMATCH[1]}"
            if (( idx > max_idx )); then
                max_idx=$idx
            fi
        fi
    done < "$config_file"

    echo $((max_idx + 1))
}

# ----------------------------------------------------------------------------
# reconcile_vendor_counters — Phase 2.5
#
# Recorre el connection.config después de los probes y:
#   1. Para cada vendor presente en el archivo, cuenta cuántos devices tiene
#      realmente (vía `<vendor>_<idx>_ip=`).
#   2. Renumera los devices para que sean contiguos (1..N).
#   3. Ajusta `<vendor>_count=N`.
#   4. Elimina los contadores de vendors que ya no tienen devices.
#
# Esto resuelve el bug donde después de un probe que renombra solo el
# primer device de un vendor, los demás quedaban con índices "huérfanos"
# (e.g. fortinet_1 pero generic_2, generic_3 sin generic_1).
# ----------------------------------------------------------------------------
reconcile_vendor_counters() {
    local config_file="$1"

    if [[ ! -f "$config_file" ]]; then
        return 0
    fi

    local tmpfile="${config_file}.reconcile.$$"

    # ────────────────────────────────────────────────────────
    # Estrategia con awk PORTABLE (compatible con mawk, gawk y busybox):
    #
    # IMPORTANTE: Debian 12 trae mawk por default, que NO soporta
    # `match(str, regex, array)` (extensión GNU). Por eso usamos
    # SOLO `split()` y `substr()` (POSIX).
    #
    # Phase 1: parsear todo el archivo en memoria.
    #   - device_lines[vendor|idx|field] = línea completa
    #   - device_idx_set[vendor|idx] = 1 si existe _ip (ancla)
    #   - vendors_presentes[vendor] = 1
    #   - other[NR] = líneas que no son devices ni contadores
    #
    # Phase 2: por cada vendor, contar idx únicos (basados en _ip),
    #   ordenarlos ascendente, renumerar 1..N.
    # ────────────────────────────────────────────────────────

    awk '
    BEGIN { FS = "=" }
    # Capturar líneas de devices: <vendor>_<idx>_<field>=<value>
    # Usamos split() con "_" y "=" para parsear (POSIX portable).
    {
        line = $0
        # Saltar comentarios y vacías
        if (line ~ /^[[:space:]]*#/ || line ~ /^[[:space:]]*$/) {
            other[NR] = line
            next
        }
        # Extraer la parte antes del "="
        eq_pos = index(line, "=")
        if (eq_pos == 0) {
            other[NR] = line
            next
        }
        key_part = substr(line, 1, eq_pos - 1)

        # Detectar si es un contador <vendor>_count=N (vendor sin idx numérico)
        # Si el último segmento NO es numérico, probablemente es un contador o
        # una línea rara. Detectamos contador como "vendor_count" donde vendor
        # no contiene dígitos y el sufijo es exactamente "count".
        n_parts = split(key_part, kp, "_")
        if (kp[n_parts] == "count" && n_parts == 2) {
            # Es un contador — descartar (lo regeneramos)
            next
        }

        # Si no, buscar el PRIMER segmento numérico — ese es idx.
        # Formato real: <vendor con _>[idx numérico]_<field con _>
        # Ej: fortinet_1_ip, fortinet_1_snmp_version, fortinet_1_v3_user,
        #     generic_2_community, mikrotik_fw_3_v3_priv_password
        idx_pos = 0
        for (i = 1; i <= n_parts; i++) {
            if (kp[i] ~ /^[0-9]+$/) {
                idx_pos = i
                break
            }
        }
        if (idx_pos < 2 || idx_pos >= n_parts) {
            # Sin idx numérico o sin field después del idx → no es device
            other[NR] = line
            next
        }
        idx = kp[idx_pos] + 0

        # vendor = kp[1..idx_pos-1], field = kp[idx_pos+1..n_parts]
        vendor = kp[1]
        for (p = 2; p <= idx_pos - 1; p++) {
            vendor = vendor "_" kp[p]
        }
        field = kp[idx_pos + 1]
        for (p = idx_pos + 2; p <= n_parts; p++) {
            field = field "_" kp[p]
        }

        # Guardar
        composite = vendor SUBSEP idx SUBSEP field
        device_lines[composite] = line
        vendors_presentes[vendor] = 1

        # Si es _ip, marcar este device como existente
        if (field == "ip") {
            device_idx_set[vendor SUBSEP idx] = 1
        }
        next
    }
    END {
        # ────────────────────────────────────────────────────────
        # Phase 2: por cada vendor, listar idx únicos y renumerar
        # ────────────────────────────────────────────────────────
        for (vendor in vendors_presentes) {
            n = 0
            for (key in device_idx_set) {
                split(key, parts, SUBSEP)
                if (parts[1] == vendor) {
                    n++
                    idx_arr[n] = parts[2] + 0
                }
            }
            # Ordenar idx_arr ascendente (insertion sort)
            for (i = 2; i <= n; i++) {
                for (j = i; j > 1 && idx_arr[j-1] > idx_arr[j]; j--) {
                    tmp = idx_arr[j-1]
                    idx_arr[j-1] = idx_arr[j]
                    idx_arr[j] = tmp
                }
            }

            # Emitir línea de contador
            print vendor "_count=" n

            # Emitir los bloques de devices renumerados
            for (new_idx = 1; new_idx <= n; new_idx++) {
                old_idx = idx_arr[new_idx]
                # Recolectar TODOS los fields de este device en orden
                # Estrategia: linear scan del device_lines
                for (key in device_lines) {
                    split(key, parts, SUBSEP)
                    if (parts[1] == vendor && parts[2] + 0 == old_idx) {
                        old_line = device_lines[key]
                        field = parts[3]
                        eq_pos = index(old_line, "=")
                        value = substr(old_line, eq_pos + 1)
                        # Emitir línea renumerada
                        printf "%s_%d_%s=%s\n", vendor, new_idx, field, value
                    }
                }
                # Línea en blanco entre devices para legibilidad
                if (new_idx < n) print ""
            }
            print ""
        }
    }
    ' "$config_file" > "$tmpfile"

    # Si el awk produjo salida vacía (caso raro), no sobrescribir.
    if [[ -s "$tmpfile" ]]; then
        mv "$tmpfile" "$config_file"
        chmod 600 "$config_file"
        log_message "INFO" "Contadores reconciliados y devices renumerados contiguamente"
    else
        rm -f "$tmpfile"
        log_message "WARN" "Reconcile produjo salida vacía — connection.config no modificado"
    fi
}

# ----------------------------------------------------------------------------
# recrypt_sensitive_fields — Phase 2.5.1: re-cifra los campos sensibles del
# connection.config usando el AAD con el device_id ACTUAL (post-probe).
#
# Por qué: `generate_config_file` cifra los campos cuando el config todavía
# tiene `generic_1` como device_id. Después, `post_install_probe_devices`
# renombra `generic_1` → `fortinet_1` (o lo que detecte el probe). Los
# tokens cifrados SE preservan, pero el AAD con que se cifraron era
# `generic_1|<field>`, mientras que al descifrar el binario principal usa
# `fortinet_1|<field>` → AES-GCM AEAD fail.
#
# Solución: después del probe, recorrer el config y re-cifrar los tokens
# `$enc$2$...` con el AAD correcto. Se hace transparente al usuario.
#
# Implementación: por cada línea `<vendor>_<idx>_<field> = $enc$2$...`,
# leer el valor, descifrarlo con el AAD VIEJO (current vendor_idx actual
# del config, que ya es el post-probe), volver a cifrarlo con el AAD
# NUEVO (= mismo vendor_idx porque el config YA está renombrado), y
# reescribir la línea.
#
# Espera, eso no es correcto. Vamos paso a paso:
#  1. El config tiene `fortinet_1_v3_auth_password = $enc$2$XXX` donde
#     XXX fue cifrado con AAD = "generic_1|v3_auth_password".
#  2. device_id actual = "fortinet_1" (post-probe).
#  3. Para re-cifrar necesito el plaintext. Lo descifro con el AAD viejo
#     ("generic_1|v3_auth_password"), y lo vuelvo a cifrar con el AAD
#     nuevo ("fortinet_1|v3_auth_password").
#
# Como no tenemos la pass original a mano, el instalador debe poder
# descifrar y recifrar. La forma más simple: leer el config, identificar
# tokens $enc$2$..., y reemplazar `generic_<idx>` por el vendor_<idx>
# ACTUAL antes de pasarlo al descifrador del binario.
#
# Otra forma más limpia: hacer el cifrado SNMPv3 DESPUÉS del probe (no
# antes). Pero eso requiere refactorizar `generate_config_file` para
# que pueda llamarse en dos pasadas. Demasiado invasivo.
#
# Forma más pragmática: en el propio `post_install_probe_devices`, después
# del rename, re-cifrar los campos sensibles. La pass se descifra usando
# el device_id VIEJO (= old_slug) y se cifra con el NUEVO (= new_slug).
# Para esto, NECESITAMOS una forma de pedir al binario que descifre con
# un AAD arbitrario. Hoy no existe — solo descifra on-demand desde
# config.rs (que usa device_id actual).
#
# Workaround definitivo: agregar al binario un subcomando
# `ness-relay recrypt-field <old_aad> <new_aad>` que lea el config,
# identifique tokens $enc$2$..., los descifre con el AAD viejo y los
# recifre con el AAD nuevo. Por device, por campo.
# ----------------------------------------------------------------------------
recrypt_sensitive_fields() {
    local config_file="$1"
    local relay_bin="$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME"
    local -A processed=()
    local recrypted=0
    local skipped=0
    local errors=0

    # Detectar pares <vendor>_<idx> presentes en el config.
    # Recorremos una vez para saber todos los device_id.
    local -a device_ids=()
    while IFS='=' read -r key _; do
        key=$(echo "$key" | xargs)
        if [[ "$key" =~ ^([a-z][a-z0-9_]*)_([0-9]+)_(v3_auth_password|v3_priv_password)$ ]]; then
            local v="${BASH_REMATCH[1]}"
            local i="${BASH_REMATCH[2]}"
            local composite="${v}_${i}"
            # Dedupe
            local found=false
            for existing in "${device_ids[@]:-}"; do
                if [[ "$existing" == "$composite" ]]; then found=true; break; fi
            done
            if [[ "$found" == "false" ]]; then
                device_ids+=("$composite")
            fi
        fi
    done < "$config_file"

    # Por cada device, intentar re-cifrar.
    for composite_key in "${device_ids[@]:-}"; do
        [[ -z "$composite_key" ]] && continue
        local v="${composite_key%_*}"
        local i="${composite_key##*_}"

        for field in v3_auth_password v3_priv_password; do
            local full_key="${composite_key}_${field}"
            # Extraer el valor actual
            local current_val
            current_val=$(grep -E "^${full_key}\s*=" "$config_file" | head -1 | sed -E "s/^${full_key}\s*=\s*//" | xargs)
            [[ -z "$current_val" ]] && continue
            # Si no es un token cifrado, skip
            [[ "$current_val" != \$enc\$* ]] && { skipped=$((skipped+1)); continue; }

            # Phase 2.5.2: el token se cifró originalmente con AAD =
            # "<v>_generic|i>_<field>" (= antes del probe, ej:
            # "generic_1|v3_auth_password"). Ahora el config ya está
            # renombrado a "fortinet_1" pero el token sigue con el AAD
            # viejo.
            #
            # La solución correcta NO es intentar descifrar con AAD nuevo
            # (que falla, como vimos). Es cifrar DESDE CERO con el AAD
            # nuevo. Para eso necesitamos el plaintext, que solo existe
            # en el momento del cifrado original (en generate_config_file).
            #
            # Workaround pragmático: detectar el AAD viejo y re-cifrar
            # con el AAD nuevo. Como el AAD viejo siempre fue
            # "generic_<idx>|" (porque ese era el device_id antes del
            # probe), basta con probar ese AAD.
            local old_composite_key="generic_${i}"
            local plain
            plain=$(echo "$current_val" | "$relay_bin" decrypt-field "$old_composite_key" "$field" 2>/dev/null) || plain=""

            if [[ -z "$plain" ]]; then
                # El AAD viejo tampoco funciona. Esto es raro pero puede
                # pasar si el config ya estaba en fortinet_N (re-instalación).
                # Intentamos con el AAD actual por si acaso.
                plain=$(echo "$current_val" | "$relay_bin" decrypt-field "$composite_key" "$field" 2>/dev/null) || plain=""
            fi

            if [[ -z "$plain" ]]; then
                log_message "WARN" "No se pudo re-cifrar $full_key (AAD mismatch irreparable). Vuelva a correr install_relay.sh o use migrate-plaintext."
                errors=$((errors+1))
                continue
            fi

            # Re-cifrar con el AAD actual (= post-probe).
            local new_token
            new_token=$(printf '%s' "$plain" | "$relay_bin" encrypt-field "$composite_key" "$field" 2>/dev/null) || {
                log_message "WARN" "Fallo re-cifrando $full_key"
                errors=$((errors+1))
                continue
            }

            # Reemplazar en el config (preservando espacios/indentación).
            sed -i "s|^${full_key}\s*=.*|${full_key} = ${new_token}|" "$config_file"
            recrypted=$((recrypted+1))
        done
    done

    if [[ $recrypted -gt 0 ]]; then
        log_message "INFO" "Re-cifrados $recrypted campo(s) sensible(s) con AAD post-probe (Phase 2.5.1)"
    fi
    if [[ $errors -gt 0 ]]; then
        log_message "WARN" "$errors campo(s) no se pudieron re-cifrar (AAD mismatch — re-instale o migre manualmente)"
    fi
}

# ----------------------------------------------------------------------------
# prompt_ssh_credentials_for_device — pide credenciales SSH para un device
# específico y las escribe en DEVICE_CONFIGS (que luego serán persistidas por
# generate_config_file).
#
# El operador puede elegir entre:
#   (a) Ingresar la password AHORA — se guarda en /etc/ness_relay/secrets.env
#       con permisos 600 y la carga automática desde audit_relay.sh
#   (b) Saltar (Enter vacío) — el operador exporta la env var manualmente
#       en ~/.bashrc o en /etc/ness_relay/secrets.env más tarde
# ----------------------------------------------------------------------------
prompt_ssh_credentials_for_device() {
    local vendor_slug="$1"
    local device_idx="$2"
    local device_ip="$3"
    local config_key="${vendor_slug}_${device_idx}"

    echo ""
    echo -e "${BOLD}─── Vendor compatible con auditoría detectada: $vendor_slug ───${NC}"
    echo -e "    ${WHITE}Dispositivo: $device_ip${NC}"
    echo ""

    echo -ne "${WHITE}  👤 Usuario SSH [default: admin]: ${NC}"
    read ssh_user
    ssh_user=${ssh_user:-admin}

    echo -ne "${WHITE}  🔌 Puerto SSH [default: 22]: ${NC}"
    read ssh_port
    ssh_port=${ssh_port:-22}

    local default_env="NESS_SSH_PASSWORD_$(echo "$vendor_slug" | tr '[:lower:]' '[:upper:]')_${device_idx}"
    echo ""
    echo -e "  ${WHITE}Para que el cron de auditoría (cada 6h) pueda abrir sesión${NC}"
    echo -e "  ${WHITE}SSH contra el dispositivo, la contraseña debe estar disponible${NC}"
    echo -e "  ${WHITE}en el archivo ${BOLD}/etc/ness_relay/secrets.env${NC}${WHITE} (chmod 600).${NC}"
    echo ""
    # Phase 2.7: permitir ver/ocultar la contraseña SSH mientras se escribe
    echo -ne "${WHITE}  👁️  ¿Desea ver la contraseña mientras la escribe? (Y/n): ${NC}"
    read show_pw
    if [[ "$show_pw" =~ ^[Yy]$ ]] || [[ -z "$show_pw" ]]; then
        ssh_show_mode="visible"
        echo -e "  ${DIM}(modo visible: la contraseña se mostrará mientras la escribe)${NC}"
    else
        ssh_show_mode="oculto"
        echo -e "  ${DIM}(modo oculto: la contraseña NO se mostrará)${NC}"
    fi
    echo ""
    # Phase 2.7: confirmación de contraseña SSH (doble input, OBLIGATORIO)
    echo -e "  ${DIM}(la contraseña SSH es obligatoria para que la auditoría funcione)${NC}"
    ssh_password=$(prompt_password_with_confirm \
        "Contraseña SSH para $ssh_user@$device_ip" \
        "" \
        "$ssh_show_mode" \
        "false")
    local pwd_rc=$?
    if [[ $pwd_rc -ne 0 ]]; then
        # Demasiados intentos fallidos — la contraseña SSH es OBLIGATORIA
        # porque sin ella la auditoría no puede ejecutarse.
        log_message "ERROR" "No se proporcionó contraseña SSH. Auditoría cancelada para $device_ip."
        return 1
    fi
    if [[ -z "$ssh_password" ]]; then
        log_message "ERROR" "La contraseña SSH está vacía. Auditoría cancelada para $device_ip."
        return 1
    fi

    if [[ -z "$ssh_password" ]]; then
        log_message "WARN" "Contraseña omitida — auditoría no podrá ejecutarse hasta que la contraseña esté disponible."
        log_message "INFO" "Para activarla más tarde: export $default_env='...' && sudo /opt/ness_relay/executables/audit_relay.sh"
        ssh_pw_env="$default_env"
    else
        # Phase 2.5.1: ya NO escribimos un `secrets.env` en plano. La pass
        # se cifra on-demand en /etc/ness_relay/secrets.enc mediante
        # `ness-relay set <env_var>` (subcomando del binario principal,
        # AES-256-GCM, clave maestra derivada del host).
        install -m 700 -d /etc/ness_relay
        ssh_pw_env="$default_env"

        # Llamar a `ness-relay set` con la pass por stdin. La pass
        # NUNCA aparece en el shell history porque:
        #  - El operador la tipea en un read -s.
        #  - ness-relay (subcomando `set`) la lee con read -s y NO la loguea.
        #  - Se cifra inmediatamente con AES-256-GCM antes de tocar disco.
        #
        # Phase 2.5.1: antes invocábamos un binario separado (`ness-relay-x86_64-cred`).
        # Ahora todo vive como subcomandos del binario principal `ness-relay`,
        # simplificando la distribución (un solo binario estático).
        #
        # -y (--yes) salta la confirmación interactiva porque ya pasamos
        # la pass por stdin. Sin este flag, el binario pediría confirmación
        # y como stdin ya está consumido por la pass, fallaría con
        # "confirmación no coincide" (bug arreglado en Phase 2.5.1).
        if "$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME" set -y "$default_env" \
            < <(printf '%s' "$ssh_password") >/dev/null 2>&1; then
            log_message "SUCCESS" "Contraseña SSH cifrada en /etc/ness_relay/secrets.enc (AES-256-GCM)"
        else
            # Fallback: si el subcomando `set` falla (ej: binario v2.4.0
            # antiguo sin subcomandos), escribimos a secrets.env plano
            # con un warning explícito. Esto preserva la operatividad
            # del agente mientras se completa la migración.
            local secrets_file="/etc/ness_relay/secrets.env"
            if [[ ! -f "$secrets_file" ]]; then
                echo "# NESS RELAY — Secretos SSH (chmod 600, root:root) [FALLBACK PLANO, MIGRAR]" > "$secrets_file"
                echo "# MIGRE con: $INSTALL_DIR/executables/$INSTALLED_BINARY_NAME migrate-plaintext" >> "$secrets_file"
                echo "# Generado el $(date)" >> "$secrets_file"
                echo "" >> "$secrets_file"
            fi
            if grep -qE "^export ${default_env}=" "$secrets_file"; then
                sed -i "s|^export ${default_env}=.*|export ${default_env}='$(escape_sed_replacement "$ssh_password")'|" "$secrets_file"
            else
                echo "export ${default_env}='$ssh_password'" >> "$secrets_file"
            fi
            chmod 600 "$secrets_file"
            chown root:root "$secrets_file"
            log_message "WARN" "Fallback: pass SSH en plano en $secrets_file (migre con ness-relay migrate-plaintext)"
        fi
    fi

    DEVICE_CONFIGS["${config_key}_ssh_enabled"]="true"
    DEVICE_CONFIGS["${config_key}_ssh_username"]="$ssh_user"
    DEVICE_CONFIGS["${config_key}_ssh_port"]="$ssh_port"
    DEVICE_CONFIGS["${config_key}_ssh_password_env"]="$ssh_pw_env"

    echo ""
    echo -e "${GREEN}  ✓ Credenciales SSH configuradas para $device_ip${NC}"
    if [[ -n "$ssh_password" ]]; then
        echo -e "${DIM}    La próxima ejecución del cron de auditoría (cada 6h) abrirá${NC}"
        echo -e "${DIM}    sesión SSH automáticamente usando $secrets_file.${NC}"
    else
        echo -e "${YELLOW}    ⚠ Sin contraseña — la auditoría se omitirá hasta que la${NC}"
        echo -e "${YELLOW}      exportes manualmente:${NC}"
        echo -e "${CYAN}      sudo mkdir -p /etc/ness_relay && sudo bash -c 'echo \"export ${default_env}=\\\"TU_PASSWORD\\\"\" > /etc/ness_relay/secrets.env && chmod 600 /etc/ness_relay/secrets.env'${NC}"
    fi
    echo ""

    # Persistir inmediatamente: añadir las 4 keys SSH al archivo
    local config_file="$INSTALL_DIR/configs/connection.config"
    if [[ -f "$config_file" ]]; then
        # Insertar antes de líneas en blanco / comentarios finales
        {
            echo ""
            echo "# SSH audit (Phase 2.4 — post-install probe)"
            echo "${config_key}_ssh_enabled=true"
            echo "${config_key}_ssh_username=${ssh_user}"
            echo "${config_key}_ssh_port=${ssh_port}"
            echo "${config_key}_ssh_password_env=${ssh_pw_env}"
        } >> "$config_file"
        chmod 600 "$config_file"
    fi
}

# Llamar al prompt DESPUÉS de agregar todos los dispositivos.
# En modo silent, queda en false.

# Función para cargar configuración desde archivo
load_config_file() {
    local config_source="$1"
    local config_file="$config_source"
    local temp_download_file=""

    if [[ "$config_source" =~ ^https?:// ]]; then
        temp_download_file="$(mktemp /tmp/ness_relay_config_XXXXXX.config)"
        log_message "PROGRESS" "Descargando configuración desde: $config_source"
        if ! download_file "$config_source" "$temp_download_file"; then
            log_message "ERROR" "No se pudo descargar la configuración desde: $config_source"
            rm -f "$temp_download_file" 2>/dev/null || true
            exit 1
        fi
        config_file="$temp_download_file"
    fi

    if [[ ! -f "$config_file" ]]; then
        log_message "ERROR" "Archivo de configuración no encontrado: $config_file"
        [[ -n "$temp_download_file" ]] && rm -f "$temp_download_file" 2>/dev/null || true
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

    if [[ -n "$temp_download_file" ]]; then
        rm -f "$temp_download_file" 2>/dev/null || true
    fi
}

# Función para generar archivo de configuración
generate_config_file() {
    local config_file="$INSTALL_DIR/configs/connection.config"

    # En actualización, preservar exactamente el archivo previo si existe.
    if [[ "$UPDATE_ONLY_MODE" == "true" && -n "$PRESERVED_CONFIG_SOURCE" && -f "$PRESERVED_CONFIG_SOURCE" ]]; then
        if [[ "$PRESERVED_CONFIG_SOURCE" == "$config_file" ]]; then
            log_message "SUCCESS" "Configuración preservada sin cambios en: $config_file"
        else
            cp "$PRESERVED_CONFIG_SOURCE" "$config_file"
            log_message "SUCCESS" "Configuración previa restaurada en: $config_file"
        fi
        return
    fi

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

        while IFS= read -r vendor; do
            [[ -z "$vendor" ]] && continue
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
                    # Phase 2.5.1: orden explícito y determinístico para que las
                    # pass cifradas ($enc$2$...) se lean de forma predecible.
                    # Antes el orden dependía del orden de inserción en el
                    # HashMap `DEVICE_CONFIGS`, lo que producía salidas como
                    # `priv_password ... auth_password` al final.
                    echo "${config_key}_ip=${DEVICE_CONFIGS[${config_key}_ip]}"
                    echo "${config_key}_port=${DEVICE_CONFIGS[${config_key}_port]}"
                    echo "${config_key}_vendor=${DEVICE_CONFIGS[${config_key}_vendor]}"
                    echo "${config_key}_description=${DEVICE_CONFIGS[${config_key}_description]}"
                    echo "${config_key}_snmp_version=$snmp_ver"

                    if [[ "$snmp_ver" == "1" ]] || [[ "$snmp_ver" == "2c" ]]; then
                        # SNMPv1 o SNMPv2c — solo requieren community string
                        # Phase 2.5.0: cifrar la community si NO es "public"
                        # (la mayoría de los agentes usan default plano, así
                        # que no las ciframos para mantener el config legible).
                        local comm="${DEVICE_CONFIGS[${config_key}_community]}"
                        echo "${config_key}_community=${comm}"
                    else
                        # SNMPv3 — requiere credenciales completas
                        # Phase 2.5.1: cifrar v3_auth_password y v3_priv_password
                        # al vuelo con el subcomando `encrypt-field` del binario
                        # principal `ness-relay` (ya NO hay binario separado).
                        # La pass NUNCA toca disco en plano.
                        local relay_bin="$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME"
                        local auth_p="${DEVICE_CONFIGS[${config_key}_v3_auth_password]}"
                        local priv_p="${DEVICE_CONFIGS[${config_key}_v3_priv_password]}"
                        local auth_enc=""
                        local priv_enc=""
                        if [[ -x "$relay_bin" ]] && [[ -n "$auth_p" ]]; then
                            auth_enc="$(printf '%s' "$auth_p" | "$relay_bin" encrypt-field "$config_key" v3_auth_password 2>/dev/null)" \
                                || auth_enc="$auth_p"
                        else
                            auth_enc="$auth_p"
                        fi
                        if [[ -x "$relay_bin" ]] && [[ -n "$priv_p" ]]; then
                            priv_enc="$(printf '%s' "$priv_p" | "$relay_bin" encrypt-field "$config_key" v3_priv_password 2>/dev/null)" \
                                || priv_enc="$priv_p"
                        else
                            priv_enc="$priv_p"
                        fi
                        echo "${config_key}_v3_user=${DEVICE_CONFIGS[${config_key}_v3_user]}"
                        echo "${config_key}_v3_auth_protocol=${DEVICE_CONFIGS[${config_key}_v3_auth_protocol]}"
                        echo "${config_key}_v3_auth_password=${auth_enc}"
                        echo "${config_key}_v3_priv_protocol=${DEVICE_CONFIGS[${config_key}_v3_priv_protocol]}"
                        echo "${config_key}_v3_priv_password=${priv_enc}"
                    fi

                    # ────────────────────────────────────────────────────
                    # Phase 2.4 — bloque SSH (opt-in audit)
                    # Solo se emite si `collect_ssh_credentials` pobló las
                    # claves `_ssh_*` en `DEVICE_CONFIGS`. Bug previo: este
                    # bloque no existía; las credenciales SSH recolectadas
                    # nunca se persistían a connection.config.
                    # ────────────────────────────────────────────────────
                    if [[ "${DEVICE_CONFIGS[${config_key}_ssh_enabled]:-}" == "true" ]]; then
                        echo "# SSH audit (Phase 2.4)"
                        echo "${config_key}_ssh_enabled=true"
                        echo "${config_key}_ssh_username=${DEVICE_CONFIGS[${config_key}_ssh_username]}"
                        echo "${config_key}_ssh_port=${DEVICE_CONFIGS[${config_key}_ssh_port]}"
                        echo "${config_key}_ssh_password_env=${DEVICE_CONFIGS[${config_key}_ssh_password_env]}"
                    fi
                done
                echo ""
            fi
        done < <(get_configured_vendors)
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
        --update-only)
            UPDATE_ONLY_MODE=true
            SILENT_MODE=true
            shift
            ;;
        --help)
            echo "Uso: sudo ./install_relay.sh [opciones]"
            echo ""
            echo "Opciones:"
            echo "  --silent               Instalar en modo silencioso (sin menús)"
            echo "  --config-file FILE     Usar archivo de configuración existente o una URL"
            echo "  --force                Forzar instalación sobre existente"
            echo "  --token TOKEN          Token de API de NESS HQ"
            echo "  --env ENV_ID           ID del servidor (1=On-premise, 2=Testing, 3=Cloud)"
            echo "  --verify-setup         Ejecutar solo Smart Tester y salir"
            echo "  --update-only          Modo actualización: solo reemplaza binarios (sin reconfiguración)"
            echo "  --help                 Mostrar esta ayuda"
            echo ""
            echo "Modo interactivo (recomendado):"
            echo "  sudo ./install_relay.sh"
            echo ""
            echo "Modo silencioso:"
            echo "  sudo ./install_relay.sh --silent --config-file connection.config --token TU_TOKEN --env 3"
            echo ""
            echo "Modo guiado (frontend, sin preguntas):"
            echo "  sudo NESS_GUIDED_INSTALL=true NESS_TOKEN=... NESS_SERVER_ID=3 NESS_RELAY_DEVICE_IP=10.0.0.5 NESS_RELAY_SNMP_VERSION=2c ./install_relay.sh"
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

# ═══════════════════════════════════════════════════════════════════════════
# LEER VARIABLES DE ENTORNO (GUIDED INSTALLATION MODE)
# ═══════════════════════════════════════════════════════════════════════════
# Si no se pasó --token, buscar NESS_TOKEN en el entorno (frontend mode)
if [[ -z "$API_TOKEN" && -n "$NESS_TOKEN" ]]; then
    API_TOKEN="$NESS_TOKEN"
    log_message "INFO" "Token obtenido de variable de entorno NESS_TOKEN"
fi

# Si no se pasó --env, buscar NESS_SERVER_ID en el entorno (frontend mode)
if [[ -n "$NESS_SERVER_ID" ]]; then
    case "$NESS_SERVER_ID" in
        1|2|3)
            SERVER_ENV="$NESS_SERVER_ID"
            log_message "INFO" "Servidor obtenido de variable de entorno NESS_SERVER_ID=$NESS_SERVER_ID"
            ;;
        *)
            log_message "WARNING" "NESS_SERVER_ID inválido ($NESS_SERVER_ID). Se mantiene valor actual: $SERVER_ENV"
            ;;
    esac
fi

# Permitir que el frontend pase una URL temporal de connection.config.
if [[ -z "$CONFIG_FILE" && -n "$NESS_DEVICES_FILE_URL" ]]; then
    CONFIG_FILE="$NESS_DEVICES_FILE_URL"
    SILENT_MODE=true
    GUIDED_MODE=true
    log_message "INFO" "Configuración de dispositivos obtenida desde NESS_DEVICES_FILE_URL"
fi

# Capturar NESS_TIME si está disponible (para auto-configurar cron, en minutos)
if [[ -n "$NESS_TIME" ]]; then
    CRON_INTERVAL="$NESS_TIME"
    log_message "INFO" "Intervalo de ejecución solicitado: $NESS_TIME minutos"
fi

# Activar modo guiado automáticamente cuando llegan variables del frontend
if [[ -z "$CONFIG_FILE" && ( "${NESS_GUIDED_INSTALL:-}" == "true" || -n "$NESS_RELAY_DEVICE_IP" ) ]]; then
    GUIDED_MODE=true
    SILENT_MODE=true
    log_message "INFO" "Modo guiado detectado por variables de entorno"
    setup_guided_configuration_from_env
fi

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
    log_message "WARNING" "No se encontró binario local. Intentando descarga guiada desde metadata..."

    if [[ -z "$HOST_ARCH_SUFFIX" ]]; then
        log_message "ERROR" "Arquitectura no soportada para descarga automática: $HOST_ARCH_RAW"
        exit 1
    fi

    if ! download_binary_from_metadata "$HOST_ARCH_SUFFIX"; then
        log_message "ERROR" "No se pudo obtener un binario compatible automáticamente"
        echo ""
        echo -e "${YELLOW}${BOLD}Asegúrese de:${NC}"
        echo -e "  ${WHITE}1.${NC} Haber compilado el agente con ${CYAN}build_relay.sh${NC}"
        echo -e "  ${WHITE}2.${NC} El binario compilado debe estar en ${CYAN}dist/${EXEC_NAME}${NC}, ${CYAN}dist/${EXEC_NAME}-x86_64${NC}, ${CYAN}dist/${EXEC_NAME}-aarch64${NC} o en este directorio"
        echo -e "  ${WHITE}3.${NC} O publicar correctamente ${CYAN}latest.json${NC} y binarios en el bucket"
        echo ""
        exit 1
    fi
fi

log_message "SUCCESS" "Ejecutable '${BINARY_NAME_SELECTED}' encontrado: ${BINARY_SOURCE}"
INSTALLED_BINARY_NAME="$BINARY_NAME_SELECTED"
detect_source_package_dir "$SCRIPT_DIR" "$BINARY_SOURCE"
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

# ----------------------------------------------------------------------------
# _store_autocompleted_device
#
# Helper que guarda un dispositivo en los arrays DEVICE_CONFIGS / SELECTED_VENDORS
# con la misma convención que el instalador usa en otros lugares
# (`${vendor}_count`, `${vendor}_${idx}_<campo>`).
#
# Argumentos:
#   $1 vendor             (ej: "generic", "fortinet", "pfsense")
#   $2 idx                (1..N, índice del dispositivo)
#   $3 ip                 (dirección IP)
#   $4 port               (puerto SNMP, default 161)
#   $5 snmp_version       ("1", "2c" o "3")
#   $6 description        (descripción opcional)
#   $7 community          (v1/v2c: community string; v3: "")
#   $8 v3_user            (v3: usuario; v1/v2c: "")
#   $9 v3_auth_protocol   (v3: "MD5"/"SHA"/"SHA256"/"SHA384"/"SHA512"/"NONE")
#  $10 v3_auth_password   (v3)
#  $11 v3_priv_protocol   (v3)
#  $12 v3_priv_password   (v3)
# ----------------------------------------------------------------------------
_store_autocompleted_device() {
    local vendor="$1"
    local idx="$2"
    local ip="$3"
    local port="$4"
    local snmp_version="$5"
    local description="$6"
    local community="$7"
    local v3_user="$8"
    local v3_auth_protocol="$9"
    local v3_auth_password="${10}"
    local v3_priv_protocol="${11}"
    local v3_priv_password="${12}"

    local key="${vendor}_${idx}"
    SELECTED_VENDORS["$vendor"]="true"

    # Sumar al contador existente (multi-device por vendor)
    local current_count="${DEVICE_CONFIGS[${vendor}_count]:-0}"
    if [[ "$idx" -gt "$current_count" ]]; then
        DEVICE_CONFIGS["${vendor}_count"]="$idx"
    fi

    DEVICE_CONFIGS["${key}_ip"]="$ip"
    DEVICE_CONFIGS["${key}_port"]="${port:-161}"
    DEVICE_CONFIGS["${key}_description"]="$description"
    DEVICE_CONFIGS["${key}_vendor"]="$vendor"
    DEVICE_CONFIGS["${key}_snmp_version"]="$snmp_version"

    if [[ "$snmp_version" == "1" || "$snmp_version" == "2c" ]]; then
        DEVICE_CONFIGS["${key}_community"]="${community:-public}"
    elif [[ "$snmp_version" == "3" ]]; then
        DEVICE_CONFIGS["${key}_v3_user"]="$v3_user"
        DEVICE_CONFIGS["${key}_v3_auth_protocol"]="$v3_auth_protocol"
        DEVICE_CONFIGS["${key}_v3_auth_password"]="$v3_auth_password"
        DEVICE_CONFIGS["${key}_v3_priv_protocol"]="$v3_priv_protocol"
        DEVICE_CONFIGS["${key}_v3_priv_password"]="$v3_priv_password"
    fi
}

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
# Si el Smart Tester validó dispositivos manualmente, ofrecer autocompletado
if [[ "$SILENT_MODE" != "true" && -f "$AUTOCOMPLETE_FILE" ]]; then
    echo ""
    print_box "AUTOCOMPLETADO DISPONIBLE" "${GREEN}${BOLD}"
    echo ""

    # ----------------------------------------------------------------------
    # Phase 2.5: parsear el archivo de autocompletado multi-device.
    #
    # Formato del archivo (generado por el Smart Tester):
    #   [device_1]
    #   ip=192.168.10.17
    #   port=161
    #   snmp_version=3
    #   v3_user=...
    #   ...
    #
    #   [device_2]
    #   ip=192.168.10.18
    #   ...
    #
    # Construimos arrays asociativos indexados por número de dispositivo:
    #   AC_DEVICES_IP[1], AC_DEVICES_IP[2], ...
    #   AC_DEVICES_PORT[N], AC_DEVICES_SNMP_VERSION[N], ...
    # ----------------------------------------------------------------------
    declare -A AC_DEVICES_IP=()
    declare -A AC_DEVICES_PORT=()
    declare -A AC_DEVICES_SNMP_VERSION=()
    declare -A AC_DEVICES_COMMUNITY=()
    declare -A AC_DEVICES_V3_USER=()
    declare -A AC_DEVICES_V3_AUTH_PROTOCOL=()
    declare -A AC_DEVICES_V3_AUTH_PASSWORD=()
    declare -A AC_DEVICES_V3_PRIV_PROTOCOL=()
    declare -A AC_DEVICES_V3_PRIV_PASSWORD=()

    current_device_idx=0
    while IFS='=' read -r ac_key ac_value; do
        # Ignorar líneas vacías y comentarios
        ac_key=$(echo "$ac_key" | xargs)
        [[ -z "$ac_key" || "$ac_key" =~ ^[[:space:]]*# ]] && continue

        # ¿Inicio de nueva sección [device_N]?
        if [[ "$ac_key" =~ ^\[device_([0-9]+)\]$ ]]; then
            current_device_idx="${BASH_REMATCH[1]}"
            continue
        fi

        # Si no estamos dentro de ninguna sección, saltar
        [[ "$current_device_idx" -eq 0 ]] && continue

        # Guardar el campo bajo el índice del device actual
        case "$ac_key" in
            ip)                 AC_DEVICES_IP[$current_device_idx]="$ac_value" ;;
            port)               AC_DEVICES_PORT[$current_device_idx]="$ac_value" ;;
            snmp_version)       AC_DEVICES_SNMP_VERSION[$current_device_idx]="$ac_value" ;;
            community)          AC_DEVICES_COMMUNITY[$current_device_idx]="$ac_value" ;;
            v3_user)            AC_DEVICES_V3_USER[$current_device_idx]="$ac_value" ;;
            v3_auth_protocol)   AC_DEVICES_V3_AUTH_PROTOCOL[$current_device_idx]="$ac_value" ;;
            v3_auth_password)   AC_DEVICES_V3_AUTH_PASSWORD[$current_device_idx]="$ac_value" ;;
            v3_priv_protocol)   AC_DEVICES_V3_PRIV_PROTOCOL[$current_device_idx]="$ac_value" ;;
            v3_priv_password)   AC_DEVICES_V3_PRIV_PASSWORD[$current_device_idx]="$ac_value" ;;
        esac
    done < "$AUTOCOMPLETE_FILE"

    AC_DEVICES_COUNT=${#AC_DEVICES_IP[@]}
    if [[ "$AC_DEVICES_COUNT" -eq 0 ]]; then
        log_message "INFO" "Archivo de autocompletado vacío o sin dispositivos válidos"
        rm -f "$AUTOCOMPLETE_FILE"
    else
        echo -e "${WHITE}El Smart Tester validó correctamente la conexión SNMP con:${NC} ${BOLD}${AC_DEVICES_COUNT}${NC} ${WHITE}dispositivo(s):${NC}"
        echo ""
        for ((ac_idx=1; ac_idx<=AC_DEVICES_COUNT; ac_idx++)); do
            local_ip="${AC_DEVICES_IP[$ac_idx]:-?}"
            local_port="${AC_DEVICES_PORT[$ac_idx]:-161}"
            local_ver="${AC_DEVICES_SNMP_VERSION[$ac_idx]:-?}"
            echo -e "  ${WHITE}[$ac_idx]${NC} IP: ${BOLD}$local_ip${NC}  Puerto: $local_port  SNMP: v$local_ver"
            if [[ "$local_ver" == "3" ]]; then
                echo -e "       Usuario: ${AC_DEVICES_V3_USER[$ac_idx]}  Auth: ${AC_DEVICES_V3_AUTH_PROTOCOL[$ac_idx]}  Priv: ${AC_DEVICES_V3_PRIV_PROTOCOL[$ac_idx]}"
            else
                echo -e "       Community: ${AC_DEVICES_COMMUNITY[$ac_idx]:-public}"
            fi
        done
        echo ""
        echo -e "${WHITE}Puede reutilizar estos datos para ahorrar tiempo en la configuración.${NC}"
        echo -e "${DIM}Solo necesitará seleccionar: servidor y token API.${NC}"
        echo ""
        echo -ne "${YELLOW}${BOLD}¿Desea usar el autocompletado con los datos del Smart Tester? (Y/n): ${NC}"
        read -r USE_AUTOCOMPLETE

        if [[ "$USE_AUTOCOMPLETE" =~ ^[Yy]$ ]] || [[ -z "$USE_AUTOCOMPLETE" ]]; then
            AUTOCOMPLETE_USED=true
            log_message "SUCCESS" "Autocompletado activado con datos del Smart Tester"

            # --- Paso 1: Seleccionar servidor (una sola vez, compartido por todos los dispositivos) ---
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

            # --- Paso 2: Token API (una sola vez, compartido por todos los dispositivos) ---
            echo ""
            while [[ -z "$API_TOKEN" ]]; do
                echo -ne "${BOLD}🔑 Ingresa tu NESS_API_TOKEN: ${NC}"
                read API_TOKEN
                if [[ -z "$API_TOKEN" ]]; then
                    log_message "ERROR" "El token es obligatorio"
                fi
            done
            log_message "SUCCESS" "Token de API configurado"

            # ----------------------------------------------------------------------
            # Paso 3: Bucle multi-device.
            #
            # Los primeros N dispositivos se toman del AUTOCOMPLETE_FILE (los que
            # ya validó el Smart Tester). Después preguntamos al usuario si
            # quiere agregar más dispositivos. Si dice "Y", repetimos el ciclo
            # de captura de SNMP hasta que responda "N".
            #
            # Contadores: la convención del instalador es `${vendor}_count` y
            # `${vendor}_${idx}_<campo>`. Aquí usamos el vendor "generic"
            # detectado automáticamente por el agente, numerando 1..N.
            # ----------------------------------------------------------------------
            AC_VENDOR="generic"
            AC_INDEX=1
            AC_TOTAL=0

            # --- Dispositivos del AUTOCOMPLETE_FILE (1..N) ---
            for ((ac_idx=1; ac_idx<=AC_DEVICES_COUNT; ac_idx++)); do
                AC_INDEX=$ac_idx
                local_ip="${AC_DEVICES_IP[$ac_idx]}"
                local_port="${AC_DEVICES_PORT[$ac_idx]:-161}"
                local_ver="${AC_DEVICES_SNMP_VERSION[$ac_idx]}"
                local_community="${AC_DEVICES_COMMUNITY[$ac_idx]:-}"
                local_v3_user="${AC_DEVICES_V3_USER[$ac_idx]:-}"
                local_v3_auth_protocol="${AC_DEVICES_V3_AUTH_PROTOCOL[$ac_idx]:-}"
                local_v3_auth_password="${AC_DEVICES_V3_AUTH_PASSWORD[$ac_idx]:-}"
                local_v3_priv_protocol="${AC_DEVICES_V3_PRIV_PROTOCOL[$ac_idx]:-}"
                local_v3_priv_password="${AC_DEVICES_V3_PRIV_PASSWORD[$ac_idx]:-}"

                log_message "SUCCESS" "Dispositivo #${AC_INDEX} (del Smart Tester) configurado: $local_ip (SNMP v$local_ver)"
                echo -e "${CYAN}  ℹ️  El agente detectará automáticamente el tipo de dispositivo al conectarse.${NC}"
                echo ""
                echo -ne "${WHITE}  📝 Descripción del dispositivo [opcional]: ${NC}"
                read AC_DESCRIPTION

                _store_autocompleted_device "$AC_VENDOR" "$AC_INDEX" "$local_ip" "$local_port" "$local_ver" "$AC_DESCRIPTION" "$local_community" "$local_v3_user" "$local_v3_auth_protocol" "$local_v3_auth_password" "$local_v3_priv_protocol" "$local_v3_priv_password"
                AC_TOTAL=$((AC_TOTAL + 1))
            done

        # --- Dispositivos adicionales (loop) ---
        ADD_ANOTHER="Y"
        while [[ "$ADD_ANOTHER" =~ ^[Yy]$ ]]; do
            echo ""
            echo -ne "${YELLOW}${BOLD}¿Desea agregar OTRO dispositivo? (Y/n): ${NC}"
            read ADD_ANOTHER
            # Si respondió vacío o 'Y', seguir. Si 'N', terminar.
            ADD_ANOTHER=${ADD_ANOTHER:-Y}
            if [[ ! "$ADD_ANOTHER" =~ ^[Yy]$ ]]; then
                break
            fi

            AC_INDEX=$((AC_INDEX + 1))
            echo ""
            print_box "DISPOSITIVO #${AC_INDEX}" "${WHITE}${BOLD}"
            echo ""

            # IP (obligatoria)
            NEW_IP=""
            while [[ -z "$NEW_IP" ]]; do
                echo -ne "${BOLD}🌐 Dirección IP del dispositivo #${AC_INDEX}: ${NC}"
                read NEW_IP
                if [[ -z "$NEW_IP" ]]; then
                    log_message "ERROR" "La IP es obligatoria"
                fi
            done

            # Versión SNMP
            echo ""
            echo -e "${WHITE}Versión SNMP:${NC}"
            echo -e "  ${WHITE}1)${NC} v1"
            echo -e "  ${WHITE}2)${NC} v2c"
            echo -e "  ${WHITE}3)${NC} v3 ${DIM}(recomendado)${NC}"
            echo -ne "${BOLD}Ingresa 1, 2 o 3 [default: 3]: ${NC}"
            read NEW_SNMP_VERSION
            NEW_SNMP_VERSION=${NEW_SNMP_VERSION:-3}

            # Puerto
            echo ""
            echo -ne "${BOLD}🔌 Puerto SNMP [default: 161]: ${NC}"
            read NEW_PORT
            NEW_PORT=${NEW_PORT:-161}

            # Community o v3 según versión
            NEW_COMMUNITY=""
            NEW_V3_USER=""
            NEW_V3_AUTH_PROTOCOL=""
            NEW_V3_AUTH_PASSWORD=""
            NEW_V3_PRIV_PROTOCOL=""
            NEW_V3_PRIV_PASSWORD=""
            if [[ "$NEW_SNMP_VERSION" == "1" || "$NEW_SNMP_VERSION" == "2c" ]]; then
                echo ""
                echo -ne "${BOLD}🔑 Community string [default: public]: ${NC}"
                read NEW_COMMUNITY
                NEW_COMMUNITY=${NEW_COMMUNITY:-public}
            else
                echo ""
                echo -ne "${BOLD}👤 Usuario SNMPv3: ${NC}"
                read NEW_V3_USER
                while [[ -z "$NEW_V3_USER" ]]; do
                    log_message "ERROR" "El usuario es obligatorio para SNMPv3"
                    echo -ne "${BOLD}👤 Usuario SNMPv3: ${NC}"
                    read NEW_V3_USER
                done
                echo ""
                echo -e "${WHITE}Protocolo de Autenticación:${NC}"
                echo -e "  1) MD5  2) SHA  3) SHA256  4) SHA384  5) SHA512  6) NONE"
                echo -ne "${BOLD}Selecciona 1-6 [default: 2 (SHA)]: ${NC}"
                read AUTH_PROTO_NUM
                AUTH_PROTO_NUM=${AUTH_PROTO_NUM:-2}
                case "$AUTH_PROTO_NUM" in
                    1) NEW_V3_AUTH_PROTOCOL="MD5" ;;
                    2) NEW_V3_AUTH_PROTOCOL="SHA" ;;
                    3) NEW_V3_AUTH_PROTOCOL="SHA256" ;;
                    4) NEW_V3_AUTH_PROTOCOL="SHA384" ;;
                    5) NEW_V3_AUTH_PROTOCOL="SHA512" ;;
                    6) NEW_V3_AUTH_PROTOCOL="NONE" ;;
                    *) NEW_V3_AUTH_PROTOCOL="SHA" ;;
                esac
                echo ""
                if [[ "$NEW_V3_AUTH_PROTOCOL" != "NONE" ]]; then
                    echo -ne "${BOLD}🔑 Auth password (mín. 8 chars): ${NC}"
                    read -s NEW_V3_AUTH_PASSWORD
                    echo ""
                    echo -ne "${BOLD}🔑 Confirmar auth password: ${NC}"
                    read -s NEW_V3_AUTH_PASSWORD_CONFIRM
                    echo ""
                    if [[ "$NEW_V3_AUTH_PASSWORD" != "$NEW_V3_AUTH_PASSWORD_CONFIRM" ]]; then
                        log_message "ERROR" "Las contraseñas no coinciden. Descartando dispositivo #${AC_INDEX}."
                        continue
                    fi
                fi
                echo ""
                echo -e "${WHITE}Protocolo de Privacidad:${NC}"
                echo -e "  1) AES128  2) AES192  3) AES256  4) DES  5) NONE"
                echo -ne "${BOLD}Selecciona 1-5 [default: 1 (AES128)]: ${NC}"
                read PRIV_PROTO_NUM
                PRIV_PROTO_NUM=${PRIV_PROTO_NUM:-1}
                case "$PRIV_PROTO_NUM" in
                    1) NEW_V3_PRIV_PROTOCOL="AES128" ;;
                    2) NEW_V3_PRIV_PROTOCOL="AES192" ;;
                    3) NEW_V3_PRIV_PROTOCOL="AES256" ;;
                    4) NEW_V3_PRIV_PROTOCOL="DES" ;;
                    5) NEW_V3_PRIV_PROTOCOL="NONE" ;;
                    *) NEW_V3_PRIV_PROTOCOL="AES128" ;;
                esac
                if [[ "$NEW_V3_PRIV_PROTOCOL" != "NONE" ]]; then
                    echo ""
                    echo -ne "${BOLD}🔑 Priv password (mín. 8 chars): ${NC}"
                    read -s NEW_V3_PRIV_PASSWORD
                    echo ""
                    echo -ne "${BOLD}🔑 Confirmar priv password: ${NC}"
                    read -s NEW_V3_PRIV_PASSWORD_CONFIRM
                    echo ""
                    if [[ "$NEW_V3_PRIV_PASSWORD" != "$NEW_V3_PRIV_PASSWORD_CONFIRM" ]]; then
                        log_message "ERROR" "Las contraseñas no coinciden. Descartando dispositivo #${AC_INDEX}."
                        continue
                    fi
                fi
            fi

            echo ""
            echo -ne "${WHITE}  📝 Descripción del dispositivo [opcional]: ${NC}"
            read NEW_DESCRIPTION

            _store_autocompleted_device "$AC_VENDOR" "$AC_INDEX" "$NEW_IP" "$NEW_PORT" "$NEW_SNMP_VERSION" "$NEW_DESCRIPTION" "$NEW_COMMUNITY" "$NEW_V3_USER" "$NEW_V3_AUTH_PROTOCOL" "$NEW_V3_AUTH_PASSWORD" "$NEW_V3_PRIV_PROTOCOL" "$NEW_V3_PRIV_PASSWORD"
            AC_TOTAL=$((AC_TOTAL + 1))
            log_message "SUCCESS" "Dispositivo #${AC_INDEX} agregado: ${NEW_IP} (total: ${AC_TOTAL})"
        done

        log_message "SUCCESS" "Autocompletado finalizado con ${AC_TOTAL} dispositivo(s)"
    fi
    fi

    # Limpiar archivo de autocompletado (contiene contraseñas)
    rm -f "$AUTOCOMPLETE_FILE"
fi

# Selección de fabricantes (flujo normal, solo si no se usó autocompletado)
# Prioridad de carga del connection.config (de mayor a menor):
#   1. NESS_DEVICES_FILE_URL (URL firmada del bulk upload) — gana sobre la config existente
#   2. --update-only con config previa
#   3. Autocompletado del Smart Tester
#   4. Variables de instalación guiada (modo single)
#   5. --silent con --config-file
if [[ -n "$NESS_DEVICES_FILE_URL" && -n "$CONFIG_FILE" ]]; then
    # Bulk upload: la URL firmada siempre tiene prioridad absoluta.
    log_message "INFO" "Cargando connection.config desde URL firmada (bulk upload)"
    load_config_file "$CONFIG_FILE"
elif [[ "$UPDATE_ONLY_MODE" == "true" ]]; then
    log_message "INFO" "Modo actualización (--update-only): usando configuración existente"
    local_existing_config="$EXISTING_INSTALL_DIR/configs/connection.config"
    if [[ -f "$local_existing_config" ]]; then
        log_message "PROGRESS" "Recargando connection.config existente antes de regenerarlo..."
        PRESERVED_CONFIG_SOURCE="$local_existing_config"
        load_config_file "$local_existing_config"
    else
        log_message "WARNING" "No se encontró connection.config previo en $local_existing_config"
    fi
elif [[ "$AUTOCOMPLETE_USED" == "true" ]]; then
    log_message "INFO" "Configuración completada vía autocompletado del Smart Tester"
elif [[ "$GUIDED_MODE" == "true" ]]; then
    log_message "INFO" "Configuración completada vía variables de instalación guiada"
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
    interactive_device_configuration
else
    log_message "ERROR" "En modo silencioso debe especificar --config-file o usar variables de instalación guiada"
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

# ═══════════════════════════════════════════════════════════════════════════
# AUTO-CONFIGURAR CRON DESDE NESS_TIME
# ═══════════════════════════════════════════════════════════════════════════
# Si NESS_TIME se proporcionó como variable de entorno, generar cron expression
if [[ -n "$CRON_INTERVAL" && "$CRON_INTERVAL" =~ ^[0-9]+$ ]]; then
    if (( CRON_INTERVAL <= 10 )); then
        FINAL_CRON="*/$CRON_INTERVAL * * * *"
    elif (( CRON_INTERVAL <= 60 )); then
        local_interval=$(( (CRON_INTERVAL + 4) / 5 ))
        FINAL_CRON="*/$local_interval * * * *"
    else
        local_hours=$(( (CRON_INTERVAL + 59) / 60 ))
        FINAL_CRON="0 */$local_hours * * *"
    fi
    CRON_AUTO_CONFIGURED="true"
    log_message "SUCCESS" "Cron configurado automáticamente: $FINAL_CRON"
else
    CRON_AUTO_CONFIGURED="false"
    FINAL_CRON="0 3 * * *"
fi


# En modo update-only, leer variables existentes para preservar el entorno previo
if [[ "$UPDATE_ONLY_MODE" == "true" ]]; then
    load_existing_env_vars
    if [[ -f "/etc/profile.d/ness_relay.sh" ]]; then
        log_message "PROGRESS" "Leyendo configuración existente de /etc/profile.d/ness_relay.sh..."
        source /etc/profile.d/ness_relay.sh
        
        # Cargar token y server ID del archivo de entorno
        if [[ -n "$NESS_API_TOKEN" ]]; then
            API_TOKEN="$NESS_API_TOKEN"
            log_message "SUCCESS" "Token cargado desde configuración existente"
        fi
        
        if [[ -n "$NESS_SERVER_ID" ]]; then
            SERVER_ENV="$NESS_SERVER_ID"
            log_message "SUCCESS" "Server ID cargado desde configuración existente: $SERVER_ENV"
        fi
    else
        log_message "WARNING" "No se encontró configuración existente en /etc/profile.d/ness_relay.sh"
    fi
fi

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
while IFS= read -r vendor; do
    [[ -z "$vendor" ]] && continue
    if [[ "${SELECTED_VENDORS[$vendor]}" == "true" ]]; then
        count="${DEVICE_CONFIGS[${vendor}_count]}"
        echo -e "${GREEN}✓${NC} ${WHITE}$vendor:${NC} ${DIM}$count dispositivo(s)${NC}"
        total_devices=$((total_devices + count))
    fi
done < <(get_configured_vendors)
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
    # En modo actualización, salta menú y va directo a actualizar binarios
    if [[ "$UPDATE_ONLY_MODE" == "true" ]]; then
        log_message "INFO" "Modo actualización (--update-only): actualizando binarios existentes"
        
        BACKUP_DATE=$(date '+%Y%m%d_%H%M%S')
        BACKUP_DIR="/opt/ness_relay_backup_${BACKUP_DATE}"
        log_message "PROGRESS" "Creando backup de binarios actuales..."
        mkdir -p "$BACKUP_DIR"
        
        # Backup solo de ejecutables, no de configuración
        for existing_exec in "ness-relay" "ness-relay-x86_64" "ness-relay-aarch64"; do
            [[ -f "$INSTALL_DIR/executables/$existing_exec" ]] && cp "$INSTALL_DIR/executables/$existing_exec" "$BACKUP_DIR/" 2>/dev/null
        done
        [[ -f "$INSTALL_DIR/executables/install_relay.sh" ]] && cp "$INSTALL_DIR/executables/install_relay.sh" "$BACKUP_DIR/" 2>/dev/null
        log_message "SUCCESS" "Backup de binarios creado: $BACKUP_DIR"
        
        # En modo update-only, procede directamente a actualizar
        reinstall_option="update_only"
    else
    echo ""
    echo -e "${YELLOW}${BOLD}⚠️  INSTALACIÓN EXISTENTE DETECTADA${NC}"
    echo -e "${WHITE}El directorio $INSTALL_DIR ya existe.${NC}"
    echo ""

    if [[ "$SILENT_MODE" == "true" ]]; then
        reinstall_option="2"
        log_message "INFO" "Modo silencioso: actualización de configuración seleccionada automáticamente"
    else
        echo -e "${WHITE}${BOLD}Selecciona una opción:${NC}"
        echo -e "  ${WHITE}1)${NC} ${GREEN}Reinstalar completamente${NC} ${DIM}(elimina todo y crea una instalación nueva)${NC}"
        echo -e "  ${WHITE}2)${NC} ${YELLOW}Actualizar configuración${NC} ${DIM}(mantiene estructura, actualiza configuraciones)${NC}"
        echo -e "  ${WHITE}3)${NC} ${RED}Cancelar instalación${NC}"
        echo ""
        echo -ne "${BOLD}Selecciona una opción (1-3): ${NC}"
        read reinstall_option
    fi
    fi

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

        2|update_only)
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
rm -f "$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME"
cp "${BINARY_SOURCE}" "$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME"
chmod +x "$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME"
log_message "SUCCESS" "Ejecutable instalado en: $INSTALL_DIR/executables/$INSTALLED_BINARY_NAME"

# Actualizar script de instalación también
SCRIPT_SOURCE="${BASH_SOURCE[0]}"
if [[ -f "$SCRIPT_SOURCE" && "$SCRIPT_SOURCE" != "$INSTALL_DIR/executables/install_relay.sh" ]]; then
    log_message "PROGRESS" "Actualizando script de instalación..."
    rm -f "$INSTALL_DIR/executables/install_relay.sh"
    cp "$SCRIPT_SOURCE" "$INSTALL_DIR/executables/install_relay.sh"
    chmod +x "$INSTALL_DIR/executables/install_relay.sh"
    log_message "SUCCESS" "Script de instalación actualizado en: $INSTALL_DIR/executables/install_relay.sh"
fi

###############################################################################
# CONFIGURAR VARIABLES DE ENTORNO
###############################################################################
ENV_FILE="/etc/profile.d/ness_relay.sh"

# En modo update-only, usar valores preservados si están disponibles
if [[ "$UPDATE_ONLY_MODE" == "true" ]]; then
    [[ -n "$PRESERVED_SERVER_ID" ]] && SERVER_ENV="$PRESERVED_SERVER_ID"
    [[ -n "$PRESERVED_API_TOKEN" ]] && API_TOKEN="$PRESERVED_API_TOKEN"
    [[ -n "$PRESERVED_INSTALL_DIR" ]] && INSTALL_DIR="$PRESERVED_INSTALL_DIR"
fi

log_message "PROGRESS" "Configurando variables de entorno..."
{
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# NESS RELAY — Variables de Entorno (Rust Edition)"
    echo "# Generado automáticamente el $(date)"
    if [[ "$UPDATE_ONLY_MODE" == "true" ]]; then
        echo "# MODO ACTUALIZACIÓN: Variables preservadas de instalación anterior"
    fi
    echo "# NOTA: SERVER_ID es un identificador interno (1=On-premise, 2=Testing, 3=Cloud)"
    echo "# Las URLs reales están protegidas dentro del ejecutable compilado"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""
    echo "export NESS_SERVER_ID=\"$SERVER_ENV\""
    echo "export NESS_API_TOKEN=\"$API_TOKEN\""
    echo "export NESS_INSTALL_DIR=\"$INSTALL_DIR\""
    echo "export NESS_DEVICES_FILE=\"$INSTALL_DIR/configs/connection.config\""
    echo "export NESS_OUTPUT_DIR=\"$INSTALL_DIR/output\""
    echo "export NESS_LOG_DIR=\"$INSTALL_DIR/logs\""
    # Phase 2.4 — opt-in audit toggle. Se setea DESPUÉS de prompt_audit_optin
    # (ver bloque "PROMPT OPT-IN" más abajo). Si re-ejecutas este bloque,
    # asegúrate de que el orden sea: prompt_audit_optin → generate_config_file.
    echo "export NESS_AUDIT_ENABLED=\"${ENABLE_AUDIT:-false}\""
} > "$ENV_FILE"
chmod +x "$ENV_FILE"
source "$ENV_FILE"
log_message "SUCCESS" "Variables de entorno configuradas en: $ENV_FILE"

###############################################################################
# PROMPT OPT-IN: ANÁLISIS DE VULNERABILIDADES + CIS (Phase 2.4)
###############################################################################
# En modo interactivo, pregunta si el usuario quiere habilitar el análisis
# SSH de vulnerabilidades/CIS. En silent mode queda en false por defecto.
# Este prompt DEBE ejecutarse ANTES de generar el config + escribir el env
# file final, para que NESS_AUDIT_ENABLED quede con el valor correcto en
# /etc/profile.d/ness_relay.sh.
#
# Capturamos el resultado en stdout (true/false) y lo asignamos a ENABLE_AUDIT
# globalmente. Esto funciona tanto en `source` como en invocación directa.
# Sanitizamos por si acaso (en modo interactivo real, prompt_audit_optin
# solo emite `true` o `false` por stdout; los banners van a stderr).
ENABLE_AUDIT=$(prompt_audit_optin | tail -1 | tr -d '[:space:]')
[[ -z "$ENABLE_AUDIT" || "$ENABLE_AUDIT" != "true" ]] && ENABLE_AUDIT="false"

# Re-escribir el env file con ENABLE_AUDIT ya seteado (el bloque anterior
# lo escribió antes del prompt, con ENABLE_AUDIT indefinido).
# Solo si NO estamos en update-only (ahí preservamos).
if [[ "${UPDATE_ONLY_MODE:-false}" != "true" ]]; then
    log_message "PROGRESS" "Reescribiendo env file con NESS_AUDIT_ENABLED=${ENABLE_AUDIT}..."
    sed -i.bak "s|^export NESS_AUDIT_ENABLED=.*|export NESS_AUDIT_ENABLED=\"${ENABLE_AUDIT}\"|" "$ENV_FILE"
    rm -f "${ENV_FILE}.bak"
    source "$ENV_FILE"
fi

# Si el usuario aceptó, recolectar credenciales SSH para los devices que
# YA tengan vendor compatible con audit. En el flujo interactivo estándar
# todos los devices están como `generic_*` en este punto, así que este
# loop es un no-op; las credenciales SSH se piden después en
# `post_install_probe_devices()` cuando el binario detecta la vendor real.
collect_ssh_credentials

###############################################################################
# GENERAR ARCHIVO DE CONFIGURACIÓN DE DISPOSITIVOS
###############################################################################
generate_config_file

###############################################################################
# POST-INSTALL: DETECTAR VENDORS REALES VÍA --probe Y CONFIGURAR SSH (Phase 2.4)
###############################################################################
# El binario ness-relay implementa auto-detección de vendor vía SNMP
# (sysObjectID + sysDescr). Esta función corre esa detección para cada
# device del connection.config recién generado, reescribe el archivo con
# el slug real (e.g. `fortinet_1_*` en lugar de `generic_1_*`), y si la
# vendor soporta auditoría SSH (Phase 1: fortinet) pregunta las credenciales.
#
# Esto evita que el usuario tenga que conocer el vendor de antemano — la
# fuente de verdad es lo que el binario detecta por SNMP.
#
# En modo silent: skip (el opt-in audit no aplica sin consentimiento).
# En modo update-only: skip (preserva el config existente).
if [[ "${UPDATE_ONLY_MODE:-false}" != "true" && "${ENABLE_AUDIT:-false}" == "true" ]]; then
    post_install_probe_devices
fi

###############################################################################
# SMART TESTER DEEP VALIDATION (CON CONNECTION.CONFIG)
###############################################################################
log_message "PROGRESS" "Ejecutando Smart Tester Deep Validation sobre connection.config..."
if [[ "$SILENT_MODE" == "true" ]]; then
    "$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME" \
        --verify-setup \
        --verify-auto-fix \
        --verify-assume-yes \
        --config "$INSTALL_DIR/configs/connection.config" || true
else
    "$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME" \
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
    echo "    ./executables/$INSTALLED_BINARY_NAME --config $INSTALL_DIR/configs/connection.config"
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
    echo "    ./executables/$INSTALLED_BINARY_NAME --silent --config $INSTALL_DIR/configs/connection.config"
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
# CREAR SCRIPT DE ACTUALIZACIÓN (update_relay.sh)
###############################################################################
UPDATE_SCRIPT="$INSTALL_DIR/executables/update_relay.sh"
UPDATE_LOG_DIR="$INSTALL_DIR/executables/logs"
log_message "PROGRESS" "Creando script de auto-actualización..."
mkdir -p "$UPDATE_LOG_DIR"

{
    echo "#!/bin/bash"
    echo "#"
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# NESS RELAY — Script de Auto-Actualización (Rust Edition)"
    echo "# Generado automáticamente el \$(date)"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""
    echo "source $ENV_FILE"
    echo "cd $INSTALL_DIR"
    echo ""
    echo "LOG_FILE=\"$UPDATE_LOG_DIR/relay-update.log\""
    echo "echo \"[\$(date '+%Y-%m-%d %H:%M:%S')] Verificando actualizaciones...\" >> \"\$LOG_FILE\""
    echo "./executables/$INSTALLED_BINARY_NAME --update --silent >> \"\$LOG_FILE\" 2>&1"
} > "$UPDATE_SCRIPT"

chmod +x "$UPDATE_SCRIPT"
log_message "SUCCESS" "Script de auto-actualización creado: $UPDATE_SCRIPT"

###############################################################################
# CREAR SCRIPT DE AUDITORÍA (audit_relay.sh) — Phase 2.4
###############################################################################
# Solo se crea y registra cuando NESS_AUDIT_ENABLED=true. El binario se
# invoca con --audit; el opt-in gate (NESS_AUDIT_ENABLED) vive en main.rs.
AUDIT_SCRIPT="$INSTALL_DIR/executables/audit_relay.sh"
AUDIT_CRON_EXPR="0 */6 * * *"

# Phase 2.16: `audit_relay.sh` SOLO se crea si el operador aceptó
# explícitamente activar el análisis de vulnerabilidades (ENABLE_AUDIT=true).
# Si respondió N al prompt, este script NO se crea:
#   - No hay tarea en crontab que intente ejecutar SSH cada 6h.
#   - No hay forma accidental de mandar hallazgos de vulns/CIS al servidor.
#   - El operador puede activarlo después reinstalando con respuesta "y".
if [[ "${ENABLE_AUDIT:-false}" == "true" ]]; then
    {
        echo "#!/bin/bash"
        echo "#"
        echo "# ═══════════════════════════════════════════════════════════"
        echo "# NESS RELAY — Script de Auditoría (Phase 2.4)"
        echo "# Ejecuta fases 9 (vulnerabilidades) + 10 (CIS) vía SSH."
        echo "# Generado automáticamente el $(date)"
        echo "# ═══════════════════════════════════════════════════════════"
        echo "#"
        echo "# USO:"
        echo "#   sudo /opt/ness_relay/executables/audit_relay.sh                  # auditoría normal"
        echo "#   sudo NESS_AUDIT_LOCAL_ONLY=true /opt/ness_relay/executables/audit_relay.sh   # SOLO local, sin enviar al servidor"
        echo "#"
        echo "# Cuando NESS_AUDIT_LOCAL_ONLY=true:"
        echo "#   • El binario NO aborta si la nube NESS HQ no responde."
        echo "#   • Los hallazgos se escriben en /opt/ness_relay/devices/<vendor>/output/"
        echo "#     con la estructura (Phase 2.16):"
        echo "#       snmp/relay_snmp_data.json"
        echo "#       vulnerabilities/relay_sentinel_vulnerabilities_data.json"
        echo "#       cis_compliance/relay_sentinel_cis_data.json"
        echo "#     quedan escritos en /opt/ness_relay/devices/<vendor>/output/."
        echo "#   • Sirve para PROBAR el flujo completo en un laboratorio sin servidor."
        echo "# ═══════════════════════════════════════════════════════════"
        echo ""
        echo "# Cargar variables de entorno públicas (NESS_AUDIT_ENABLED, etc.)"
        echo "source $ENV_FILE"
        echo ""
        # Phase 2.5.0: las credenciales SSH ya NO se cargan desde un archivo
        # en plano. El binario las descifra on-demand desde
        # /etc/ness_relay/secrets.enc (AES-256-GCM, AAD = env-var|<name>).
        # Si un operador NESS_AUDIT_LOCAL_ONLY está exportando manualmente
        # NESS_SSH_PASSWORD_* en su shell (modo legacy), se respetan como
        # fallback (v2.4.0 compat). Ver `ness-relay migrate-plaintext`
        # para migrar las pass al vault cifrado.
        echo "# (Las pass SSH ahora viven en /etc/ness_relay/secrets.enc y se descifran dentro del binario.)"
        echo ""
        echo "cd $INSTALL_DIR"
        echo ""
        # FORZAR cd primero porque si $PWD del shell padre apunta a un dir
        # eliminado (caso típico: el instalador borra /home/.../rust-sentinel al
        # final), bash internamente considera que no hay TTY y [ -t 1 ] retorna
        # FALSE → entraría al modo silent. Con cd explícito restauramos TTY.
        echo "cd $INSTALL_DIR >/dev/null 2>&1 || true"
        echo ""
        # Mostrar banner solo si stdout es TTY (modo interactivo del operador).
        # En cron (no TTY) no contaminamos el log.
        #
        # TRUCO: usamos `tty -s` en lugar de `[ -t 1 ]` porque tty -s NO depende
        # del estado del $PWD del shell padre. Es robusto contra dir eliminado.
        if tty -s 2>/dev/null; then
            echo "echo -e \"\${YELLOW}Ejecutando auditoría NESS Relay...\${NC}\""
            echo "echo -e \"\${DIM}  (Ctrl+C para abortar; los hallazgos parciales quedan en disco)\${NC}\""
            echo "./executables/$INSTALLED_BINARY_NAME --audit --config $INSTALL_DIR/configs/connection.config"
        else
            # Modo cron: --silent para no contaminar logs con banners.
            echo "./executables/$INSTALLED_BINARY_NAME --audit --silent --config $INSTALL_DIR/configs/connection.config"
        fi
        echo "EXIT_CODE=\$?"
        # Cambio Phase 2.4 — audit_local_first: si NESS_AUDIT_LOCAL_ONLY=true,
        # exit code distinto de 0 NO se considera error fatal (puede ser un fallo
        # de envío al servidor, que en modo local es esperable).
        echo "if [ \$EXIT_CODE -ne 0 ]; then"
        echo "    if [[ \"\${NESS_AUDIT_LOCAL_ONLY:-false}\" == \"true\" ]]; then"
        echo "        echo \"[\$(date '+%Y-%m-%d %H:%M:%S')] WARN: Audit terminó con código \$EXIT_CODE (modo local: probablemente servidor caído). Hallazgos locales OK.\" >> $INSTALL_DIR/logs/ness_relay.log"
        echo "        exit 0"
        echo "    else"
        echo "        echo \"[\$(date '+%Y-%m-%d %H:%M:%S')] ERROR: Audit falló con código \$EXIT_CODE\" >> $INSTALL_DIR/logs/ness_relay.log"
        echo "        exit \$EXIT_CODE"
        echo "    fi"
        echo "fi"
        echo "exit 0"
    } > "$AUDIT_SCRIPT"
    chmod +x "$AUDIT_SCRIPT"
    log_message "SUCCESS" "Script de auditoría creado: $AUDIT_SCRIPT"
else
    # ENABLE_AUDIT=false → el operador NO quiere escaneo de vulnerabilidades.
    # Importante: si existe un audit_relay.sh de una instalación PREVIA,
    # lo borramos para que (1) no se ejecute por error manual y (2) el
    # operador no se confunda pensando que está activo.
    if [[ -f "$AUDIT_SCRIPT" ]]; then
        rm -f "$AUDIT_SCRIPT"
        log_message "INFO" "audit_relay.sh eliminado (auditoría desactivada por el operador)"
    fi
    log_message "INFO" "Análisis de vulnerabilidades/CIS NO activado — audit_relay.sh no creado"
fi

###############################################################################
# CONFIGURAR CRON
###############################################################################
if [[ "$UPDATE_ONLY_MODE" == "true" ]]; then
    log_message "INFO" "Modo actualización: manteniendo configuración de cron existente"
else
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

# Definir programación: interactiva para modo normal, automática para modo silencioso/guiado.
if [[ "$SILENT_MODE" == "true" ]]; then
    if [[ "$CRON_AUTO_CONFIGURED" == "true" && -n "$FINAL_CRON" ]]; then
        CRON_EXPR="$FINAL_CRON"
        SCHEDULE_LABEL="automática desde NESS_TIME ($FINAL_CRON)"
    else
        CRON_EXPR="*/5 * * * *"
        SCHEDULE_LABEL="cada 5 minutos (default silencioso)"
    fi
else
    # Fase de pruebas: Hardcodeado a 5 minutos, sin preguntar al usuario
    CRON_EXPR="*/5 * * * *"
    SCHEDULE_LABEL="cada 5 minutos (recolección y auto-actualización)"
fi

# Eliminar entradas existentes del relay y añadir la nueva según la expresión
EXISTING_CRON="$(crontab -l 2>/dev/null | grep -v "$RUN_SCRIPT" | grep -v "$UPDATE_SCRIPT" | grep -v "$AUDIT_SCRIPT" | grep -v "ness.relay" | grep -v "ness_relay" || true)"

# Construir las nuevas entradas de cron. La línea de auditoría es
# condicional: solo se registra si NESS_AUDIT_ENABLED=true quedó en true
# tras el prompt.
CRON_NEW_ENTRIES="$CRON_EXPR $RUN_SCRIPT"$'\n'"$CRON_EXPR $UPDATE_SCRIPT"

if [[ "${ENABLE_AUDIT:-false}" == "true" ]]; then
    CRON_NEW_ENTRIES="$CRON_NEW_ENTRIES"$'\n'"$AUDIT_CRON_EXPR $AUDIT_SCRIPT"
    SCHEDULE_LABEL="$SCHEDULE_LABEL + auditoría cada 6h"
fi

if printf "%s\n%s\n" "$EXISTING_CRON" "$CRON_NEW_ENTRIES" | sed '/^[[:space:]]*$/d' | crontab -; then
    log_message "SUCCESS" "Tarea programada configurada ($SCHEDULE_LABEL)"
else
    log_message "ERROR" "No se pudo registrar la tarea en crontab"
    exit 1
fi
fi

###############################################################################
# PRUEBA OPCIONAL
###############################################################################
# PRUEBA OPCIONAL
###############################################################################
if [[ "$UPDATE_ONLY_MODE" != "true" ]]; then
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

        # Cambio Phase 2.4: en la PRIMERA ejecución post-instalación, si el
        # operador activó la auditoría (NESS_AUDIT_ENABLED=true), ejecutamos
        # el binario con --audit para validar TODO el flujo en un solo paso:
        #   - Fases 1-8: telemetría SNMP
        #   - Fase 9: vulnerabilidades (vía SSH)
        #   - Fase 10: controles CIS (vía SSH)
        #
        # En ejecuciones posteriores (cron):
        #   - Cada 5 min: SNMP-only (vía run_relay.sh, SIN --audit)
        #   - Cada 6 h:  Solo audit (vía audit_relay.sh, CON --audit)
        #
        # Esto cumple el requerimiento: "en la primera ejecución se recolecta
        # absolutamente todo".
        TEST_OUTPUT_FILE=$(mktemp /tmp/ness_relay_test_XXXXXX.log)
        source "$ENV_FILE"
        # Cargar también secrets.env si existe, para que el binario pueda
        # resolver las credenciales SSH en esta primera ejecución.
        if [[ -f /etc/ness_relay/secrets.env ]]; then
            source /etc/ness_relay/secrets.env
        fi
        cd "$INSTALL_DIR"
        if [[ "${ENABLE_AUDIT:-false}" == "true" ]]; then
            echo -e "${CYAN}${BOLD}ℹ Auditoría ACTIVADA — primera ejecución incluirá SNMP + fases 9 y 10 (SSH)${NC}"
            echo ""
            ./executables/$INSTALLED_BINARY_NAME --audit --config "$INSTALL_DIR/configs/connection.config" 2>&1 | tee "$TEST_OUTPUT_FILE"
        else
            ./executables/$INSTALLED_BINARY_NAME --config "$INSTALL_DIR/configs/connection.config" 2>&1 | tee "$TEST_OUTPUT_FILE"
        fi
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
    if [[ "$GUIDED_MODE" == "true" ]]; then
        echo -e "${WHITE}Modo guiado: ejecutando primera corrida automática del relay...${NC}"
        FIRST_RUN_OUTPUT_FILE=$(mktemp /tmp/ness_relay_first_run_XXXXXX.log)
        source "$ENV_FILE"
        cd "$INSTALL_DIR"
        ./executables/$INSTALLED_BINARY_NAME --config "$INSTALL_DIR/configs/connection.config" 2>&1 | tee "$FIRST_RUN_OUTPUT_FILE"
        FIRST_RUN_EXIT_CODE=${PIPESTATUS[0]}

        if [[ $FIRST_RUN_EXIT_CODE -eq 0 ]]; then
            INSTALL_STATUS="success"
            log_message "SUCCESS" "Primera ejecución automática completada correctamente"
        else
            INSTALL_STATUS="unknown_error"
            log_message "WARNING" "Primera ejecución automática finalizó con código $FIRST_RUN_EXIT_CODE"
            echo -e "${YELLOW}Revise el detalle en: $FIRST_RUN_OUTPUT_FILE${NC}"
        fi
    else
        INSTALL_STATUS="skipped"
        echo -e "${WHITE}Modo silencioso: omitiendo prueba interactiva.${NC}"
    fi
fi
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
echo -e "${WHITE}  • Ejecutable:${NC}               ${DIM}$INSTALL_DIR/executables/$INSTALLED_BINARY_NAME${NC}"
echo -e "${WHITE}  • Configuración:${NC}            ${DIM}$INSTALL_DIR/configs/connection.config${NC}"
echo -e "${WHITE}  • Script de ejecución:${NC}      ${DIM}$RUN_SCRIPT${NC}"
echo -e "${WHITE}  • Log de ejecución:${NC}         ${DIM}$INSTALL_DIR/logs/ness_relay.log${NC}"
echo -e "${WHITE}  • Programación:${NC}             ${GREEN}${SCHEDULE_LABEL}${NC}"
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
    echo -e "${WHITE}   El relay quedó programado como: ${GREEN}${SCHEDULE_LABEL}${NC}"
    echo -e "${WHITE}   Para ver diagnósticos en tiempo real, ejecute: ${GREEN}sudo $RUN_SCRIPT${NC}"
else
    echo -e "${YELLOW}${BOLD}⚠️  INSTALACIÓN FINALIZADA CON ADVERTENCIAS${NC}"
    echo -e "${WHITE}   Los archivos se instalaron y el cron está configurado.${NC}"
    echo -e "${WHITE}   Corrija los errores reportados y ejecute manualmente: ${GREEN}sudo $RUN_SCRIPT${NC}"
fi
echo -e "${DIM}   Gracias por usar NESS HQ Network Relay System${NC}"
echo ""

cleanup_source_package_if_needed
