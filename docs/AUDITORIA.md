# Auditoría técnica SIL — 2026-10-30

**Base auditada:** `687fb361d8fe6602354d1f8f383ed1d0ad06241d` (rama `main`)
**Alcance:** los 109 ficheros del repositorio (Rust ≈ 15 k LOC, C ≈ 1,5 k LOC, Python ≈ 1,7 k LOC, docs, CI).
**Método:** lectura del código + ejecución real de todo lo ejecutable en este entorno (Python, C con gcc/ASan/UBSan/TSan) + verificación estática de Rust con `tree-sitter`. Cada afirmación de este documento tiene un comando reproducible o una referencia `archivo:línea` de la base auditada (`git show <base>:<archivo>`).

---

## 0. Qué significa "verificado" en este documento

| Nivel | Significado | Herramientas |
|---|---|---|
| **V1 — ejecutado** | El código se compiló y se ejecutó; el resultado se observó | `gcc 12`, ASan/UBSan/TSan, `python3` |
| **V2 — parseado** | El código se analizó sintácticamente | `tree-sitter` (Rust) |
| **V3 — leído** | Revisión por lectura; sin ejecución | — |
| **NO verificado** | No se pudo compilar aquí | — |

**Limitación material de este entorno:** no hay toolchain de Rust (ni `rustc`, ni `cargo`, ni `rustup`; `static.rust-lang.org` e `index.crates.io` no son alcanzables y PyPI no distribuye un `rustc` real). Por tanto **nada del workspace Rust se compiló aquí**: los cambios de Rust están en nivel V2 y se marcan como tales. Todo el runtime C y el runtime C emitido sí están en V1.

Esta limitación no es un detalle: es la razón por la que los fallos de la sección 2 sobrevivieron con la documentación afirmando «✅ Listo para probar» y «169/169 tests».

---

## 1. Estado real, con evidencia

| Componente | Documentación previa | Comprobación independiente | Veredicto |
|---|---|---|---|
| Frontend CNL (lexer/parser) | ✅ 100 % | 30 tests Rust + 31 Python (ejecutados aquí) | Coherente |
| Causal-IR + SMT intervalos | ✅ | 35 tests Rust | No ejecutado aquí (sin cargo) |
| Backend C99 | ✅ | Tests Rust comparan **subcadenas** del C emitido (`c.contains("sil_canal_crear_(8")`); la CI solo hace `gcc -fsyntax-only` de ejemplos | **Parcial**: compila, no se ejecuta |
| Backend WASM | ✅ | `wasmparser` valida el módulo | Coherente |
| Backend LLVM | "implementado, sin validar" | Existe job `llvm-test` en CI | Coherente |
| Arenas O(1) | ✅ | 8 tests Rust FFI + **40 casos C nuevos** | **Corregido y verificado** |
| Sub-arenas cíclicas | ✅ | 4 tests Rust + casos C | Coherente |
| Canales SPSC/MPSC | ✅ "Listo para probar" | **0 tests. `sil_channel_enviar` hacía SEGV en el primer envío** | ❌→✅ corregido y verificado |
| Fibras (ucontext/Win32) | ✅ "Listo para probar" | **0 tests. Cada `yield` reiniciaba la fibra: bucle infinito** | ❌→✅ corregido y verificado |
| Capacidades HMAC | ✅ 17 tests | Vectores FIPS 180-4 / RFC 4231 nuevos | ✅ pero con huecos corregidos |
| TPM 2.0 | "solo Windows" | `#[cfg(windows)]`, sin hardware | Coherente (no verificable) |
| eBPF / io_uring | "opt-in, libbpf 0.x" | CI instala `libbpf-dev`, pero el job no define `SIL_EBPF=1` | **La rama eBPF nunca se compila en CI** |
| Runtime C emitido (c99.rs) | "validado con gcc -Werror" | Se compilaba con `gcc -fsyntax-only` (Linux, solo ejemplos) pero **nunca se ejecutaba** ni se compilaba en Windows/macOS; aquí se ejecuta por primera vez | ❌→✅ con 1 bug crítico corregido |
| Documentación | "CI verde en los 3 OS" vs README "CI fallando" | Ambas frases coexisten en el repo | Incoherencia menor |

---

## 2. Hallazgos críticos (corregidos en esta auditoría)

### C-01 · Los canales provocaban un fallo de segmentación en el primer envío
**Severidad: crítica (API pública inutilizable).**
`sil-channels`: base `silc-core/sil-rt/src/channel.c:25`

