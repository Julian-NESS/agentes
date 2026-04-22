 #!/usr/bin/env bash
# ==============================================================================
# NESS Relay v2.0.0 — Script de compilación multi-distro (musl estático)
# ==============================================================================
#
# Genera un binario estático que funciona en CUALQUIER Linux (kernel ≥ 3.x)
# sin dependencias de glibc ni OpenSSL.
#
# Uso:
#   ./build_relay.sh                    # Build para x86_64 (por defecto)
#   ./build_relay.sh --arch aarch64     # Build para ARM64
#   ./build_relay.sh --release          # Release (por defecto)
#   ./build_relay.sh --debug            # Debug build
# ==============================================================================

set -euo pipefail

# ── Colores ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

# ── Parámetros por defecto ─────────────────────────────────────────────────────
ARCH="x86_64"
PROFILE="release"
BINARY_NAME="ness-relay"
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="${PROJECT_DIR}/dist"
YES_MODE=false

# ── Parseo de argumentos ───────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch)
            ARCH="$2"
            shift 2
            ;;
        --debug)
            PROFILE="debug"
            shift
            ;;
        --release)
            PROFILE="release"
            shift
            ;;
        --yes|-y)
            YES_MODE=true
            shift
            ;;
        *)
            echo "Opción desconocida: $1"
            echo ""
            echo "Uso: ./build_relay.sh [--arch x86_64|aarch64] [--release|--debug] [--yes]"
            echo ""
            echo "  --arch ARCH    Arquitectura destino (default: x86_64)"
            echo "  --release      Build optimizado para producción (default)"
            echo "  --debug        Build con símbolos de depuración"
            echo "  --yes, -y      Aceptar todas las instalaciones sin preguntar"
            exit 1
            ;;
    esac
done

# ── Función para preguntar confirmación ────────────────────────────────────────
ask_confirm() {
    local prompt="$1"
    if [[ "$YES_MODE" == "true" ]]; then
        return 0
    fi
    echo -ne "${YELLOW}${prompt} (Y/n): ${NC}"
    read -r answer
    [[ "$answer" =~ ^[Yy]$ ]] || [[ -z "$answer" ]]
}

# ── Determinar target musl ─────────────────────────────────────────────────────
case "$ARCH" in
    x86_64)
        TARGET="x86_64-unknown-linux-musl"
        MUSL_GCC="x86_64-linux-musl-gcc"
        ;;
    aarch64 | arm64)
        TARGET="aarch64-unknown-linux-musl"
        MUSL_GCC="aarch64-linux-musl-gcc"
        ARCH="aarch64"
        ;;
    *)
        echo "Arquitectura no soportada: $ARCH (soportadas: x86_64, aarch64)"
        exit 1
        ;;
esac

BINARY_PATH="${PROJECT_DIR}/target/${TARGET}/${PROFILE}/${BINARY_NAME}"
ARCH_BINARY_NAME="${BINARY_NAME}-${ARCH}"

echo "══════════════════════════════════════════════════════"
echo "  NESS Relay v2.0.0 — Build ${PROFILE} para ${ARCH}"
echo "══════════════════════════════════════════════════════"
echo "  Target   : ${TARGET}"
echo "  Proyecto : ${PROJECT_DIR}"
echo "  Salida   : ${OUTPUT_DIR}"
echo ""

# ── 1. Instalar Rust si no está disponible ─────────────────────────────────────
if ! command -v cargo &>/dev/null; then
    echo -e "[1/5] ${YELLOW}Rust no está instalado.${NC}"
    echo ""
    echo -e "  ${BOLD}¿Qué es Rust?${NC}"
    echo -e "  Rust es el lenguaje de programación con el que está escrito el agente NESS."
    echo -e "  Se necesita el compilador ${CYAN}rustc${NC} y el gestor de paquetes ${CYAN}cargo${NC} para"
    echo -e "  poder compilar el binario del agente."
    echo ""
    echo -e "  ${GREEN}Si acepta:${NC}  Se instalará Rust vía rustup (~300 MB). Puede desinstalarlo"
    echo -e "             después con: ${DIM}rustup self uninstall${NC}"
    echo -e "  ${RED}Si rechaza:${NC} No se podrá compilar el agente en este equipo."
    echo ""
    if ask_confirm "¿Desea instalar Rust?"; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        source "${HOME}/.cargo/env"
        echo -e "  ${GREEN}✓${NC} Rust instalado exitosamente."
    else
        echo -e "  ${RED}✗${NC} Compilación cancelada. Instale Rust manualmente: https://rustup.rs"
        exit 1
    fi
else
    echo "[1/5] Rust ya instalado: $(rustc --version)"
fi

# ── 2. Añadir target musl ──────────────────────────────────────────────────────
echo "[2/5] Añadiendo target ${TARGET}…"
rustup target add "${TARGET}"

# ── 3. Instalar toolchain musl ──────────────────────────────────────────────────
echo "[3/5] Verificando toolchain musl (${MUSL_GCC})…"

