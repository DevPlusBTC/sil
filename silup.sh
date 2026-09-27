#!/usr/bin/env bash
set -euo pipefail

SIL_VERSION="1.0.0"
SIL_INSTALL_DIR="${HOME}/.sil"
SIL_BIN_DIR="${SIL_INSTALL_DIR}/bin"

echo "==> Instalando el entorno de ejecución y compilador de SIL v${SIL_VERSION}..."

# Crear estructura de directorios local
mkdir -p "${SIL_BIN_DIR}"
mkdir -p "${SIL_INSTALL_DIR}/sdk"

# Detección de Arquitectura y SO
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "${ARCH}" in
    x86_64) ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    *) echo "Error: Arquitectura ${ARCH} no soportada."; exit 1 ;;
esac

TARGET="silc-${OS}-${ARCH}"
DOWNLOAD_URL="https://releases.sil-lang.org/v${SIL_VERSION}/${TARGET}.tar.gz"

echo "==> Descargando binario nativo compilado desde ${DOWNLOAD_URL}..."
curl -sSL "${DOWNLOAD_URL}" -o "/tmp/${TARGET}.tar.gz"

# Verificación de firma SHA256
curl -sSL "${DOWNLOAD_URL}.sha256" -o "/tmp/${TARGET}.tar.gz.sha256"
(cd /tmp && sha256sum -c "${TARGET}.tar.gz.sha256")

# Extracción de binarios y librería estándar (.sil)
tar -xzf "/tmp/${TARGET}.tar.gz" -C "${SIL_INSTALL_DIR}"
chmod +x "${SIL_BIN_DIR}/silc"
chmod +x "${SIL_BIN_DIR}/sil-lsp"

# Configuración del PATH en la shell
SHELL_CONFIG=""
if [ -n "${BASH_VERSION:-}" ]; then
    SHELL_CONFIG="${HOME}/.bashrc"
elif [ -n "${ZSH_VERSION:-}" ]; then
    SHELL_CONFIG="${HOME}/.zshrc"
elif [ -n "${FISH_VERSION:-}" ]; then
    SHELL_CONFIG="${HOME}/.config/fish/config.fish"
fi

if [ -n "${SHELL_CONFIG}" ] && ! grep -q "SIL_INSTALL_DIR" "${SHELL_CONFIG}"; then
    if [[ "${SHELL_CONFIG}" == *fish* ]]; then
        echo "set -gx SIL_INSTALL_DIR \"${SIL_INSTALL_DIR}\"" >> "${SHELL_CONFIG}"
        echo "set -gx PATH \"${SIL_BIN_DIR}\" \$PATH" >> "${SHELL_CONFIG}"
    else
        echo "export SIL_INSTALL_DIR=\"${SIL_INSTALL_DIR}\"" >> "${SHELL_CONFIG}"
        echo "export PATH=\"\${SIL_BIN_DIR}:\$PATH\"" >> "${SHELL_CONFIG}"
    fi
    echo "==> PATH actualizado en ${SHELL_CONFIG}."
fi

echo "==> ¡Instalación exitosa! Ejecuta 'silc --version' para comenzar."
echo "==> Reinicia tu terminal o ejecuta 'source ${SHELL_CONFIG}' para aplicar cambios."