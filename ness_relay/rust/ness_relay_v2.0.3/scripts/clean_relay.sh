#!/usr/bin/env bash

# NESS RELAY — Script de limpieza
# Elimina únicamente artefactos creados por el instalador del agente:
# 1. /opt/ness_relay (estructura principal + backups dentro)
# 2. /etc/profile.d/ness_relay.sh (variables de entorno)
# 3. Binarios y scripts de instalación en otras ubicaciones
# 4. Finalmente, el propio script de limpieza

COLOR_GREEN="\e[32m"
COLOR_YELLOW="\e[33m"
COLOR_RED="\e[31m"
COLOR_CYAN="\e[36m"
COLOR_RESET="\e[0m"

warn() { echo -e "${COLOR_YELLOW}WARN:${COLOR_RESET} $*" >&2; }
info() { echo -e "${COLOR_GREEN}✓${COLOR_RESET} $*" ; }
header() { echo -e "${COLOR_CYAN}$*${COLOR_RESET}" ; }

require_root() {
    if [[ $(id -u) -ne 0 ]]; then
        warn "Se recomienda ejecutar este script como root (sudo)"
        exit 1
    fi
}

confirm() {
    local prompt default reply
    prompt="$1"
    default=${2:-N}
    read -r -p "$prompt [$default]: " reply || return 1
    reply=${reply:-$default}
    case "$reply" in
        y|Y|s|S) return 0 ;;
        *) return 1 ;;
    esac
}

require_tools() {
    local missing=()
    for tool in find rm readlink basename dirname; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            missing+=("$tool")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        warn "Faltan herramientas básicas: ${missing[*]}"
        exit 1
    fi
}

