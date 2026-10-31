#!/usr/bin/env bash
# run_c_tests.sh - Verificacion del runtime C de SIL (sil-rt) y del runtime
# C que emite el backend C99.
#
# Fases:
#   1. Puerta estricta: cada fuente de sil-rt compila con -std=c99 -pedantic-errors
#      -Wall -Wextra -Werror (las mismas advertencias que usa `cc` en el build de
#      Rust, pero sin silenciarlas).
#   2. Tests unitarios con AddressSanitizer + UndefinedBehaviorSanitizer.
#   3. Tests concurrentes con ThreadSanitizer.
#   4. Extraccion verbatim del runtime emitido por silc-backend y sus tests.
#
# Uso:
#   scripts/run_c_tests.sh              # todo
#   CC=clang scripts/run_c_tests.sh     # otro compilador
#   scripts/run_c_tests.sh --rapido     # sin TSan (mas rapido en local)
#   scripts/run_c_tests.sh --bench      # ademas imprime las medidas de bench
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RT="$RAIZ/silc-core/sil-rt"
TESTS="$RT/tests"
BUILD="$RAIZ/build/c-tests"
CC="${CC:-cc}"
RAPIDO=0
BENCH=0

for arg in "$@"; do
    case "$arg" in
        --rapido) RAPIDO=1 ;;
        --bench) BENCH=1 ;;
        *) echo "argumento desconocido: $arg" >&2; exit 2 ;;
    esac
done

mkdir -p "$BUILD"
echo "== compilador: $("$CC" --version | head -1)"

FLAGS_ESTRICTOS=(-std=c99 -pedantic-errors -Wall -Wextra -Werror -D_GNU_SOURCE -I"$RT/include")
FLAGS_ASAN=(-g -O1 -fno-omit-frame-pointer -fsanitize=address,undefined -fno-sanitize-recover=undefined)
FLAGS_TSAN=(-g -O1 -fno-omit-frame-pointer -fsanitize=thread)

fallos=0
resumen=()

ejecutar() { # nombre binario
    local nombre="$1" bin="$2"
    echo "-- $nombre"
    if "$bin"; then
        resumen+=("OK   $nombre")
    else
        fallos=$((fallos + 1))
        resumen+=("FALLO $nombre")
    fi
}

echo "== fase 1: compilacion estricta de sil-rt"
for src in arena cap channel fiber sil_hmac aio ebpf; do
    if "$CC" "${FLAGS_ESTRICTOS[@]}" -c "$RT/src/$src.c" -o "$BUILD/$src.o"; then
        resumen+=("OK   estricto $src.c")
    else
        fallos=$((fallos + 1))
        resumen+=("FALLO estricto $src.c")
    fi
done

echo "== fase 2: tests con ASan + UBSan"
compilar_test() { # fuente binario [extras...]
    local fuente="$1" bin="$2"
    shift 2
    "$CC" -std=c99 -Wall -Wextra -Werror -D_GNU_SOURCE -I"$RT/include" -I"$TESTS" \
        "${FLAGS_ASAN[@]}" "$@" "$TESTS/$fuente" -o "$BUILD/$bin"
}

compilar_test test_arena.c test_arena "$RT/src/arena.c"
ejecutar "arena (ASan/UBSan)" "$BUILD/test_arena"

compilar_test test_crypto_cap.c test_crypto_cap "$RT/src/sil_hmac.c" "$RT/src/cap.c"
ejecutar "sha256/hmac/capacidades (ASan/UBSan)" "$BUILD/test_crypto_cap"

compilar_test test_channel.c test_channel "$RT/src/channel.c" "$RT/src/arena.c" -pthread
ejecutar "canales SPSC/MPSC (ASan/UBSan)" "$BUILD/test_channel"

compilar_test test_fiber.c test_fiber "$RT/src/fiber.c"
ejecutar "fibras y scheduler (ASan/UBSan)" "$BUILD/test_fiber"

compilar_test test_aio.c test_aio "$RT/src/aio.c"
ejecutar "E/S asincrona POSIX (ASan/UBSan)" "$BUILD/test_aio"

echo "== fase 3: ThreadSanitizer (canales concurrentes)"
if [ "$RAPIDO" -eq 0 ]; then
    "$CC" -std=c99 -Wall -Wextra -Werror -D_GNU_SOURCE -I"$RT/include" -I"$TESTS" \
        "${FLAGS_TSAN[@]}" "$TESTS/test_channel.c" "$RT/src/channel.c" "$RT/src/arena.c" \
        -pthread -o "$BUILD/test_channel_tsan"
    ejecutar "canales SPSC/MPSC (TSan)" "$BUILD/test_channel_tsan"
else
    resumen+=("SKIP canales (TSan) --rapido")
fi

echo "== fase 4: runtime emitido por silc-backend (extraccion verbatim)"
python3 "$RAIZ/scripts/extraer_runtime_c.py" "$BUILD/runtime_emitido.h"
"$CC" -std=c99 -Wall -Wextra -Werror -D_GNU_SOURCE -I"$BUILD" -I"$TESTS" \
    "${FLAGS_ASAN[@]}" "$TESTS/test_runtime_emitido.c" -o "$BUILD/test_runtime_emitido"
ejecutar "runtime emitido C99 (ASan/UBSan)" "$BUILD/test_runtime_emitido"

if [ "$BENCH" -eq 1 ]; then
    echo "== fase 5: benchmark de canales y arena (sin sanitizers)"
    "$CC" -std=c99 -Wall -Wextra -Werror -D_GNU_SOURCE -O2 -I"$RT/include" \
        "$TESTS/bench_canales.c" "$RT/src/channel.c" "$RT/src/arena.c" -pthread \
        -o "$BUILD/bench_canales"
    "$BUILD/bench_canales"
fi

echo
echo "===== resumen ====="
printf '%s\n' "${resumen[@]}"
if [ "$fallos" -ne 0 ]; then
    echo "FALLARON $fallos comprobaciones"
    exit 1
fi
echo "todo correcto"