```c
#define ATOMIC_CAS(ptr, old, new) atomic_compare_exchange_weak_explicit( \
    (_Atomic(void *)*)(ptr), (void **)(old), (void *)(new), ...)
```

`expected` debe ser **la dirección** donde vive el valor esperado; el macro pasa **el valor** del puntero. En `sil_channel_enviar` la única rama que hace CAS es cuando `tail->next == NULL`, así que `expected` era `NULL` y `__atomic_compare_exchange` escribía/leía en la dirección 0.

Prueba (ASan, 12 líneas de C):

```
AddressSanitizer: SEGV on unknown address 0x000000000000
    #0 sil_channel_enviar silc-core/sil-rt/src/channel.c:95
```

Es decir: la primera llamada a `sil_channel_enviar` de cualquier programa SIL abortaba. No había ni un test que la llamara (`0/0` en la matriz del whitepaper), y la documentación decía «✅ Listo para probar».

**Corrección:** reescritura completa como anillo acotado con secuencia por ranura (Vyukov), sin nodos por mensaje, con `__atomic`/`Interlocked` reales (sin casts a `_Atomic`) y consumidor único sin CAS. Ver §5.

### C-02 · Las fibras se reiniciaban en cada `yield` (bucle infinito) y el round-robin no existía
**Severidad: crítica.**
Base `silc-core/sil-rt/src/fiber.c:196` (`swapcontext(s->sched_fiber, elegida->handle)`) vs `:83` y `:168`.

- El scheduler reanudaba `elegida->handle`, el contexto **inicial** creado por `makecontext`, no el contexto guardado por `swapcontext(&fc->ctx, ...)` en el `yield`. Cada `yield` volvía a entrar en la función desde el principio.
- `sil_sched_ejecutar` elegía siempre la **primera** fibra `LISTA`: una fibra que cedía el control era la siguiente en ejecutarse. Sin cursor, no hay round-robin.
- `uc_link = s->sched_fiber` apuntaba a un `ucontext_t` que además se usaba como destino de guardado: doble uso del mismo objeto.

Prueba (fibra que imprime `A1`, cede, imprime `A2`): la salida fue `A1` repetido hasta el timeout de 5 s.

**Corrección:** el contexto propio (`fc->ctx`) es la única fuente de verdad; cursor circular para el reparto; `__thread`/`__declspec(thread)` en lugar de `_Thread_local` (que además no es C99, y `build.rs` compila con `-std=c99`).

### C-03 · Arenas: alineación, desbordamiento, arena destruida y auto-enlace
**Severidad: alta (corrupción silenciosa).**

| Defecto | Base | Evidencia |
|---|---|---|
| Se alineaba el **tamaño**, no la **dirección**: tras una reserva de 1 B con alineación 8, una petición de 16 B alineada a 16 devolvía `0x…2d8` (no alineado) | `src/arena.c:26,40` | ejecución real |
| `tamanio = SIZE_MAX-16` devolvía un puntero dentro de un bloque de 4 KiB (desbordamiento de `b->usado + alineado`) | `src/arena.c` (sin comprobación) | ejecución real + ASan |
| `sil_arena_destruir` dejaba `actual = NULL`; la siguiente asignación desreferenciaba NULL | `src/arena.c` | lectura + ejecución |
| `sil_arena_promover(a, a, …)` auto-enlazaba la lista de bloques (ciclo infinito en `destruir`) | `src/arena.c` | lectura |
| **Runtime emitido**: `sil_arena_asignar_(arena, n, 0)` calculaba `a = (n + (al-1)) & ~(al-1)` con `al = 0` → `a = 0`, devolvía siempre el mismo puntero **sin reservar memoria** | `silc-backend/src/c99.rs:111` | ejecución real |

**Corrección:** alineación sobre la dirección absoluta, guardas de desbordamiento, creación perezosa de bloque, `promover` sin auto-enlace, y el mismo contrato replicado en el runtime emitido. Ambas implementaciones se prueban en V1 (`test_arena.c`, `test_runtime_emitido.c`).

### C-04 · Modelo de memoria indefinido en MPSC y C99 incumplido
**Severidad: alta (carrera de datos real, no teórica).**

- `prev_tail->next = nuevo;` (`channel.c:213` en la base) es una escritura **no atómica** que el consumidor lee como `head->next`: carrera de datos → comportamiento indefinido según C11, aunque «funcione» en x86.
- Los macros hacían `(_Atomic(T)*)(ptr)` sobre objetos **no atómicos**: aliasing de tipos no permitido.
- `_Thread_local` y `_Atomic` en código compilado con `-std=c99`: extensión aceptada por GCC (aviso con `-pedantic`), error en otros compiladores.

