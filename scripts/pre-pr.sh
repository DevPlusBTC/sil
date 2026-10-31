#!/usr/bin/env bash
# pre-pr.sh - Ejecuta en local los mismos gates que la CI, antes de abrir un PR.
#
# Objetivo: que un PR no llegue a GitHub con fallos que ya se podian ver aqui.
# Cada gate imprime OK / FALLO / OMITIDO y el script resume al final.
#
# Uso:
#   scripts/pre-pr.sh                     # gates por defecto
#   scripts/pre-pr.sh --rapido            # omite los gates lentos (clippy, smt)
#   scripts/pre-pr.sh --completo          # anade feature smt y extension VS Code
#   scripts/pre-pr.sh --permitir-omitidos # no falla si falta una herramienta
#   scripts/pre-pr.sh --permitir-main     # no avisa por estar en la rama main
#
# Codigos de salida:
#   0  todo OK (las omisiones estan permitidas o no hubo ninguna)
#   1  algun gate FALLO
#   2  hubo gates OMITIDOS por falta de herramientas y no se permitio
#
# Regla de honestidad: si una herramienta no esta instalada, el gate se marca
# OMITIDO y el script NO lo cuenta como exito. Nunca se simulan resultados.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RAIZ" || exit 1

RAPIDO=0
COMPLETO=0
PERMITIR_OMITIDOS=0
PERMITIR_MAIN=0
for arg in "$@"; do
    case "$arg" in
        --rapido) RAPIDO=1 ;;
        --completo) COMPLETO=1 ;;
        --permitir-omitidos) PERMITIR_OMITIDOS=1 ;;
        --permitir-main) PERMITIR_MAIN=1 ;;
        -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
        *) echo "argumento desconocido: $arg" >&2; exit 2 ;;
    esac
done

fallos=0
omitidos=0
resumen=()

ok() { resumen+=("OK      $1"); }
fallo() { fallos=$((fallos + 1)); resumen+=("FALLO   $1"); }
omitido() { omitidos=$((omitidos + 1)); resumen+=("OMITIDO $1 ($2)"); }

# gate <nombre> <comando...>  |  se ejecuta mostrando la salida solo al fallar
gate() {
    local nombre="$1"
    shift
    local log
    log="$(mktemp)"
    printf '\n== %s\n   $ %s\n' "$nombre" "$*"
    if "$@" >"$log" 2>&1; then
        ok "$nombre"
    else
        fallo "$nombre"
        echo "   ---- ultimas lineas de la salida ----"
        tail -n 25 "$log" | sed 's/^/   /'
    fi
    rm -f "$log"
}

echo "================ pre-PR de SIL ================"

# --- 0. Contexto de git (avisa, no bloquea salvo en main) -------------------
rama="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')"
echo "rama: $rama"
if [ "$rama" = "main" ]; then
    if [ "$PERMITIR_MAIN" -eq 1 ]; then
        ok "rama (main, permitido con --permitir-main)"
    else
        fallo "rama: estas en main; el flujo del repo es rama -> PR -> main"
    fi
else
    ok "rama ($rama)"
fi
if git diff --quiet && git diff --cached --quiet; then
    ok "arbol de trabajo limpio"
else
    echo "   aviso: hay cambios sin commitear (el PR solo incluye lo commiteado)"
    ok "arbol de trabajo (con cambios locales)"
fi
if git rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1; then
    lee="$(git rev-list --left-right --count '@{u}...HEAD' | awk '{print $1, $2}')"
    echo "   vs upstream (detras adelante): $lee"
fi

# --- 1. Herramientas disponibles -------------------------------------------
tiene_cargo=0
command -v cargo >/dev/null 2>&1 && tiene_cargo=1

