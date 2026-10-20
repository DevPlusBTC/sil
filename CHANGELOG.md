# Changelog

## Unreleased

### Corregido
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
