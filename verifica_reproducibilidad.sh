#!/usr/bin/env bash
# verifica_reproducibilidad.sh — M9: doble build local + comparación SHA256.
# Uso: ./verifica_reproducibilidad.sh [--release]
set -euo pipefail

cd "$(dirname "$0")/silc-core"

PERFIL="debug"
BIN="target/debug/silc"
if [ "${1:-}" = "--release" ]; then
  PERFIL="release"
  BIN="target/release/silc"
fi

# Timestamp fijo desde git (o época).
if git rev-parse --git-dir >/dev/null 2>&1; then
  export SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)"
else
  export SOURCE_DATE_EPOCH="1700000000"
fi
echo "SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH"

echo "=== Build 1 ($PERFIL) ==="
cargo build --locked -p silc-cli --profile "$PERFIL" 2>&1 | tail -2
HASH1="$(sha256sum "$BIN" | cut -d' ' -f1)"
echo "SHA256(1): $HASH1"

echo "=== Limpieza + Build 2 ($PERFIL) ==="
cargo clean -p silc-cli 2>/dev/null
cargo build --locked -p silc-cli --profile "$PERFIL" 2>&1 | tail -2
HASH2="$(sha256sum "$BIN" | cut -d' ' -f1)"
echo "SHA256(2): $HASH2"

if [ "$HASH1" = "$HASH2" ]; then
  echo "REPRODUCIBLE: hashes idénticos"
  exit 0
else
  echo "NO REPRODUCIBLE: difieren"
  exit 1
fi
