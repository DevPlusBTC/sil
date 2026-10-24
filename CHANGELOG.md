# Changelog

## Unreleased

### Corregido
- CI: el job `build+test` ahora pasa en las tres plataformas. Causas reales:
  - `-std=c99` estricto ocultaba `sigset_t`/`AT_FDCWD`/`struct timespec`/
    `getaddrinfo`; se define `_GNU_SOURCE` en `silc-runtime/build.rs` y
    `_POSIX_C_SOURCE 200809L` al inicio del C emitido.
  - `ebpf_linux.c` usa la API de libbpf 0.x y no compila con libbpf 1.x.
  - `io_uring`/eBPF pasan a ser opt-in (`SIL_IO_URING=1`, `SIL_EBPF=1`): el
    build por defecto ya no necesita `liburing-dev` ni `libbpf-dev`.
  - `fiber.c` declaraba el trampolín Win32 también en POSIX (`-Wunused-function`).
- Backend C99: `(Entero64)` sustituido por `(int64_t)` en `BinOpSegura` (no
  compilaba: `Entero64` no existe en C); se emiten etiquetas `blk_N` sólo si
  algún `goto` las referencia y el cuerpo del bloque se emite siempre (antes los
  bloques sin `goto` perdían instrucciones); `retornar X` en tarea `void` emite
  `(void)X; return;`.
- Backend C99: prototipos de todas las tareas y wrappers `sil_fibra_*` antes de
  las definiciones (`generar X` usaba un wrapper no declarado).
- Tests C99: nombre de archivo temporal único por test (los tests en paralelo se
  pisaban el `.c`).
- CI: `lukka/get-cmake@v3` no existe (el job `smt` nunca arrancaba); el job de
  reproducibilidad usaba `--frozen` sin cache de crates; el job de fuzz elegía
  la toolchain estable de `rust-toolchain.toml` y fallaba con
  “the option `Z` is only accepted on the nightly compiler”.
- CI de fuzz: los artefactos y los `*.log` se suben y se publican como anotación
  cuando un target falla.

### Añadido
- `scripts/test_counts.py`: cuenta los tests reales (`cargo test --workspace
  --no-run --message-format=json` + `--list` por binario, más los `def test_` de
  los scripts Python) y reescribe los conteos en README, SPEC y WHITE_PAPER.
  `--check` falla si hay deriva; la CI lo ejecuta en el job de Linux.
- Sub-arenas cíclicas en `sil-rt` (`sil_arena_ciclo_*`): pila de 16 ciclos
  anidados, reclamo O(1) en bytes y contabilidad de bytes por ciclo. Cuatro
  tests FFI nuevos en `silc-runtime` (anidamiento, reclamo/reuso de offset,
  `reset`, no-op sin ciclo abierto). Las funciones estaban declaradas en
  `sil_arena.h` y sin definir en `arena.c` desde v0.1.0.

### Documentación
- `WHITE_PAPER` recupera las secciones que decían «ver versión anterior» (EBNF
  implementado, ADTs, contratos SMT, arenas) y corrige el estado real: eBPF y
  io_uring opt-in, TPM sólo Windows, fibras POSIX, sub-arenas cíclicas sin
  implementar, 167 tests.

### Corregido (v0.1.0)
- `silc build` emite una biblioteca (sin `main`/`start`) cuando la última tarea
  tiene parámetros, en vez de abortar.
- `silc run` detecta bibliotecas y falla con un mensaje claro.
- El flag `--cc` ya no se ignora y `--smt-timeout` exige un valor >= 1.
- El lexer acepta BOM UTF-8 y las invariantes constantes sin variables ya no
  abortan el compilador.
- El target de fuzz `fuzz_roundtrip` compila y cubre todas las sentencias.
- Workflow de release: flags válidos en Rust stable, atestación de procedencia
  de GitHub, SBOM CycloneDX y smoke test del binario publicado.

### Añadido
- Servidor LSP en Rust (`silc lsp`) con diagnósticos, completion y ciclo de vida.
- Extensión de VS Code que lanza `silc lsp`.
- Backend SMT con Z3; activo por defecto en el CLI y desactivable con `--no-default-features`.

### Eliminado
- `silc-lsp` (duplicado fuera del workspace, framing inválido).
- NVM y targets sin codegen (`SPIR-V`).

## 0.1.0 - Primera versión

### Crates del workspace
| Crate | Descripción |
|-------|-------------|
| silc-frontend | Lexer, parser y AST |
| silc-causal-ir | Causal-IR y verificación SMT |
| silc-backend | Backends C99, WASM y LLVM |
| silc-runtime | Arenas O(1), capacidades, fibras y canales |
| silc-contracts | Contratos compartidos (`require`/`ensure`) |
| silc-cli | CLI `silc` y servidor LSP |
| silc-test | Tests de integración |

### Componentes
- **CLI `silc`**: `check`, `build --target c99|wasm`, `emit-smt`, `run`, `lsp`
- **LSP**: diagnósticos y completion (sin hover ni debugger todavía)
- **VS Code**: extensión con los comandos `sil.compile` y `sil.verify`
- **Tests**: workspace Rust completo + 31 tests del prototipo Python
- **Documentación**: `SPEC.md`, `WHITE_PAPER.md`, `API.md`, `USER_GUIDE.md`
- **CI**: fmt, clippy, tests, SMT, fuzz, reproducibilidad y release

### Instalación
```bash
# Desde el repositorio
cargo install --path silc-core/silc-cli

# Binarios precompilados (ver assets del release)
# Windows: silc-windows-x64.exe
# Linux: silc-linux-x64
# macOS: silc-macos-arm64
```

### Quick Start
```sil
definir tarea sumar(a: Entero64, b: Entero64) -> Entero64:
    asumir a > 0
    asumir b > 0
    demostrar a + b > 0
    retornar a + b
```

```bash
silc check hola.sil
silc build hola.sil --target c99 -o hola.c
silc build hola.sil --target wasm -o hola.wasm
silc emit-smt hola.sil
```
