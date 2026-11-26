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

El flujo completo está en [Flujo de trabajo con Git](#flujo-de-trabajo-con-git).
Resumen:

```bash
git switch main && git pull --ff-only origin main
git switch -c fix/canales-atomicos        # rama corta, un solo tema
# ... cambios ...
scripts/pre-pr.sh                          # los mismos gates que la CI
git commit -m "fix(runtime): canal MPSC con publicación release/acquire"
git push -u origin fix/canales-atomicos
gh pr create --fill                        # o desde la web
```

`scripts/pre-pr.sh` ejecuta en local lo que la CI va a exigir (fmt, clippy,
tests Rust, runtime C con sanitizers, tests Python y conteos de tests al día) y
distingue **OK / FALLO / OMITIDO**: si falta una herramienta, el gate se marca
OMITIDO y el script **no** lo cuenta como éxito (`--permitir-omitidos` para
entornos sin toolchain, declarándolo en el PR).

### 4. Estándares de código

- Sigue las convenciones de Rust (`rustfmt`, `clippy`)
- Escribe documentación para todas las funciones públicas
- Escribe pruebas para toda la nueva funcionalidad
- Una capacidad solo se documenta como «✅ Listo para probar» si existe un test
  que la **ejecute** y falle al romperla (los tests que comparan subcadenas no
  cuentan como verificación de comportamiento)
- Usa comentarios en español para el código SIL, inglés para comentarios técnicos internos

## Flujo de trabajo con Git

Modelo: **trunk-based development con ramas cortas**. `main` es el tronco y
siempre debe ser publicable; cada cambio nace en una rama que vive horas o días
y entra por un Pull Request.

### Por qué no se commitea directamente en `main`

En este repositorio no es una preferencia de estilo: es lo que hace que la
verificación exista.

| Razón | Consecuencia concreta en SIL |
|---|---|
| La CI se dispara con PR hacia `main` (`.github/workflows/rust.yml`) | Si empujas solo una rama sin PR, **no corre nada**; si commiteas en `main`, la verificación llega **después** de romper el tronco |
| Los releases se cortan de tags `v*` con atestación de procedencia y SBOM | Un tag sobre un commit no verificado produce una release mala que **no se puede despublicar** (4 crates están en crates.io) |
| `main` debe permitir `git bisect` | Con commits intermedios rotos, encontrar una regresión deja de ser logarítmico |
| Reversión quirúrgica | Revertir un PR es un commit; revertir commits sueltos de `main` es cirugía sobre historial ya replicado |
| Trazabilidad EAL7 / DO-178C / ASIL-D (objetivo del proyecto) | Estos estándares exigen cadena requisito → código → test → revisión; el PR **es** ese registro |
| Trabajo en paralelo | Dos cambios sobre `arena.c` y `c99.rs` se resuelven en la rama, antes de integrar, no en `main` con prisa |

**Excepciones aceptables** (y qué hacer después):

- Errata en documentación: sigue siendo más rápido un PR de 10 segundos.
- Emergencia en producción: el mantenedor puede hacer `revert` del commit culpable
  directamente en `main` (un revert es un cambio pequeño y reversible) y abrir
  la rama para el arreglo real. La regla: *saltarse la protección deja registro y
  se limpia después*.

### Ciclo completo

```bash
# 1. Partir del tronco actualizado (nunca de una rama vieja)
git switch main
git pull --ff-only origin main

# 2. Rama corta con nombre que diga qué y por qué
git switch -c perf/arena-alineacion

# 3. Commits pequeños, uno por unidad lógica
git add silc-core/sil-rt/src/arena.c silc-core/sil-rt/tests/test_arena.c
git commit -m "perf(arena): alinear la dirección y no el tamaño"

# 4. Gates locales ANTES de empujar
scripts/pre-pr.sh

# 5. Empujar y abrir PR (incluye evidencia, no promesas)
git push -u origin perf/arena-alineacion
gh pr create --fill

# 6. Esperar la matriz completa de CI y la revisión
# 7. Integrar (squash para un PR de un solo tema) y limpiar
gh pr merge <n> --squash
git switch main && git pull --ff-only
git branch -d perf/arena-alineacion

# 8. Release: solo desde un main ya verde
git tag -a v0.2.0 -m "v0.2.0" && git push origin v0.2.0
```

### Nombres de rama y mensajes

Ramas: `<tipo>/<tema-en-kebab-case>` con los mismos tipos que los commits
(`fix/`, `feat/`, `perf/`, `docs/`, `ci/`, `refactor/`, `test/`).
Ejemplos reales del historial: `fix/llvm-imports`, `perf/arena-alineacion`.

Commits: **Conventional Commits** con los scopes del proyecto:

```
<tipo>(<scope>): <qué cambia, en imperativo>

<cuerpo: por qué, evidencia y límites de la verificación>
```

| Scope | Ámbito |
|---|---|
| `frontend` | lexer, parser, AST |
| `causal-ir` | IR, lowering, SMT |
| `backend` | c99, wasm, llvm |
| `runtime` | `sil-rt` (C) y `silc-runtime` (Rust) |
| `cli` / `lsp` | `silc-cli`, servidor LSP |
| `docs` / `ci` / `scripts` | documentación, workflows, herramientas |

Un commit debe poder revertirse solo. Si el mensaje necesita «y», probablemente
son dos commits.

### Evidencia en el PR

Un PR se aprueba por evidencia reproducible, no por confianza. `scripts/pre-pr.sh`
mapea así con la CI:

| Gate local (`scripts/pre-pr.sh`) | Job de CI |
|---|---|
| `rustfmt`, `clippy`, `cargo test` | `fmt`, `build+test` (Linux/macOS/Windows) |
| `cargo test --features smt` (`--completo`) | `smt-test` |
| `scripts/run_c_tests.sh` | `runtime-c` |
| tests Python (pytest o unittest) | `python-proto` |
| `test_counts.py --check` | `build-test` (paso «Docs al día») |
| `extraer_runtime_c.py` | `runtime-c` (contrato `c99.rs` ↔ tests) |

En el PR pega el comando y su salida. «Verificado con `scripts/run_c_tests.sh`:
40 casos, 0 fallos» sirve; «funciona en local» no. Si un gate no se pudo ejecutar,
dilo: los OMITIDOS se declaran.

### Actualizar tu rama y resolver conflictos

- `git rebase origin/main` **solo** si la rama es tuya y nadie más la tiene.
- Si alguien más la tiene (o ya está en revisión con commits citados), usa
  `git merge origin/main` dentro de la rama: no se reescribe historia publicada.
- Si hay que reescribir tras un rebase: `git push --force-with-lease`
  (nunca `--force`, que puede borrar el trabajo de otro).
- Resuelve el conflicto **en la rama**, ejecutando `scripts/pre-pr.sh` después:
  un conflicto resuelto sin re-ejecutar los tests no está resuelto.

### Cómo integrar

| Situación | Estrategia |
|---|---|
| PR de un solo tema con varios commits de trabajo | **Squash**: un commit en `main` que cuenta la historia completa |
| Serie de commits ya limpios e independientes (p. ej. migración grande) | **Rebase**: historial lineal, cada commit compila |
| Rama de release o merge de una línea de largo recorrido | **Merge commit**, consciente y justificado |

Nunca `--auto` sin supervisión en este repo: el merge debe ocurrir cuando la
matriz completa esté verde, no cuando el primero de los jobs pase.

### Protección de `main` (para administradores)

La integración de GitHub de los agentes no tiene permisos de administración; esto
lo configura un humano en *Settings → Branches*:

- Require a pull request before merging (1 aprobación; 0 para cambios de `docs/`)
- Require status checks: `fmt --check`, `build+test (...)`, `runtime-c`,
  `smt-test`, `python-proto`
- Require linear history (obliga a squash o rebase)
- Require conversation resolution
- Block force pushes y borrado de la rama
- Allow bypass solo para administradores (el caso de emergencia de arriba)

### Cuando la CI falla, y los atajos peligrosos

- **Falla un job**: se arregla en la misma rama con un commit nuevo (no se
  reescribe el PR para «tapar» el fallo).
- **Falla solo en un OS**: reproduce en ese OS o lee el log completo; los tests de
  C99 se saltan en Windows por diseño y eso debe seguir siendo explícito.
- **Falla por deriva de documentación**: `python3 scripts/test_counts.py` y
  commitea el resultado; nunca edites los conteos a mano.
- **No silencies un gate** (`#[allow]`, `#pragma`, `--no-verify`, `-D warnings`
  desactivado) para que pase: si un gate estorba, se discute en un issue aparte.
  Un `#pragma` que silencia warnings del runtime emitido fue justo lo que ocultó
  el fallo de alineación documentado en `docs/AUDITORIA.md`.

### Reglas anti-deriva

1. Una capacidad es «✅» si compila en CI, tiene un test que la **ejecuta** y ese
   test falla si se rompe.
2. Los tests que comparan subcadenas no verifican comportamiento: no cuentan como
   prueba de un backend.
3. Los conteos de tests se generan, no se escriben.
4. Todo defecto corregido deja un test que falla sin la corrección.
5. Si añades un directorio nuevo, añádelo a los filtros `paths` de la CI (un
   cambio en `scripts/` antes no disparaba nada).

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