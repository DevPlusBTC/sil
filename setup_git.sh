#!/usr/bin/env bash
set -e

echo "=== Inicializando Repositorio Git para SIL ==="

# 1. Inicializar git (si no existe)
if [ ! -d .git ]; then
    git init
fi

# 2. Configurar rama principal
git branch -M main

# 3. Agregar todos los archivos
git add silc_bootstrap.py test_sil_compiler.py sil_stdlib.py test_sil_stdlib.py \
         sil_arena.py sil_wasm.py test_sil_wasm.py sil_lsp.py \
         README.md LICENSE setup_git.sh sil.toml \
         silup.sh silup.ps1 package.json syntaxes/sil.tmLanguage.json \
         .github/workflows/release.yml docs/WHITE_PAPER.md

# 4. Crear el commit inicial
git commit -m "feat: release inicial del compilador y suite de pruebas de SIL v1.0.0

- Compilador Fase 0 (Python): Lexer, Parser, Causal-IR, SMT Pipeline, Backend C99/WASM
- Stdlib C99: Capabilities Zero-Trust, Crypto constant-time, I/O gated, Canales lock-free
- Arenas Causales: Bump allocation O(1), Sub-arenas cíclicas, Hoisting, NVM persistentes
- LSP Server: JSON-RPC 3.17, diagnósticos SMT incremental, autocompletado CNL
- Tests: Lexer, Parser, Backend C99, Stdlib, WASM, Integración E2E
- CI/CD: GitHub Actions matrix Linux/macOS/Windows, pyinstaller, SHA256, GH Release
- Distribución: silup.sh/ps1, VS Code extension (TextMate grammar), package.json
- Docs: Whitepaper formal, README, MIT License
"

# 5. Crear tag de release
git tag -a v1.0.0 -m "Versión oficial v1.0.0 - Especificación completa y prototipo funcional"

echo ""
echo "=== Repositorio preparado con éxito ==="
echo "Para subir a GitHub ejecuta:"
echo "  git remote add origin https://github.com/devplusdesarrollo/sil.git"
echo "  git push -u origin main --tags"