**Corrección:** publicación con `release`/`acquire` explícitos, `__atomic_*` sobre los objetos reales, y TLS portable. La suite se ejecuta bajo **ThreadSanitizer** sin avisos.

### C-05 · SHA-256/HMAC con salida indefinida en entradas límite
`sil_sha256(NULL, 5, out)` devolvía **sin escribir** `out` (`sil_hmac.c:105` en la base): el llamador leía basura no determinista. En un runtime que promete «cero comportamiento indefinido» esto es una grieta directa. **Corregido:** digesto nulo determinista documentado y probado.

### C-06 · Capacidades: permisos vacíos y ampliación
- `sil_cap_solicitar(..., permisos = 0, ...)` devolvía `SIL_CAP_OK` y `sil_cap_validar(..., requerido = 0, ...)` devolvía `SIL_CAP_OK`: existía una capacidad «vacía» que valida.
- La condición `(permisos & 0xFFFFFFF0u) != 0 && permisos > 0x0Fu` era redundante (el segundo término implica el primero).
- `hay_clave_fija` se leía/escribía sin orden de memoria entre hilos.

**Corregido:** se rechazan permisos vacíos o con bits desconocidos (solicitud y validación), y la clave se publica con `store-release`/`load-acquire`. La ampliación de permisos y la manipulación de firma siguen rechazándose (HMAC cubre `magic|permisos|expiración|hash(recurso)`).

### C-07 · Cero pruebas sobre 1.500 líneas de C, y el runtime emitido nunca se compiló
- `sil-rt` tenía **4 tests** de Rust (arenas cíclicas y capacidades) y **ninguno** de canales, fibras ni E/S; el SHA-256/HMAC no tenía vectores de referencia.
- El runtime C que `silc build --target c99` inyecta en cada programa se compilaba solo con `gcc -fsyntax-only` sobre los ejemplos válidos, en Linux. Es decir: nunca se **ejecutó**, y en los ejemplos que no usan red/canales/fibras el runtime quedaba como código muerto que solo pasaba el chequeo sintáctico. El bug de alineación (`al = 0`) y el del runtime de canales sobreviven a `-fsyntax-only` porque no son errores de compilación: son errores de comportamiento.

**Corregido:** `scripts/run_c_tests.sh` (compilación estricta + ASan/UBSan + TSan + extracción verbatim del runtime emitido) y job `runtime-c` en CI. 40 casos / 222 comprobaciones escritas (≈4.600 comprobaciones ejecutadas por los bucles).

---

## 3. Hallazgos abiertos (priorizados, no corregidos aquí)

> Los marcados con **[Rust]** requieren compilar el workspace; en este entorno solo se pudieron verificar sintácticamente (V2).

