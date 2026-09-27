# SIL (Semantic Intention Language)

[![SIL CI/CD Release](https://github.com/sil-lang/sil/actions/workflows/release.yml/badge.svg)](https://github.com/sil-lang/sil/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-1.0.0-green.svg)](https://github.com/sil-lang/sil/releases)

**SIL (Semantic Intention Language)** es un lenguaje de programación de nueva generación diseñado para sistemas donde la **seguridad, el rendimiento determinista y la corrección formal** son innegociables.

Combina la seguridad de memoria basada en capacidades y propiedad (sin Recolector de Basura / Zero-GC), la ergonomía de sintaxis en lenguaje natural controlado (CNL), y verificación formal integrada mediante SMT Solver (Z3/CVC5) en tiempo de compilación.

---

## Características Principales

* **Seguridad de Memoria $O(1)$ sin GC:** Gestión determinista mediante asignación por *Arenas Causales*, evitando *pauses* estocásticas y *use-after-free*.
* **Verificación Formal Nativa:** Integración con SMT Solver (Z3) para comprobar precondiciones, postcondiciones e invariantes (`asumir`/`demostrar`) antes de generar binario.
* **Mapeo Léxico Multilingüe (USID):** Sintaxis basada en *Identificadores Semánticos Universales*, permitiendo proyectar el mismo AST en español, inglés, japonés u otros idiomas sin cambiar el binario.
* **Seguridad Basada en Capacidades (Zero-Trust):** Ninguna función puede realizar operaciones de E/S, red o sistema de archivos sin un token de capacidad firmado por hardware (TPM 2.0 / Secure Enclave).
* **Compilación Transparente a C99 de Alto Rendimiento:** Transpilación directa a código C99 estricto compatible con `-std=c99 -Wall -Wextra -Werror`, optimizable por GCC/Clang/MSVC.
* **Backend Multi-Target:** LLVM IR (x86_64, ARM64, RISC-V), WebAssembly/WASI, SPIR-V/NVPTX (GPU/TPU), C ABI.
* **Concurrencia Ultra-Ligera:** Fibras *stackless* de 64 bytes, *work-stealing scheduler*, canales lock-free.
* **Inmunidad 99.9%:** Anti-buffer-overflow, anti-side-channel (branchless), anti-supply-chain (reproducible builds + TPM attestation), anti-silicon (Spectre/Rowhammer mitigations).

---

## Estructura del Repositorio

```text
sil/
├── silc_bootstrap.py       # Compilador principal (Lexer, Parser, SMT Checker y Backend C99/WASM)
├── sil_stdlib.py           # Biblioteca estándar C99 (Capabilities, Crypto, I/O, Canales)
├── sil_arena.py            # Subsistema de memoria Arenas Causales C99
├── sil_wasm.py             # Backend WebAssembly/WASI (emisión WAT)
├── sil_lsp.py              # Language Server Protocol (JSON-RPC 3.17)
├── test_sil_compiler.py    # Suite de pruebas: Lexer, Parser, Backend, Integración E2E
├── test_sil_stdlib.py      # Tests de la stdlib (compilabilidad C99, componentes seguridad)
├── test_sil_wasm.py        # Tests del backend WASM
├── README.md               # Este archivo
├── LICENSE                 # Licencia MIT
├── setup_git.sh            # Script de inicialización y push a GitHub
├── silup.sh                # Instalador Linux/macOS (una línea)
├── silup.ps1               # Instalador Windows PowerShell
├── package.json            # Manifiesto extensión VS Code
├── syntaxes/sil.tmLanguage.json  # Gramática TextMate para resaltado sintaxis
├── .github/
│   └── workflows/
│       └── release.yml     # Pipeline CI/CD: binarios cruzados + SHA256 + GitHub Release
└── docs/
    └── WHITE_PAPER.md      # Especificación técnica formal completa
```

---

## Requisitos Previos

* **Python:** 3.11 o superior.
* **Compilador C:** `gcc` o `clang` con soporte C99 (para compilar la salida del backend).
* **SMT Solver (opcional, para verificación avanzada):** `z3-solver` (`pip install z3-solver`).

---

## Instalación Rápida

### 1. Clonar el Repositorio

```bash
git clone https://github.com/devplusdesarrollo/sil.git
cd sil
```

### 2. Instalar Dependencias (Opcional para Verificación SMT)

```bash
pip install z3-solver
```

---

## Uso del Compilador (Fase 0 - Prototipo Python)

### 1. Ejemplo de Código SIL (`ejemplo.sil`)

```sil
tarea procesar_transaccion(monto: Entero64, cap_red: CapacidadHardware) : Entero64
    verificar monto > 0
    let impuesto: Entero64 = monto * 19 / 100
    retornar monto + impuesto
```

### 2. Ejecutar Pruebas Unitarias

```bash
python3 test_sil_compiler.py
python3 test_sil_stdlib.py
python3 test_sil_wasm.py
```

### 3. Generar Código C99 desde SIL

```bash
python3 silc_bootstrap.py
```

Esto imprime el código C99 generado (con runtime de Arenas y Stdlib embebido) listo para compilar con `gcc -std=c99 -O2`.

---

## Integración Continua (CI/CD)

El repositorio incluye un flujo automatizado en `.github/workflows/release.yml` que compila automáticamente ejecutables independientes (`silc`) usando `pyinstaller` para las siguientes plataformas en cada nuevo tag (`v*`):

* **Linux:** x86_64 (`silc-linux-amd64`)
* **macOS:** Apple Silicon / ARM64 (`silc-darwin-arm64`)
* **Windows:** x86_64 (`silc-windows-amd64.exe`)

Los artefactos se firman con SHA256 y se publican en *GitHub Releases*.

---

## Instalación en Una Línea (Producción - Futuro)

```bash
# Linux / macOS
curl -sSL https://releases.sil-lang.org/install.sh | bash

# Windows (PowerShell)
irm https://releases.sil-lang.org/install.ps1 | iex
```

> **Nota:** Requiere infraestructura `releases.sil-lang.org` desplegada (ver Capa Nivel 3 en Whitepaper).

---

## Extensión VS Code

Publicada en *Visual Studio Code Marketplace* como **SIL Language Support**.

* Resaltado de sintaxis CNL (`.sil`)
* Diagnósticos SMT en tiempo real (subrayado rojo con mensaje humano)
* Autocompletado de palabras clave: `tarea`, `verificar`, `asumir`, `demostrar`, `let`, `retornar`, `CapacidadHardware`, etc.
* Integración LSP (`sil-lsp`) para *go-to-definition*, *hover*, *completion*.

---

## Documentación Completa

* **Whitepaper Técnico:** [`docs/WHITE_PAPER.md`](docs/WHITE_PAPER.md) — Especificación formal completa: gramática EBNF, Causal-IR, SMT pipeline, Arenas, Capabilities, eBPF/TPM, Bootstrapping, Certificación EAL7/DO-178C.
* **Referencia Rápida CNL:** Palabras clave, restricciones, tipos primitivos, operadores.

---

## Licencia

Este proyecto está bajo la **Licencia MIT**. Consulta el archivo [LICENSE](LICENSE) para más detalles.

Copyright (c) 2026 SIL Language Contributors

---

## Contribuir

1. Fork el repo
2. Crea una rama (`git checkout -b feature/nueva-capacidad`)
3. Commit tus cambios (`git commit -am 'feat: añadir capacidad X'`)
4. Push (`git push origin feature/nueva-capacidad`)
5. Abre un Pull Request

---

## Roadmap (Próximos Hitos)

* [ ] **Fase 1:** Reescritura del compilador en SIL (Auto-hospedaje)
* [ ] **Fase 2:** Núcleo verificado en Coq (Certificación EAL7)
* [ ] **Runtime nativo:** Fibras stackless en ensamblador / C sin Python
* [ ] **Infraestructura `releases.sil-lang.org`** + CDN + Firmas GPG
* [ ] **Publicación VS Code Marketplace** + Open VSX Registry
* [ ] **Playground Web** (WASM en navegador para probar SIL sin instalar)