# Changelog

## Unreleased

### Corregido
- Backend C99: helpers `sil_le64`/`sil_le32` renombrados sin sufijo `_` (coinciden con sus llamadas en el runtime); etiquetas `blk_N` solo se emiten si son target de salto o bloque de entrada (evita `-Wunused-label`).
- Runtime Linux: `_GNU_SOURCE` definido antes de includes en `sil-rt/src/aio_linux.c` (expone `sigset_t`/`AT_FDCWD` para `liburing.h`).
- CI: `dtolnay/rust-toolchain@1.97.1` pinned en workflow (evita fallos transitorios de instalación de toolchain).

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