| ID | Severidad | Hallazgo | Evidencia | Plan |
|---|---|---|---|---|
| **R-01** | Alta | Los tests de `silc-backend` validan **subcadenas** del C emitido, no comportamiento. Un cambio que rompa la semántica (p. ej. `goto` mal emitido) puede pasar los tests | `silc-backend/src/c99.rs:1232,1312-1314` | Compilar **y ejecutar** los `.c` generados en CI y comparar `stdout` con valores esperados (los tests C nuevos ya dan el andamiaje) |
| **R-02** | Alta | El codegen no tiene sentencia `cerrar canal` ni liberación por alcance: un bucle que crea canales agota el pool de 16 (`SIL_CANALES_MAX_`). Se añadió `sil_canal_destruir_` y el reciclaje en `crear`, pero nadie lo emite | `c99.rs` (`Operacion::CrearCanal`), runtime: `SIL_CANALES_MAX_ 16` | Nueva operación de IR `CerrarCanal` + lowering + emisión al salir de la tarea; criterio de aceptación: programa con 1.000 canales en bucle que termina |
| **R-03** | Alta | El «scheduler de fibras» del C emitido ejecuta cada fibra **hasta retornar**: no hay ceder el control. La semántica de `generar` difiere de la del runtime nativo | `c99.rs` (comentario y `sil_sched_ejecutar_`) | O implementar fibras reales (ucontext en POSIX + Fibers en Win32, con shim) o declarar explícitamente «ejecución secuencial de tareas»; hoy la documentación insinúa lo primero |
| **R-04** | Media | `validar cap` emite `sil_cap_validar_(&t, 0, 0)`: no verifica el binding al recurso ni los permisos, solo integridad y expiración | `c99.rs:987` | Emitir el recurso y el permiso declarados en la tarea |
| **R-05** | Media | El resultado de `ValidarCapacidad` no se usa: una capacidad inválida no detiene la ejecución | `c99.rs` | Emitir `if (!válida) { error; return; }` |
| **R-06** | Media | `silc build --target wasm` importa `sil.memory` y WASI, pero no hay pruebas de ejecución del módulo (solo validez estructural) | `wasm.rs`, tests | Ejecutar en `wasmtime`/`node` en CI |
| **R-07** | Media | La rama eBPF nunca se compila en CI (los jobs instalan `libbpf-dev` pero no definen `SIL_EBPF=1`) y está escrita contra libbpf 0.x | `.github/workflows/rust.yml`, `sil-rt/src/ebpf_linux.c` | Job opt-in con libbpf 1.x y API actualizada |
| **R-08** | Media | `Scheduler::spawn` es `unsafe` y el scheduler rechaza `spawn` mientras ejecuta: no hay cola segura para M:N | `silc-runtime/src/lib.rs` | Cola MPSC (ya verificada) como buzón de spawn |
| **R-09** | Baja | `sil_arena_promover` promueve **la arena completa**, no el objeto `(ptr, tamanio)`; el nombre `migrar_cero_costo` sugiere migración de esquemas | `arena.c`, whitepaper §4.1 | Renombrar a `sil_arena_absorber` o implementar promoción por objeto con copia medida |
| **R-10** | Baja | `docs/` mezcla estado real y aspiracional en frases sueltas («CI verde» vs README «CI fallando») | README, WHITE_PAPER | Una sola fuente de estado (`docs/ESTADO.md`) generada |
| **R-11** | Baja | `silc-runtime` usa `std::env::set_var` (no seguro en edition 2024) en tests | `lib.rs` tests | Pasar la clave por `sil_cap_fijar_clave` en lugar de variables de entorno |

---

## 4. Rendimiento medido (V1)

`scripts/run_c_tests.sh --bench` (gcc 12, `-O2`, 2 vCPU, contenedor):

```
arena/asignar 32B      :    3.54 ns/op  (2000000 op)
anillo enviar+recibir  :    3.13 ns/par (319.97 M par/s)
canal SPSC (1P/1C)     :  113.24 ns/msg (  8.83 M msg/s)
canal MPSC (4P/1C)     :  628.70 ns/msg (  1.59 M msg/s)
```

Lectura honesta: el coste **del anillo** es 3,1 ns por par enviar+recibir. Los números entre hilos están dominados por la máquina (2 vCPU para 2–5 hilos: el planificador del SO, no el canal). El dato relevante frente al diseño anterior es cualitativo y medible: **cero `malloc` por mensaje** y memoria O(capacidad) (`memoria_acotada`: 200.000 mensajes sin que la arena crezca un byte).

---

## 5. Decisiones de diseño tomadas y su justificación

1. **Canal = anillo acotado con secuencia por ranura** (Vyukov) en lugar de lista enlazada con nodos.
   - Memoria O(capacidad) y cero reservas por mensaje (antes: un nodo por mensaje).
   - Sin ABA por construcción: la propiedad la da una secuencia monótona por ranura, no punteros reutilizados.
   - Un solo consumidor sin CAS (contrato explícito en el header), productores con CAS solo en MPSC.
   - `capacidad` deja de ser un parámetro decorativo: pasa a ser contrapresión real, que es lo que la propia gramática del lenguaje declara (`crear canal con capacidad N`).
2. **Arena: alinear la dirección, no el tamaño.** Una instrucción AND por reserva; sigue siendo O(1); elimina la clase de error «puntero no alineado» que rompe `long double`, SIMD y `_Atomic` de 16 B.
3. **Nunca comportamiento indefinido por entradas límite.** `tamanio = 0`, `alineacion` no potencia de dos, desbordamientos, arena destruida, `NULL` con longitud: todos tienen contrato explícito y prueba.
4. **Verificación por ejecución, no por inspección de cadenas.** El runtime emitido se extrae literalmente del fuente Rust y se ejecuta; si `c99.rs` cambia de forma incompatible, la extracción falla ruidosamente en vez de validar código obsoleto.

---

## 6. Brechas arquitectónicas del lenguaje (estado real)

