# SIL v0.1.0 - First Release

## Toolchain SIL v0.1.0 - Complete

### 📦 Crates publicados (5/5)
| Crate | Versión | Descripción |
|-------|---------|-------------|
| silc-frontend | 0.1.0 | Parser + SMT verification |
| silc-causal-ir | 0.1.0 | Causal IR + SMT encoding |
| silc-backend | 0.1.0 | Backends C99 / WASM |
| silc-runtime | 0.1.0 | Runtime O(1) arenas + capabilities |
| silc-std | 0.1.0 | Standard library prelude |

### ✅ Componentes
- **CLI `silc`**: `silc check`, `silc build --target c99|wasm`, `silc emit-smt`
- **LSP Server** (`silc-lsp`): diagnostics, completion, hover, time-travel debugger
- **VS Code Extension**: `silc-vscode` (diagnostics, completion, hover)
- **Tests**: 7/7 integración pasando (check, build c99/wasm, emit-smt)
- **Documentación**: SPEC.md, USER_GUIDE.md, API.md, 9 ejemplos
- **CI/CD**: deny.toml, SBOM CycloneDX, release workflow

### 📦 Instalación
```bash
# Cargo (recomendado)
cargo install silc-cli

# Binarios precompilados (ver assets)
# Windows: silc-windows-x64.exe
# Linux: silc-linux-x64
# macOS: silc-macos-arm64
```

### 🧪 Quick Start
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

---

**5 crates en crates.io** | **LSP + VS Code** | **7/7 tests** | **Docs completas**