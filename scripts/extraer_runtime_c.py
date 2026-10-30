#!/usr/bin/env python3
"""Extrae el runtime C embebido del backend C99 a un fichero compilable.

`silc-core/silc-backend/src/c99.rs` emite el runtime (arenas, tipos, HMAC,
capacidades, canales, red, scheduler) como cadenas Rust. Ese texto es codigo C
real que hasta ahora nadie compilaba en CI: los tests de Rust solo comparan
subcadenas. Este script lo extrae *verbatim* para que `scripts/run_c_tests.sh`
lo compile con `-Wall -Wextra -Werror` y ejecute `test_runtime_emitido.c`
contra exactamente el codigo que recibe el usuario.

Uso:
    python3 scripts/extraer_runtime_c.py <salida.h>

El script falla (exit 1) si el fuente Rust cambia de forma incompatible: es
preferible romper el build a validar un runtime que ya no es el que se emite.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
FUENTE = RAIZ / "silc-core" / "silc-backend" / "src" / "c99.rs"

# Constantes en el mismo orden en que `emitir()` / `emitir_programa()` las
# concatenan. Los pragmas de diagnostico forman parte del C emitido: silencian
# los "unused" del runtime autocontenido sin relajar el codigo de usuario.
CONSTANTES = (
    "RUNTIME_DIAG_PUSH",
    "RUNTIME_ARENA",
    "RUNTIME_TIPOS",
    "RUNTIME_CONFIANZA",
    "RUNTIME_DIAG_POP",
)

# Anclas minimas: si desaparecen, la extraccion no representa el runtime.
ANCLAS = (
    "sil_arena_crear_",
    "sil_arena_asignar_",
    "sil_arena_destruir_",
    "sil_hmac_",
    "sil_cap_solicitar_",
    "sil_cap_validar_",
    "sil_canal_crear_",
    "sil_canal_enviar_",
    "sil_sched_ejecutar_",
)

BARRA = chr(92)
ESCAPES = {
    "n": chr(10),
    "t": chr(9),
    "r": chr(13),
    "0": chr(0),
    BARRA: BARRA,
    '"': chr(34),
    "'": chr(39),
}


def decodificar_literal(texto: str, inicio: int) -> str:
    """Decodifica un literal de cadena Rust normal (con escapes)."""
    salida: list[str] = []
    i = inicio
    while i < len(texto):
        c = texto[i]
        if c == BARRA:
            nxt = texto[i + 1]
            if nxt in (chr(10), chr(13)):
                # Continuacion de linea de Rust: se descarta el salto y la
                # indentacion siguiente (equivale a "\" + newline).
                i += 2
                while i < len(texto) and texto[i] in (" ", chr(9)):
                    i += 1
                continue
            if nxt in ESCAPES:
                salida.append(ESCAPES[nxt])
                i += 2
                continue
            if nxt == "x":
                salida.append(chr(int(texto[i + 2 : i + 4], 16)))
                i += 4
                continue
            if nxt == "u" and texto[i + 2] == "{":
                cierre = texto.index("}", i)
                salida.append(chr(int(texto[i + 3 : cierre], 16)))
                i = cierre + 1
                continue
            raise SystemExit("error: escape no soportado " + BARRA + nxt + " en c99.rs")
        if c == '"':
            return "".join(salida)
        salida.append(c)
        i += 1
    raise SystemExit("error: literal Rust sin cerrar en c99.rs")


def extraer_constante(texto: str, nombre: str) -> str:
    """Devuelve el valor de `const <nombre>: &str` (raw string o literal)."""
    patron = re.compile(
        r"const\s+" + nombre + r"\s*:\s*&str\s*=\s*(r#\"|r\"|\")", re.S
    )
    m = patron.search(texto)
    if not m:
        raise SystemExit(f"error: no se encontro `const {nombre}` en {FUENTE}")
    delimitador = m.group(1)
    inicio = m.end()
    if delimitador == 'r#"':
        cierre = texto.find('"#', inicio)
        if cierre < 0:
            raise SystemExit(f"error: raw string sin cerrar en `const {nombre}`")
        return texto[inicio:cierre]
    if delimitador == 'r"':
        cierre = texto.find('"', inicio)
        if cierre < 0:
            raise SystemExit(f"error: raw string sin cerrar en `const {nombre}`")
        return texto[inicio:cierre]
    return decodificar_literal(texto, inicio)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    salida = Path(argv[1])
    fuente = FUENTE.read_text(encoding="utf-8")

    runtime = "\n".join(extraer_constante(fuente, c) for c in CONSTANTES)

    faltan = [a for a in ANCLAS if a not in runtime]
    if faltan:
        print(
            "error: el runtime extraido no contiene: " + ", ".join(faltan),
            file=sys.stderr,
        )
        return 1
    if len(runtime) < 8000:
        print(
            f"error: runtime sospechosamente corto ({len(runtime)} bytes)",
            file=sys.stderr,
        )
        return 1

    salida.parent.mkdir(parents=True, exist_ok=True)
    cabecera = (
        "/* GENERADO por scripts/extraer_runtime_c.py - NO EDITAR.\n"
        " * Runtime C embebido extraido de silc-core/silc-backend/src/c99.rs\n"
        " * (DIAG_PUSH + ARENA + TIPOS + CONFIANZA + DIAG_POP).\n"
        " * Se compila y ejecuta en scripts/run_c_tests.sh. */\n"
        "/* El programa emitido incluye stdio/stdlib/string antes del runtime;\n"
        " * aqui se incluyen para poder compilar la extraccion por separado. */\n"
        "#include <stdio.h>\n"
        "#include <stdlib.h>\n"
        "#include <string.h>\n"
    )
    salida.write_text(cabecera + runtime + "\n", encoding="utf-8")
    print(f"runtime emitido extraido: {salida} ({len(runtime)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