if ! command -v "${MUSL_GCC}" &>/dev/null; then
    echo ""
    echo -e "  ${YELLOW}La herramienta '${MUSL_GCC}' no se encontró en el sistema.${NC}"
    echo ""
    echo -e "  ${BOLD}¿Qué es esta herramienta?${NC}"
    echo -e "  Es un cross-compiler de C basado en ${CYAN}musl libc${NC}. Se requiere para que"
    echo -e "  Rust pueda generar un binario 100% estático para ${BOLD}${ARCH}${NC}, sin depender"
    echo -e "  de bibliotecas del sistema (glibc). Esto garantiza que el binario"
    echo -e "  funcione en CUALQUIER distribución Linux."
    echo ""

    if [[ "$ARCH" == "x86_64" ]]; then
        echo -e "  ${GREEN}Si acepta:${NC}  Se instalará ${CYAN}musl-tools${NC} vía apt/dnf (~5 MB)."
        echo -e "             Puede desinstalarlo después con: ${DIM}sudo apt remove musl-tools${NC}"
        echo -e "  ${RED}Si rechaza:${NC} No se podrá compilar un binario estático para ${ARCH}."
        echo ""
        if ask_confirm "¿Desea instalar musl-tools?"; then
            if command -v apt-get &>/dev/null; then
                sudo apt-get install -y musl-tools
            elif command -v dnf &>/dev/null; then
                sudo dnf install -y musl-gcc musl-libc-static
            elif command -v yum &>/dev/null; then
                echo "  musl-gcc no disponible via yum. Compilando musl desde fuente…"
                TMP_MUSL="/tmp/musl-build"
                mkdir -p "${TMP_MUSL}"
                curl -sSL "https://musl.libc.org/releases/musl-1.2.4.tar.gz" | tar -xz -C "${TMP_MUSL}" --strip-components=1
                pushd "${TMP_MUSL}"
                ./configure --prefix=/usr/local/musl --disable-shared
                make -j"$(nproc)"
                sudo make install
                sudo ln -sf /usr/local/musl/bin/musl-gcc /usr/local/bin/x86_64-linux-musl-gcc
                popd
            elif command -v zypper &>/dev/null; then
                sudo zypper install -y musl-tools
            else
                echo -e "  ${RED}✗${NC} Administrador de paquetes no reconocido."
                echo -e "  Instale manualmente: ${MUSL_GCC}"
                echo -e "  Guía: https://musl.cc/"
                exit 1
            fi
        else
            echo -e "  ${RED}✗${NC} Compilación cancelada. Sin ${MUSL_GCC} no se puede generar el binario estático."
            exit 1
        fi
    else
        # aarch64: necesita el cross-compiler musl descargado manualmente
        echo -e "  ${BOLD}Nota:${NC} Los repositorios de Ubuntu/Debian solo proveen ${DIM}aarch64-linux-gnu-gcc${NC}"
        echo -e "  (enlazado contra glibc/dinámico), pero para binarios estáticos se necesita"
        echo -e "  ${CYAN}aarch64-linux-musl-gcc${NC} (enlazado contra musl/estático)."
        echo ""
        echo -e "  ${GREEN}Si acepta:${NC}  Se descargará el toolchain musl para aarch64 (~100 MB)."
        echo -e "             Se instalará en ${DIM}/opt/musl-cross/${NC} y se crearán symlinks"
        echo -e "             en ${DIM}/usr/local/bin/${NC}."
        echo -e "             Puede desinstalarlo después con: ${DIM}sudo rm -rf /opt/musl-cross${NC}"
        echo -e "  ${RED}Si rechaza:${NC} No se podrá compilar el agente para aarch64 (ARM64/Raspberry Pi)."
        echo ""
        if ask_confirm "¿Desea descargar e instalar el toolchain musl para aarch64?"; then
            MUSL_CROSS_DIR="/opt/musl-cross/aarch64-linux-musl-cross"

            if [[ ! -d "$MUSL_CROSS_DIR" ]]; then
                sudo mkdir -p /opt/musl-cross

                # Lista de mirrors para descargar el toolchain
                MUSL_URLS=(
                    "https://musl.cc/aarch64-linux-musl-cross.tgz"
                    "https://more.musl.cc/11.2.1/x86_64-linux-musl/aarch64-linux-musl-cross.tgz"
                    "https://github.com/nickhutchinson/musl-cross/releases/latest/download/aarch64-linux-musl-cross.tgz"
                )

                DOWNLOAD_OK=false
                for url in "${MUSL_URLS[@]}"; do
                    echo -e "  Intentando descargar desde: ${DIM}${url}${NC}"
                    if curl -fSL --connect-timeout 30 --max-time 300 -o /tmp/aarch64-linux-musl-cross.tgz "$url" 2>/dev/null; then
                        echo -e "  ${GREEN}✓${NC} Descarga exitosa."
                        DOWNLOAD_OK=true
                        break
                    else
                        echo -e "  ${YELLOW}⚠${NC} No se pudo descargar desde este mirror. Probando otro…"
                    fi
                done

                if [[ "$DOWNLOAD_OK" != "true" ]]; then
                    echo ""
                    echo -e "  ${RED}✗ No se pudo descargar el toolchain desde ningún mirror.${NC}"
                    echo ""
                    echo -e "  ${BOLD}Instalación manual:${NC}"
                    echo -e "  1. Descarga el archivo desde un navegador:"
                    echo -e "     ${CYAN}https://musl.cc/aarch64-linux-musl-cross.tgz${NC}"
                    echo -e "  2. Cópialo al servidor:"
                    echo -e "     ${DIM}scp aarch64-linux-musl-cross.tgz user@server:/tmp/${NC}"
                    echo -e "  3. Extrae e instala:"
                    echo -e "     ${DIM}sudo mkdir -p /opt/musl-cross${NC}"
                    echo -e "     ${DIM}sudo tar -xzf /tmp/aarch64-linux-musl-cross.tgz -C /opt/musl-cross${NC}"
                    echo -e "     ${DIM}sudo ln -sf /opt/musl-cross/aarch64-linux-musl-cross/bin/aarch64-linux-musl-gcc /usr/local/bin/${NC}"
                    echo -e "     ${DIM}sudo ln -sf /opt/musl-cross/aarch64-linux-musl-cross/bin/aarch64-linux-musl-ar /usr/local/bin/${NC}"
                    echo -e "  4. Vuelve a ejecutar: ${DIM}./build_relay.sh --arch aarch64 --release${NC}"
                    exit 1
                fi

                echo "  Extrayendo toolchain…"
                sudo tar -xzf /tmp/aarch64-linux-musl-cross.tgz -C /opt/musl-cross
                rm -f /tmp/aarch64-linux-musl-cross.tgz
            fi

            # Crear symlinks en /usr/local/bin para que esté en el PATH
            sudo ln -sf "$MUSL_CROSS_DIR/bin/aarch64-linux-musl-gcc" /usr/local/bin/aarch64-linux-musl-gcc
            sudo ln -sf "$MUSL_CROSS_DIR/bin/aarch64-linux-musl-ar"  /usr/local/bin/aarch64-linux-musl-ar

            if ! command -v aarch64-linux-musl-gcc &>/dev/null; then
                echo -e "  ${RED}✗${NC} Error: aarch64-linux-musl-gcc no se encuentra después de la instalación."
                echo -e "  Verifique que /usr/local/bin esté en su PATH."
                exit 1
            fi
            echo -e "  ${GREEN}✓${NC} Toolchain musl para aarch64 instalado en ${DIM}$MUSL_CROSS_DIR${NC}"
        else
            echo -e "  ${RED}✗${NC} Compilación cancelada. Sin ${MUSL_GCC} no se puede generar el binario para aarch64."
            exit 1
        fi
    fi
