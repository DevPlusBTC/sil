# SIL - Source Implementation Language

**Versión:** 0.1.0 | **Estado:** Beta Funcional

Lenguaje de sistemas de ultra alto rendimiento con verificación formal en tiempo de compilación. Elimina corrupción de memoria, desbordamientos de búfer, condiciones de carrera y comportamientos no definidos directamente durante la fase de compilación.

## Características

- **Invarianza Semántica Directa:** El código fuente expresa explícitamente la intención del programador y sus restricciones operativas. No existen comportamientos implícitos o no definidos.
- **Verificación SMT:** Seguridad de memoria y lógica se verifica íntegramente en tiempo de compilación mediante un motor SMT (Satisfiability Modulo Theories).
- **Cero Sobrecoste de Seguridad en Ejecución:** La verificación de seguridad se realiza en tiempo de compilación, sin impacto en runtime.
- **Gestión Determinista de Recursos O(1):** Eliminación total de recolectores de basura mediante Arenas Causales y Sub-Arenas Cíclicas.
- **Backends:** LLVM IR, C99, WASM/WASI

## Instalación

```bash
# Desde crates.io
cargo install silc-cli

# Desde fuente
git clone https://github.com/DevPlusBTC/silc.git
cd silc/silc-core
cargo build --release
```

## Uso Rápido

```bash
# Verificar un archivo SIL
silc check mi_servicio.sil

# Compilar a C99
silc build mi_servicio.sil --target c99 -o mi_servicio.c

# Compilar a WASM
silc build mi_servicio.sil --target wasm -o mi_servicio.wasm

# Emitir script SMT-LIB2
silc emit-smt mi_servicio.sil
```

## Estructura del Proyecto

```
silc-core/
├── silc-frontend/     # Lexer + Parser CNL → AST
├── silc-causal-ir/    # Causal-IR + SMT encoding
├── silc-backend/      # Codegen C99, WASM, LLVM IR
├── silc-runtime/      # Runtime: Arenas O(1), capacidades
├── silc-cli/          # Interfaz de línea de comandos
└── silc-test/         # Tests de integración

silc-vscode/           # Extensión VS Code
docs/                  # Documentación
```

## Ecosistema

| Crate | Versión | Descripción |
|-------|---------|-------------|
| `silc-frontend` | 0.1.0 | Lexer, Parser CNL, AST |
| `silc-causal-ir` | 0.1.0 | Causal-IR, SMT encoding |
| `silc-backend` | 0.1.0 | Codegen C99, WASM, LLVM |
| `silc-runtime` | 0.1.0 | Arenas O(1), capacidades |
| `silc-cli` | 0.1.0 | Interfaz de línea de comandos |

## Licencia

MIT

## Contribución

Ver [CONTRIBUTING.md](CONTRIBUTING.md)
