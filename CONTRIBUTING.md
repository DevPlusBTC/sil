# Contribución a SIL

Gracias por tu interés en contribuir a SIL, el Lenguaje de Intención Semántica. Este documento describe cómo contribuir al proyecto.

## Cómo contribuir

### 1. Reportar errores

Si encuentras un error, abre un issue en GitHub con:
- Descripción clara del error
- Pasos para reproducirlo
- Código fuente que causa el error
- Comportamiento esperado vs. comportamiento real

### 2. Proponer mejoras

Para proponer nuevas características o mejoras:
- Abre un issue describiendo la propuesta
- Explica el caso de uso y por qué es necesario
- Si es posible, proporciona un ejemplo de código

### 3. Enviar cambios (Pull Requests)

1. Haz un fork del repositorio
2. Crea una rama desde `main`: `git checkout -b feature/nombre-de-la-feature`
3. Realiza tus cambios
4. Asegúrate de que todos los tests pasen: `cargo test --workspace`
5. Asegúrate de que el código pase clippy: `cargo clippy --workspace --all-targets`
6. Asegúrate de que el código esté formateado: `cargo fmt --check`
7. Haz commit de tus cambios: `git commit -m "feat: descripción del cambio"`
8. Sube tu rama: `git push origin feature/nombre-de-la-feature`
9. Abre un Pull Request

### 4. Estándares de código

- Sigue las convenciones de Rust (`rustfmt`, `clippy`)
- Escribe documentación para todas las funciones públicas
- Escribe pruebas para toda la nueva funcionalidad
- Usa comentarios en español para el código SIL, inglés para comentarios técnicos internos

## Estructura del repositorio

```
SIL/
├── silc-core/           # Compilador SIL (Rust)
│   ├── silc-frontend/   # Lexer + Parser CNL → AST
│   ├── silc-causal-ir/  # Causal-IR + SMT (Intervalos + Z3 con feature `smt`)
│   ├── silc-backend/    # Codegen C99, WASM
│   ├── silc-runtime/    # Runtime: Arenas O(1), Capacidades HMAC, TPM 2.0, Fibras, Canales
│   ├── silc-cli/        # CLI + servidor LSP (`silc lsp`): check, build, emit-smt, run
│   ├── silc-contracts/  # Contratos compartidos
│   └── silc-test/       # Tests de integración
├── silc-vscode/         # Extensión VS Code (cliente LSP + comandos)
├── sil-rt/              # Runtime C (arenas, capacidades, canales, fibras, E/S)
├── silc_bootstrap.py    # Bootstrapper del compilador (Python)
├── sil_stdlib.py        # Biblioteca estándar SIL (C99)
├── sil_wasm.py          # Backend WASM (Python)
├── sil_lsp.py           # Prototipo LSP legado en Python (el canónico es `silc lsp`)
├── README.md            # Documentación principal
├── sil.toml             # Archivo de configuración de proyecto SIL
├── CONTRIBUTING.md      # Esta guía de contribución
└── docs/                # Documentación técnica
    ├── SPEC.md          # Especificación del lenguaje
    ├── WHITE_PAPER.md   # Whitepaper técnico
    ├── API.md           # Referencia de API
    └── USER_GUIDE.md    # Guía de usuario
```

## Código de conducta

Sé respetuoso con otros contribuyentes. No se tolera acoso, lenguaje ofensivo o comportamiento discriminatorio.

## Preguntas

Si tienes preguntas, abre un issue en GitHub o contacta al mantenedor del proyecto.