fi

echo -e "  ${GREEN}✓${NC} Toolchain musl OK: $(${MUSL_GCC} --version | head -1)"

# ── 4. Compilar ────────────────────────────────────────────────────────────────
echo "[4/5] Compilando ${BINARY_NAME} (${PROFILE})…"
cd "${PROJECT_DIR}"

# Forzar enlace estático de libc y flags optimizados
export RUSTFLAGS="-C target-feature=+crt-static"

if [[ "$PROFILE" == "release" ]]; then
    cargo build --release --target "${TARGET}"
else
    cargo build --target "${TARGET}"
fi

# ── 5. Verificar y copiar ──────────────────────────────────────────────────────
echo "[5/5] Verificando binario…"

if [[ ! -f "${BINARY_PATH}" ]]; then
    echo "  ERROR: El binario no se generó en ${BINARY_PATH}"
    exit 1
fi

mkdir -p "${OUTPUT_DIR}"
cp "${BINARY_PATH}" "${OUTPUT_DIR}/${BINARY_NAME}"
cp "${BINARY_PATH}" "${OUTPUT_DIR}/${ARCH_BINARY_NAME}"

# Verificar que sea estático
if command -v ldd &>/dev/null; then
    LDD_OUT=$(ldd "${OUTPUT_DIR}/${ARCH_BINARY_NAME}" 2>&1 || true)
    if echo "${LDD_OUT}" | grep -q "not a dynamic executable\|statically linked\|Nicht ein dynamisch"; then
        echo "  ✓ Binario 100% estático (sin dependencias externas)"
    else
        echo "  ADVERTENCIA: El binario puede tener dependencias dinámicas:"
        echo "  ${LDD_OUT}"
    fi
fi

BINARY_SIZE=$(du -sh "${OUTPUT_DIR}/${ARCH_BINARY_NAME}" | cut -f1)
echo ""
echo "══════════════════════════════════════════════════════"
echo "  Build completado exitosamente!"
echo "  Binario : ${OUTPUT_DIR}/${ARCH_BINARY_NAME}"
echo "  Alias   : ${OUTPUT_DIR}/${BINARY_NAME}"
echo "  Tamaño  : ${BINARY_SIZE}"
echo "══════════════════════════════════════════════════════"
echo ""
echo "Para instalar (el binario ya está en dist/):"
echo "  sudo ./install_relay.sh"
echo ""
echo "Para modo silencioso:"
echo "  sudo ./install_relay.sh --silent --config-file connection.config --token TU_TOKEN --env 3"
