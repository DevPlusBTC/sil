#!/usr/bin/env python3
"""Sincroniza los conteos de tests de la documentación con la realidad.

Fuente de verdad: `cargo test -p <crate> -- --list` por cada crate del
workspace, más los `def test_` de los tres scripts Python de la raíz.

Uso:
    python3 scripts/test_counts.py           # reescribe los documentos
    python3 scripts/test_counts.py --check   # falla (exit 1) si hay deriva

La CI ejecuta `--check`: si alguien añade tests y no regenera los docs, el job
falla indicando qué archivo está desfasado y el total esperado.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
WORKSPACE = RAIZ / "silc-core"
PY_TESTS = [
    RAIZ / "test_sil_compiler.py",
    RAIZ / "test_sil_stdlib.py",
    RAIZ / "test_sil_wasm.py",
]
C_TESTS = sorted((RAIZ / "silc-core" / "sil-rt" / "tests").glob("test_*.c"))

MARCA_INICIO = "<!-- generado-por: scripts/test_counts.py -->"
MARCA_FIN = "<!-- /generado -->"


def leer(ruta: Path) -> tuple[str, bool]:
    """Devuelve el texto con fin de línea normalizado a LF y si el archivo usa CRLF.

    Se normaliza para que los patrones con `$` funcionen igual en Windows y en
    Linux; al escribir se restaura el estilo original para no generar ruido de
    CRLF en el diff.
    """
    with ruta.open(encoding="utf-8", newline="") as f:
        crudo = f.read()
    return crudo.replace("\r\n", "\n"), "\r\n" in crudo


def escribir(ruta: Path, texto: str, crlf: bool) -> None:
    salida = texto.replace("\n", "\r\n") if crlf else texto
    with ruta.open("w", encoding="utf-8", newline="") as f:
        f.write(salida)


def contar_rust_falso() -> dict[str, int] | None:
    """Conteos inyectados (solo pruebas locales sin toolchain de Rust).

    Formato: TEST_COUNTS_FAKE_RUST="crate=n,crate=n". Documentado en el propio
    script para que nadie lo confunda con una fuente de verdad: en CI nunca se
    define y los conteos salen de `cargo test`.
    """
    crudo = os.environ.get("TEST_COUNTS_FAKE_RUST")
    if not crudo:
        return None
    conteos: dict[str, int] = {}
    for trozo in crudo.split(","):
        nombre, _, valor = trozo.partition("=")
        if not nombre or not valor.strip().isdigit():
            sys.exit(f"error: TEST_COUNTS_FAKE_RUST invalido en {trozo!r}")
        conteos[nombre.strip()] = int(valor)
    return dict(sorted(conteos.items()))


def contar_c() -> tuple[int, int]:
    """Casos y comprobaciones del runtime C (statics, sin compilar).

    Un `ST_CASO(...)` es un caso de prueba; las macros `ST_CHECK*` cuentan
    comprobaciones escritas (algunas viven en bucles, asi que el numero real
    ejecutado es mayor: `scripts/run_c_tests.sh` lo imprime).
    """
    casos = 0
    comprobaciones = 0
    for archivo in C_TESTS:
        texto = archivo.read_text(encoding="utf-8")
        casos += len(re.findall(r"^\s*ST_CASO\(", texto, re.M))
        comprobaciones += len(re.findall(r"^\s*ST_CHECK(?:_EQ|_STR)?\(", texto, re.M))
    return casos, comprobaciones


def contar_rust() -> dict[str, int]:
    """Tests por crate, contados sobre los binarios reales del workspace.

    Se compila con `cargo test --workspace --no-run --message-format=json` y se
    ejecuta `--list` contra cada ejecutable. Así el conteo refleja la
    unificación de features del workspace (p. ej. `smt`, que activa tests de
    `silc-causal-ir` sólo cuando se construye todo junto), no lo que vería un
    `cargo test -p <crate>` aislado.
    """
    falso = contar_rust_falso()
    if falso is not None:
        return falso

    compilacion = subprocess.run(
        ["cargo", "test", "--workspace", "--no-run", "--message-format=json"],
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
    )
    if compilacion.returncode != 0:
        sys.exit(
            "error: 'cargo test --workspace --no-run' falló "
            f"({compilacion.returncode}):\n{compilacion.stderr[-800:]}"
        )

    ejecutables: dict[str, list[str]] = {}
    for linea in compilacion.stdout.splitlines():
        if not linea.startswith("{"):
            continue
        try:
            msg = json.loads(linea)
        except json.JSONDecodeError:
            continue
        if msg.get("reason") != "compiler-artifact":
            continue
        if not (msg.get("profile") or {}).get("test"):
            continue
        exe = msg.get("executable")
        # Los paquetes del propio workspace no llevan nombre en `package_id`
        # (sólo `path+file:///...#0.1.0`), así que el nombre sale del directorio
        # del manifiesto: silc-core/<crate>/Cargo.toml.
        manifiesto = msg.get("manifest_path")
        if not exe or not manifiesto:
            continue
        if not Path(exe).exists():
            sys.exit(f"error: el ejecutable de test no existe: {exe}")
        crate = Path(manifiesto).parent.name
        ejecutables.setdefault(crate, []).append(exe)

    if not ejecutables:
        sys.exit("error: cargo no reportó ningún binario de test")

    conteos: dict[str, int] = {}
    for crate, binarios in ejecutables.items():
        total = 0
        for exe in binarios:
            listado = subprocess.run([exe, "--list"], capture_output=True, text=True)
            if listado.returncode != 0:
                sys.exit(
                    f"error: '{exe} --list' falló ({listado.returncode}):\n"
                    f"{listado.stderr[-500:]}"
                )
            total += sum(
                1 for l in listado.stdout.splitlines() if l.strip().endswith(": test")
            )
        conteos[crate] = total
    return dict(sorted(conteos.items()))


def contar_python() -> int:
    total = 0
    for archivo in PY_TESTS:
        if not archivo.exists():
            sys.exit(f"error: falta {archivo.name}")
        total += len(re.findall(r"^\s*def test_", archivo.read_text(encoding="utf-8"), re.M))
    return total


def reemplazar_una_vez(texto: str, patron: str, nuevo: str, archivo: Path) -> str:
    """Sustituye el patrón exigiendo exactamente una coincidencia.

    Si un documento se reestructura y el patrón desaparece, fallamos ruidosamente
    antes que dejar el conteo congelado.
    """
    regex = re.compile(patron, re.M)
    coincidencias = regex.findall(texto)
    if len(coincidencias) != 1:
        sys.exit(
            f"error: {archivo.relative_to(RAIZ)}: el patrón {patron!r} aparece "
            f"{len(coincidencias)} veces (se esperaba 1)"
        )
    return regex.sub(lambda _m: nuevo, texto)


def fila_c(etiqueta: str, casos: int, comprobaciones: int) -> str:
    return f"| **{etiqueta}** | **{casos} casos / {comprobaciones} comprobaciones ✅** |"


def actualizar_fila_c(texto: str, etiqueta: str, casos: int, comprobaciones: int,
                      archivo: Path, tras: str) -> str:
    """Inserta o actualiza la fila de tests C justo despues de la fila `tras`."""
    nueva = fila_c(etiqueta, casos, comprobaciones)
    patron_existente = re.compile(r"^\| \*\*" + re.escape(etiqueta) + r"\*\* \|.*$", re.M)
    if patron_existente.search(texto):
        return reemplazar_una_vez(texto, patron_existente.pattern, nueva, archivo)
    patron_ancla = re.compile(r"^" + re.escape(tras) + r"$", re.M)
    if len(patron_ancla.findall(texto)) != 1:
        sys.exit(f"error: {archivo.relative_to(RAIZ)}: falta el ancla {tras!r}")
    return patron_ancla.sub(lambda m: m.group(0) + "\n" + nueva, texto)


def tabla_crates(conteos: dict[str, int]) -> str:
    lineas = [MARCA_INICIO, "| Crate | Tests |", "|---|---|"]
    lineas += [f"| `{c}` | {n} |" for c, n in conteos.items()]
    lineas += [f"| **Total** | **{sum(conteos.values())}** |", MARCA_FIN]
    return "\n".join(lineas)


def main() -> int:
    parser = argparse.ArgumentParser(description="Sincroniza conteos de tests en docs.")
    parser.add_argument(
        "--check",
        action="store_true",
        help="no escribe nada; sale con 1 si los documentos están desfasados",
    )
    args = parser.parse_args()

    inyectado = contar_rust_falso() is not None
    if not inyectado and not shutil.which("cargo"):
        sys.exit("error: cargo no está en PATH (o define TEST_COUNTS_FAKE_RUST)")

    conteos = contar_rust()
    rust_total = sum(conteos.values())
    py_total = contar_python()
    c_casos, c_checks = contar_c()

    cambios: list[tuple[Path, str, str, bool]] = []

    readme = RAIZ / "README.md"
    t, readme_crlf = leer(readme)
    t2 = reemplazar_una_vez(
        t,
        r"\| \*\*Tests Rust\*\* \| \*\*\d+/\d+ ✅\*\* \|",
        f"| **Tests Rust** | **{rust_total}/{rust_total} ✅** |",
        readme,
    )
    t2 = reemplazar_una_vez(
        t2,
        r"\| \*\*Tests Python\*\* \| \*\*\d+/\d+ ✅\*\* \|",
        f"| **Tests Python** | **{py_total}/{py_total} ✅** |",
        readme,
    )
    t2 = actualizar_fila_c(
        t2, "Tests C (runtime)", c_casos, c_checks, readme,
        f"| **Tests Python** | **{py_total}/{py_total} ✅** |",
    )
    cambios.append((readme, t, t2, readme_crlf))

    paper = RAIZ / "docs/WHITE_PAPER.md"
    t, paper_crlf = leer(paper)
    t2 = reemplazar_una_vez(
        t,
        r"\| \*\*Tests Totales\*\* \|.*$",
        f"| **Tests Totales** | **{rust_total}/{rust_total} Rust + {py_total} Python + "
        f"{c_casos} C ✅** |",
        paper,
    )
    t2 = reemplazar_una_vez(
        t2,
        r"^- \d+ tests Rust \+ \d+ Python.*$",
        f"- {rust_total} tests Rust + {py_total} Python + {c_casos} casos C ({c_checks} "
        "comprobaciones) pasando; CI verde en los 3 OS (incluye fuzz y reproducibilidad)",
        paper,
    )
    cambios.append((paper, t, t2, paper_crlf))

    spec = RAIZ / "docs/SPEC.md"
    t, spec_crlf = leer(spec)
    t2 = reemplazar_una_vez(
        t,
        r"\| Tests \| Listo para probar \|.*$",
        f"| Tests | Listo para probar | {rust_total} Rust + {py_total} Python + "
        f"{c_casos} C |",
        spec,
    )
    bloque = re.compile(re.escape(MARCA_INICIO) + r".*?" + re.escape(MARCA_FIN), re.S)
    if not bloque.search(t2):
        sys.exit(
            f"error: {spec.name}: falta el bloque {MARCA_INICIO} … {MARCA_FIN} "
            "donde va la tabla por crate"
        )
    tabla = tabla_crates({**conteos, "sil-rt (C, casos)": c_casos})
    t2 = bloque.sub(lambda _m: tabla, t2)
    cambios.append((spec, t, t2, spec_crlf))

    desfasados = [ruta for ruta, antes, despues, _crlf in cambios if antes != despues]

    if args.check:
        if not desfasados:
            print(
                f"OK: docs al día ({rust_total} Rust + {py_total} Python + "
                f"{c_casos} C)"
            )
            return 0
        for ruta in desfasados:
            print(f"desfasado: {ruta.relative_to(RAIZ)}")
        print(
            f"\nEjecuta `python3 scripts/test_counts.py` y commitea el resultado "
            f"(esperado: {rust_total} Rust + {py_total} Python + {c_casos} C)."
        )
        return 1

    for ruta, _antes, despues, crlf in cambios:
        escribir(ruta, despues, crlf)

    estado = "actualizados" if desfasados else "sin cambios"
    print(f"{estado}: {rust_total} Rust + {py_total} Python + {c_casos} casos C "
          f"({c_checks} comprobaciones)")
    for crate, n in conteos.items():
        print(f"  {crate}: {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
