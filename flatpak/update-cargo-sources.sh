#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS_DIR="${TOOLS_DIR:-/tmp/flatpak-builder-tools}"
VENV_DIR="${VENV_DIR:-/tmp/fbt-venv}"

if [ ! -d "${TOOLS_DIR}" ]; then
  git clone --depth 1 https://github.com/flatpak/flatpak-builder-tools.git "${TOOLS_DIR}"
fi

if [ ! -x "${VENV_DIR}/bin/python" ]; then
  python3 -m venv "${VENV_DIR}"
  "${VENV_DIR}/bin/pip" install aiohttp tomlkit
fi

"${VENV_DIR}/bin/python" \
  "${TOOLS_DIR}/cargo/flatpak-cargo-generator.py" \
  "${ROOT_DIR}/Cargo.lock" \
  -o "${ROOT_DIR}/flatpak/cargo-sources.json"

echo "Updated: ${ROOT_DIR}/flatpak/cargo-sources.json"