# --- 2. Gates de Rust -------------------------------------------------------
if [ "$tiene_cargo" -eq 1 ]; then
    gate "rustfmt" bash -c 'cd silc-core && cargo fmt --all --check'
    if [ "$RAPIDO" -eq 0 ]; then
        gate "clippy (-D warnings)" bash -c \
            'cd silc-core && cargo clippy --workspace --all-targets --locked -- -D warnings'
    else
        omitido "clippy" "--rapido"
    fi
    gate "cargo test --workspace" bash -c 'cd silc-core && cargo test --workspace --locked'
    if [ "$COMPLETO" -eq 1 ]; then
        gate "cargo test --features smt (Z3)" bash -c \
            'cd silc-core && Z3_SYS_Z3_VERSION=4.13.0 cargo test --workspace --locked --features smt'
    fi
else
    omitido "rustfmt" "cargo no esta en PATH"
    omitido "clippy" "cargo no esta en PATH"
    omitido "cargo test" "cargo no esta en PATH"
fi

# --- 3. Runtime C (estricto + sanitizers + runtime emitido) -----------------
if command -v cc >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1; then
    if [ "$RAPIDO" -eq 1 ]; then
        gate "runtime C (estricto + ASan/UBSan)" scripts/run_c_tests.sh --rapido
    else
        gate "runtime C (estricto + ASan/UBSan/TSan)" scripts/run_c_tests.sh
    fi
else
    omitido "runtime C" "no hay compilador C en PATH"
fi

# --- 4. Prototipo Python ----------------------------------------------------
if command -v python3 >/dev/null 2>&1; then
    if python3 -c 'import pytest' >/dev/null 2>&1; then
        gate "tests Python (pytest)" python3 -m pytest -q
    else
        # Sin pytest se ejecutan las tres suites directamente (unittest).
        gate "tests Python (unittest)" bash -c \
            'set -e; python3 test_sil_compiler.py >/dev/null && \
             python3 test_sil_stdlib.py >/dev/null && python3 test_sil_wasm.py >/dev/null'
    fi
else
    omitido "tests Python" "python3 no esta en PATH"
fi

# --- 5. Documentacion: conteos de tests sincronizados ----------------------
if [ "$tiene_cargo" -eq 1 ]; then
    gate "docs: conteos de tests al dia" python3 scripts/test_counts.py --check
else
    omitido "docs: conteos de tests" "necesita cargo para contar los tests Rust"
fi

# --- 6. Extraccion del runtime emitido (contrato c99.rs <-> tests) ---------
if command -v python3 >/dev/null 2>&1; then
    tmp="$(mktemp -d)"
    gate "runtime emitido extraible" \
        python3 scripts/extraer_runtime_c.py "$tmp/runtime_emitido.h"
    rm -rf "$tmp"
fi

# --- 7. Extension VS Code (solo --completo) --------------------------------
if [ "$COMPLETO" -eq 1 ]; then
    if command -v npm >/dev/null 2>&1 && [ -d silc-vscode/node_modules ]; then
        gate "extension VS Code (tsc)" bash -c 'cd silc-vscode && npm run compile'
    else
        omitido "extension VS Code" "falta npm o silc-vscode/node_modules"
    fi
fi

# --- Resumen ---------------------------------------------------------------
echo
echo "================= resumen ================="
printf '%s\n' "${resumen[@]}"
echo
if [ "$fallos" -gt 0 ]; then
    echo "$fallos gate(s) FALLARON. No abras el PR todavia."
    exit 1
fi
if [ "$omitidos" -gt 0 ] && [ "$PERMITIR_OMITIDOS" -eq 0 ]; then
    echo "$omitidos gate(s) OMITIDOS por falta de herramientas."
    echo "Instalalas o repite con --permitir-omitidos (dejando constancia en el PR)."
    exit 2
fi
if [ "$omitidos" -gt 0 ]; then
    echo "Todo lo ejecutado paso. $omitidos gate(s) omitidos (declaralo en el PR)."
    exit 0
fi
echo "Todos los gates pasaron. Adelante con el PR."
exit 0
