# Changelog

## Unreleased

### Corregido — runtime C (auditoría 2026-10-30, ver `docs/AUDITORIA.md`)
- **Canales**: `sil_channel_enviar` hacía SEGV en el primer envío. El macro
  `ATOMIC_CAS` pasaba *el valor* del puntero como dirección del valor esperado
  (`(void **)(old)` en vez de `&old`), así que `atomic_compare_exchange`
  desreferenciaba `NULL`. Reescritos SPSC/MPSC como anillo acotado con número de
  secuencia por ranura (Vyukov): memoria O(capacidad), cero reservas por
  mensaje, sin ABA, contrapresión real (`enviar` falla con el canal lleno) y
  cierre que permite drenar. `capacidad` deja de ignorarse (0 = 1024, se
  redondea a potencia de dos, tope 2^24).
- **Fibras**: cada `yield` reiniciaba la fibra (el scheduler reanudaba el
  contexto de `makecontext`, no el guardado por `swapcontext`), lo que producía
  un bucle infinito; además el scheduler siempre elegía la primera fibra lista
  (sin round-robin). Ahora el contexto propio es la única fuente de verdad y el
  reparto usa un cursor circular. TLS portable (`__thread`/`__declspec(thread)`)
  en lugar de `_Thread_local`, que no es C99 y `build.rs` compila con `-std=c99`.
- **Arenas**: se alineaba el tamaño en vez de la dirección (un `asignar(16, 16)`
  tras un `asignar(1, 8)` devolvía memoria desalineada); una petición cercana a
  `SIZE_MAX` desbordaba `usado + alineado` y devolvía un puntero dentro del
  bloque; tras `destruir` la siguiente asignación desreferenciaba `NULL`;
  `promover(a, a, …)` auto-enlazaba la lista (ciclo en `destruir`). Corregido con
  alineación sobre la dirección absoluta, guardas de desbordamiento, creación
  perezosa de bloque y guardas de `promover`.
- **Runtime C emitido (`silc-backend` `RUNTIME_ARENA`)**: `sil_arena_asignar_(a,
  n, 0)` calculaba `a = 0` y devolvía siempre el mismo puntero **sin reservar
  memoria**; tampoco alineaba ni comprobaba desbordamientos. Reescrito con el
  mismo contrato que el runtime nativo.
- **SHA-256/HMAC**: `sil_sha256(NULL, len>0, out)` retornaba sin escribir `out`
  (salida indefinida). Ahora emite digesto nulo determinista; `sha256_update`
  ignora mensajes nulos y `sil_hmac_sha256` rechaza clave nula con longitud.
- **Capacidades**: se aceptaba `permisos = 0` al solicitar y `requerido = 0` al
  validar (capacidad «vacía» que valida); la comprobación de bits desconocidos
  era redundante; la clave fijada se publicaba sin orden de memoria. Corregido.
- **Rust** (`silc-runtime`): `Channel::enviar` y `ChannelMPSC::enviar` filtraban
  la caja (`Box::into_raw` sin recuperación) cuando el envío fallaba por
  contrapresión o cierre. Ahora se recupera la propiedad y se libera.

### Añadido — flujo de trabajo Git
- `CONTRIBUTING.md`: sección **«Flujo de trabajo con Git»** (trunk-based con
  ramas cortas): por qué no se commitea en `main` con el caso concreto de este
  repo (la CI solo se dispara con PR, los releases salen de tags con atestación,
  trazabilidad EAL7/DO-178C), excepciones aceptables, ciclo completo, convención
  de ramas y de mensajes por scope, tabla gates locales ↔ jobs de CI, rebase vs
  merge, squash/rebase/merge al integrar, protección de `main` recomendada y
  reglas anti-deriva.
- `scripts/pre-pr.sh`: ejecuta en local los gates de la CI con resultado
  **OK / FALLO / OMITIDO** (nunca cuenta una herramienta ausente como éxito).
  Salidas: 0 todo correcto, 1 algún fallo, 2 omisiones no permitidas. Flags:
  `--rapido`, `--completo` (feature `smt` + extensión VS Code),
  `--permitir-omitidos`, `--permitir-main`. Probado en los tres caminos.
- `.github/pull_request_template.md`: el PR pide evidencia (comando + salida),
  gates omitidos declarados, alcance y límites, y riesgos de compatibilidad.
- CI: el job `python-proto` se llamaba «python prototipo (31 tests)»; el conteo
  fijo se desfasaba con cada test nuevo. Ahora se llama «python prototipo
  (unittest)» y el número real lo mantiene `scripts/test_counts.py`.

### Añadido — verificación del runtime C
- `silc-core/sil-rt/tests/`: 40 casos y 222 comprobaciones escritas (≈4.700
  ejecutadas por los bucles) para arenas, canales (incluido SPSC de 200.000
  mensajes y MPSC 4P/1C con detección de duplicados y pérdidas), fibras,
  SHA-256/HMAC (vectores FIPS 180-4 y RFC 4231) y capacidades.
- `scripts/run_c_tests.sh`: compilación estricta de `sil-rt`
  (`-std=c99 -pedantic-errors -Wall -Wextra -Werror`), ejecución con
  AddressSanitizer/UBSan, pasada de ThreadSanitizer y `--bench` con medidas
  reproducibles (arena 3,5 ns/op; anillo 3,1 ns por par enviar+recibir).
- `scripts/extraer_runtime_c.py`: extrae *verbatim* el runtime C embebido en
  `silc-backend/src/c99.rs` (soporta raw strings y literales con escapes y
  continuaciones de línea) para compilarlo y ejecutarlo. Si el fuente Rust
  cambia de forma incompatible, la extracción falla.
- CI: job `runtime-c` (ubuntu-latest) y `scripts/**` añadido a los filtros de
  `paths`, que antes no disparaban CI al cambiar herramientas.
- `scripts/test_counts.py`: cuenta también los casos C y sincroniza la nueva
  fila/filas en README, SPEC y WHITE_PAPER; modo `TEST_COUNTS_FAKE_RUST` para
  trabajar sin toolchain de Rust (documentado y nunca usado en CI).
- `docs/AUDITORIA.md`: auditoría completa con hallazgos (C-01…C-07 corregidos,
  R-01…R-11 abiertos con prioridad y plan), medidas de rendimiento, decisiones
  de diseño y definición de «verificado».

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