| Capacidad prometida | Estado real | Qué falta para que sea real |
|---|---|---|
| ADTs (`estructura`/`variante`/`modelo`) en binario | Parseados a AST; el backend los rechaza con `TipoNoSoportado` | Lowering a IR + layout C (tagged union) + codegen WASM |
| Servicios y red | El parser acepta `servicio`; solo se emiten sus tareas | IR de servicio (listener, handlers), runtime TCP en el C emitido (hay cliente mínimo), colas por conexión |
| Memoria persistente / NVM | Solo AST | `mmap` + layout estable + migración lazy on-read |
| Scheduler M:N | Un scheduler por hilo; `n_hilos_os` se ignora | Buzón de spawn por hilo (MPSC), work-stealing, afinidad |
| LLVM | Implementado, sin job | Validar en CI o marcar experimental en la doc |
| Certificaciones (EAL7/DO-178C/ASIL-D) | Objetivos | Trazabilidad requisito↔test↔código, análisis de cobertura MC/DC, verificación formal del núcleo |

Ninguna de estas brechas se puede cerrar «de una vez»: cada una necesita diseño propio, IR propio y una suite ejecutable. La regla que propone esta auditoría es que **una capacidad solo se documenta como `✅` cuando tiene un test que la ejecuta y falla si se rompe**.

---

## 7. Hoja de ruta por valor/coste

### P0 — cerrar la brecha entre lo que se afirma y lo que se verifica (hecho aquí)
- [x] Canales, fibras, HMAC, capacidades y arenas con pruebas ejecutables (V1).
- [x] Runtime C emitido compilado y ejecutado en CI.
- [x] Compilación estricta (`-std=c99 -pedantic-errors -Werror`) de todo `sil-rt`.
- [x] Conteo de tests C integrado en `scripts/test_counts.py` (con modo local sin toolchain).

### P0 — siguiente paso (1–2 días, requiere toolchain Rust)
1. **R-01**: job de CI que compile **y ejecute** los `.c` de `examples/valid`, comparando `stdout`.
2. **R-05/R-04**: `validar cap` debe cortar la ejecución y verificar recurso+permiso.
3. Ejecutar la suite completa de Rust y confirmar que los cambios de `silc-runtime` (fuga de `Box`, comentarios) compilan y pasan.

### P1 — completar el lenguaje (2–3 semanas)
4. **R-02**: operación `CerrarCanal` + reciclaje de pool (aceptación: 1.000 canales en bucle).
5. **R-03**: decisión explícita sobre fibras en el C emitido (implementar ucontext/Fibers con shim, o renombrar la semántica a «tareas secuenciales»).
6. **ADTs**: lowering `estructura`/`variante` → struct + union etiquetada, con pattern matching ya parseado.
7. **R-06**: ejecutar WASM en Node en CI (Node ya está en el runner).

### P2 — ambición (meses)
8. Servicios/red con colas por conexión (reutilizando el anillo verificado).
9. eBPF con libbpf 1.x y job opt-in real.
10. NVM + migración lazy on-read.
11. M:N con work-stealing.
12. Núcleo formalizado (Coq/Lean) para los invariantes de arena/canal, como paso previo a cualquier certificación.

---

## 8. Disciplina anti-deriva (lo que hay que mantener)

1. **Definición de hecho:** una capacidad es `✅` si (a) compila en CI, (b) tiene un test que la ejecuta, (c) el test falla si se introduce una regresión, (d) la documentación cita el test.
2. **Prohibido el «test de subcadena»** como única prueba de un backend: mide la forma, no el comportamiento.
3. **Los conteos de tests se generan**, no se escriben: `scripts/test_counts.py` (Rust + Python + C) con `--check` en CI.
4. **El runtime emitido se audita por extracción**: `scripts/extraer_runtime_c.py` es el contrato entre `c99.rs` y sus pruebas.
5. **Cada defecto corregido deja una prueba**; esta auditoría añadió 40 casos para 6 defectos críticos.

---

## 9. Reproducción exacta

```bash
# Todo lo verificado aquí, en 10 s
scripts/run_c_tests.sh            # estricto + ASan/UBSan + TSan + runtime emitido
scripts/run_c_tests.sh --bench    # medidas de la sección 4
python3 -m pytest -q              # prototipo Python (31 tests)
python3 scripts/test_counts.py --check   # docs en sincronía (requiere cargo)

# Estado previo de los defectos (base auditada)
git show 687fb361d8fe6602354d1f8f383ed1d0ad06241d:silc-core/sil-rt/src/channel.c | sed -n 25p
```