resolve_self_path() {
    local source_path="$0"

    if command -v realpath >/dev/null 2>&1; then
        realpath "$source_path"
        return 0
    fi

    if command -v readlink >/dev/null 2>&1; then
        readlink -f "$source_path" 2>/dev/null && return 0
    fi

    case "$source_path" in
        /*) printf '%s\n' "$source_path" ;;
        *) printf '%s/%s\n' "$PWD" "$source_path" ;;
    esac
}

MAIN_INSTALL_DIR="/opt/ness_relay"
ENV_FILE="/etc/profile.d/ness_relay.sh"
EXEC_NAMES=("ness-relay" "ness-relay-x86_64" "ness-relay-aarch64")
INSTALL_SCRIPT_NAME="install_relay.sh"
RESIDUAL_PATTERNS=("*relay*" "*ness_relay*" "*relay_rust*")

require_root
require_tools

header "\n╔════════════════════════════════════════════════════════════╗"
header "║   NESS RELAY — Limpieza segura de artefactos             ║"
header "╚════════════════════════════════════════════════════════════╝\n"

# 1) DETECTAR ARTEFACTOS
header "Fase 1: Detectando artefactos..."
echo ""

# Detectar instalación principal y backups
declare -a install_detected=()
if [[ -d "$MAIN_INSTALL_DIR" ]]; then
    install_detected+=("$MAIN_INSTALL_DIR")
fi

# Detectar backups dentro de /opt
shopt -s nullglob
for b in /opt/ness_relay_backup_*; do
    if [[ -d "$b" ]]; then
        install_detected+=("$b")
    fi
done
shopt -u nullglob

# Detectar archivo de entorno
declare -a env_detected=()
if [[ -f "$ENV_FILE" ]]; then
    env_detected+=("$ENV_FILE")
fi

# Buscar binarios e instaladores en ubicaciones seguras (no escanear /etc o /usr globalmente)
echo -e "${COLOR_YELLOW}Buscando binarios e instaladores en rutas seguras...${COLOR_RESET} (esto puede tardar)"
declare -a bins_detected=()
declare -a scripts_detected=()
declare -a residual_paths_detected=()

# Rutas donde es razonable buscar artefactos del instalador sin tocar el sistema
SEARCH_PATHS=(/opt /home /root /tmp /var/tmp "$PWD")

# Buscar binarios exactos y scripts de instalación (en SEARCH_PATHS)
declare -A _found_set=()
for p in "${SEARCH_PATHS[@]}"; do
    if [[ -d "$p" ]]; then
        while IFS= read -r -d $'\0' file; do
            # bins
            base=$(basename "$file")
            if [[ "$base" == "${EXEC_NAMES[0]}" || "$base" == "${EXEC_NAMES[1]}" || "$base" == "${EXEC_NAMES[2]}" ]]; then
                _found_set["$file"]=1
                bins_detected+=("$file")
            fi
            # install script
            if [[ "$base" == "$INSTALL_SCRIPT_NAME" ]]; then
                _found_set["$file"]=1
                scripts_detected+=("$file")
            fi
        done < <(find "$p" -xdev -type f -print0 2>/dev/null || true)
    fi
done

# Buscar rutas residuales relacionadas con el agente (coincidencias en nombre)
for p in "${SEARCH_PATHS[@]}"; do
    if [[ -d "$p" ]]; then
        while IFS= read -r -d $'\0' entry; do
            entry_base=$(basename "$entry")
            # coincidencia simple: contiene ness_relay o relay_rust o ness-relay
            if [[ "${entry_base,,}" == *ness_relay* || "${entry_base,,}" == *relay_rust* || "${entry_base,,}" == ness-relay* || "${entry_base,,}" == relay* ]]; then
                # evitar capturar cosas claramente del sistema, por ejemplo syncthing-relay en /usr/lib
                case "$entry" in
                    /usr/*|/etc/*|/lib/*|/var/lib/*)
                        # excluir excepto el archivo ENV_FILE que manejamos por separado
                        if [[ "$entry" == "$ENV_FILE" ]]; then
                            residual_paths_detected+=("$entry")
                        fi
                        ;;
                    *)
                        # añadir si no estaba ya
                        if [[ -z "${_found_set[$entry]:-}" ]]; then
                            residual_paths_detected+=("$entry")
                            _found_set["$entry"]=1
                        fi
                        ;;
                esac
            fi
        done < <(find "$p" -xdev \( -type f -o -type d \) -print0 2>/dev/null || true)
    fi
done

# Mostrar resumen
echo ""
info "Carpetas a eliminar (contienen /opt/ness_relay y backups):"
for d in "${install_detected[@]}"; do
    echo "   └─ $d"
done

if [[ ${#env_detected[@]} -gt 0 ]]; then
    info "Archivos de configuración de entorno:"
    for e in "${env_detected[@]}"; do
        echo "   └─ $e"
    done
fi

if [[ ${#bins_detected[@]} -gt 0 ]]; then
    info "Binarios encontrados:"
    for b in "${bins_detected[@]}"; do
        echo "   └─ $b"
    done
fi

if [[ ${#scripts_detected[@]} -gt 0 ]]; then
    info "Scripts de instalación encontrados:"
    for s in "${scripts_detected[@]}"; do
        echo "   └─ $s"
    done
fi

if [[ ${#residual_paths_detected[@]} -gt 0 ]]; then
    info "Rutas residuales detectadas por nombre (relay / ness_relay / relay_rust):"
    for r in "${residual_paths_detected[@]}"; do
        echo "   └─ $r"
    done
fi

# 2) CONFIRMACIÓN Y ELIMINACIÓN
echo ""
header "Fase 2: Confirmación de eliminación\n"

# Paso 1: Eliminar /opt/ness_relay y backups
if [[ ${#install_detected[@]} -gt 0 ]]; then
    if confirm "¿Eliminar carpeta de instalación y backups?" Y; then
        for d in "${install_detected[@]}"; do
            info "Eliminando: $d"
            rm -rf -- "$d" 2>/dev/null || warn "No se pudo eliminar $d"
        done
    else
        warn "Se omitió la eliminación de /opt/ness_relay"
    fi
fi

# Paso 2: Eliminar archivo de entorno
if [[ ${#env_detected[@]} -gt 0 ]]; then
    if confirm "¿Eliminar variables de entorno en $ENV_FILE?" Y; then
        for e in "${env_detected[@]}"; do
            info "Eliminando: $e"
            rm -f -- "$e" 2>/dev/null || warn "No se pudo eliminar $e"
        done
    else
        warn "Se omitió la eliminación de $ENV_FILE"
    fi
fi

# Paso 3: Eliminar binarios/scripts (uno por uno con confirmación)
if [[ ${#bins_detected[@]} -gt 0 || ${#scripts_detected[@]} -gt 0 ]]; then
    echo ""
    header "Archivos encontrados fuera de /opt/ (requieren confirmación individual):"
    echo ""
    
    for b in "${bins_detected[@]}"; do
        if [[ -e "$b" ]]; then
            if confirm "¿Eliminar binario: $b?" Y; then
                info "Eliminando: $b"
                rm -f -- "$b" 2>/dev/null || warn "No se pudo eliminar $b"
            fi
        fi
    done
    
    for s in "${scripts_detected[@]}"; do
        if [[ -e "$s" ]]; then
            if confirm "¿Eliminar script: $s?" Y; then
                info "Eliminando: $s"
                rm -f -- "$s" 2>/dev/null || warn "No se pudo eliminar $s"
            fi
        fi
    done
fi

# Paso 4: Eliminar rutas residuales que coincidan por nombre exacto
if [[ ${#residual_paths_detected[@]} -gt 0 ]]; then
    echo ""
    header "Rutas residuales detectadas (se eliminarán con confirmación individual):"
    echo ""

    for p in "${residual_paths_detected[@]}"; do
        if [[ -e "$p" ]]; then
            if [[ -d "$p" ]]; then
                if confirm "¿Eliminar directorio residual: $p?" Y; then
                    info "Eliminando directorio: $p"
                    rm -rf -- "$p" 2>/dev/null || warn "No se pudo eliminar $p"
                fi
            else
                if confirm "¿Eliminar archivo residual: $p?" Y; then
                    info "Eliminando archivo: $p"
                    rm -f -- "$p" 2>/dev/null || warn "No se pudo eliminar $p"
                fi
            fi
        fi
    done
fi

# 3) UNSETEAR VARIABLES DE ENTORNO
echo ""
header "Fase 3: Limpieza de variables de entorno en sesión actual\n"
for var in NESS_SERVER_ID NESS_API_TOKEN NESS_INSTALL_DIR NESS_DEVICES_FILE NESS_OUTPUT_DIR NESS_LOG_DIR NESS_RELAY_METADATA_URL NESS_RELAY_VENDOR NESS_RELAY_SNMP_VERSION NESS_RELAY_DEVICE_IP; do
    if [[ -n "${!var:-}" ]]; then
        unset "$var" 2>/dev/null || true
        info "Unset: $var"
    fi
done

# 4) VERIFICACIÓN FINAL
echo ""
header "Fase 4: Verificación final\n"
declare -a leftovers=()

if [[ -d "$MAIN_INSTALL_DIR" ]]; then
    leftovers+=("$MAIN_INSTALL_DIR")
fi

shopt -s nullglob
for b in /opt/ness_relay_backup_*; do
    if [[ -d "$b" ]]; then
        leftovers+=("$b")
    fi
done
shopt -u nullglob

if [[ -f "$ENV_FILE" ]]; then
    leftovers+=("$ENV_FILE")
fi

mapfile -t check_bins < <(find / -xdev -type f \( -name "${EXEC_NAMES[0]}" -o -name "${EXEC_NAMES[1]}" -o -name "${EXEC_NAMES[2]}" \) -executable 2>/dev/null || true)
mapfile -t check_scripts < <(find / -xdev -type f -name "$INSTALL_SCRIPT_NAME" 2>/dev/null || true)
mapfile -t check_residual_paths < <(find / -xdev \( -type f -o -type d \) \( -iname "${RESIDUAL_PATTERNS[0]}" -o -iname "${RESIDUAL_PATTERNS[1]}" -o -iname "${RESIDUAL_PATTERNS[2]}" \) 2>/dev/null || true)

for r in "${check_bins[@]}"; do leftovers+=("$r"); done
for r in "${check_scripts[@]}"; do leftovers+=("$r"); done
for r in "${check_residual_paths[@]}"; do leftovers+=("$r"); done

if [[ ${#leftovers[@]} -eq 0 ]]; then
    header "✓ VERIFICACIÓN EXITOSA"
    echo "El sistema está limpio. No se encontraron artefactos residuales del agente NESS RELAY."
else
    warn "Se encontraron residuos después de la limpieza:"
    for l in "${leftovers[@]}"; do
        echo "   └─ $l"
    done
    warn "Revise manualmente si desea eliminarlos."
fi

# 5) ELIMINAR EL SCRIPT DE LIMPIEZA
echo ""
SELF_PATH="$(resolve_self_path)"
if confirm "¿Eliminar este script de limpieza (${SELF_PATH})?" Y; then
    info "Eliminando el script de limpieza..."
    rm -f -- "$SELF_PATH" 2>/dev/null || warn "No se pudo eliminar $SELF_PATH"
    echo -e "${COLOR_GREEN}Script removido exitosamente.${COLOR_RESET}"
else
    warn "Se conserva el script de limpieza en: $SELF_PATH"
fi

header "\n╔════════════════════════════════════════════════════════════╗"
header "║      Proceso de limpieza finalizado                     ║"
header "╚════════════════════════════════════════════════════════════╝\n"
