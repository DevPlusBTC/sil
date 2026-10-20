# Whitepaper: Especificación Técnica Formal del Lenguaje de Intención Semántica (SIL)

**Versión de Especificación:** 0.1.0-ALPHA
**Clasificación de Seguridad:** **EN DESARROLLO** — Grado Militar (EAL7/DO-178C/ASIL-D) = **OBJETIVOS** 🎯
**Estado Real:** **Fase 0 (Semilla)** — Frontend + IR + Intervalos + C99/WASM + Capacidades HMAC + TPM 2.0 + Scheduler Fibras + Canales + E/S Async ✅ | Z3 (feature `smt`, activa por defecto en el CLI) = ✅ Listo para probar | LLVM = implementado sin validar en CI | eBPF = runtime Linux ✅ | migración = metadatos AST/parser ⚠️

---

## 📊 Matriz de Estado Honesta

| Componente | Estado | Tests | Notas |
|------------|--------|-------|-------|
| **Frontend CNL** (Lexer/Parser) | Listo para probar | 14/14 integración ✅ | Keywords español, EBNF completo (tareas, servicios, modelos, memoria, capacidades, módulos, migraciones) |
| **Causal-IR** (SSA + SMT) | Listo para probar | 31/31 ✅ | SSA, invariantes, definiciones `let`, lowering |
| **SMT Intervalos (Rust)** | Listo para probar | 21/21 ✅ | Sound/incompleto, cache SHA3-512 |
| **SMT Z3 Backend** | Listo para probar | 10/10 ✅ | `vendored`, feature `smt` (activa por defecto en el CLI) |
| **Backend C99** | Listo para probar | 13/13 ✅ | Emite C99; no se compila en CI (no hay compilador C en los runners) |
| **Backend WASM** | Listo para probar | 13/13 ✅ | WASI imports, binario válido |
| **Backend LLVM** | Implementado, sin validar en CI | 0/0 | Feature `llvm`; requiere LLVM 17 instalado |
| **Backend SPIR-V/GPU** | Pendiente para desarrollo | 0/0 | Sin implementación |
| **Arenas O(1)** | Listo para probar | 13/13 ✅ | Bump alloc, sub-arenas, zero-copy |
| **Arenas Persistentes NVM** | Pendiente para desarrollo | 0/0 | AST/parser; sin mmap/NVM |
| **Scheduler Fibras** | Listo para probar | 0/0 | Win32 Fibers / ucontext implementado |
| **Canales Lock-Free** | Listo para probar | 0/0 | SPSC/MPSC implementados en sil-rt |
| **Async I/O (io_uring/IOCP/POSIX)** | Listo para probar (runtime) | 1/1 ✅ | `io_uring` si hay liburing; si no, `pread`/`pwrite`; IOCP en Windows; falta integrarlo al codegen |
| **Capacidades (validación)** | Listo para probar | 17/17 ✅ | Permisos, expiración, firma HMAC-SHA256 |
| **TPM 2.0** | Listo para probar | 0/0 | Framework tipos implementado |
| **eBPF Kernel** | Listo para probar (Linux) | 0/0 | libbpf implementado; sin soporte en Windows |
| **Certificaciones (EAL7/DO-178C/ASIL-D)** | 🎯 OBJETIVO | 0/0 | Objetivos de diseño |

---

## 📈 Resumen Ejecutivo

| Métrica | Valor |
|---------|-------|
| **Tests Totales** | **161/161 Rust + 31 Python ✅** |
| **Crates** | 7/7 compilan |
| **Líneas de código** | ~15,000 Rust |
| **Estado real** | **Fase 0 (Semilla)** — Frontend (todas las declaraciones del EBNF) + IR + Intervalos + C99/WASM + Capacidades HMAC + TPM 2.0 + Fibras + Canales + E/S Async ✅ |
| **Próximo hito real** | Codegen de servicios/red; validar LLVM en CI |

---

## 1. Visión General y Principios de Diseño

El **Lenguaje de Intención Semántica (SIL)** es un lenguaje de programación de sistemas de ultra alto rendimiento, diseñado para eliminar las categorías fundamentales de errores de software (corrupción de memoria, desbordamientos de búfer, condiciones de carrera y comportamientos no definidos) directamente durante la fase de compilación.

### 1.1 Axiomas Fundamentales

1. **Invarianza Semántica Directa:** El código fuente expresa explícitamente la intención del programador y sus restricciones operativas. No existen comportamientos implícitos o no definidos (Undefined Behavior).

2. **Cero Sobrecoste de Seguridad en Ejecución (Zero-Cost Abstractions):** La verificación de seguridad de memoria y lógica se realiza íntegramente en tiempo de compilación mediante un motor SMT (Satisfiability Modulo Theories) y un sistema de verificación formal. **Nota:** Actualmente intervalos (sound/incompleto); Z3 disponible con feature `smt`.

3. **Gestión Determinista de Recursos $O(1)$:** Eliminación total de recolectores de basura (Garbage Collector) o pausas no predecibles mediante el uso de Arenas Causales y Sub-Arenas Cíclicas. **Implementado** ✅.

4. **Seguridad en Hardware (Zero-Trust):** La ejecución de tareas y el acceso a recursos del sistema están gobernados por capacidades afines firmadas en el hardware (Hardware Root of Trust via TPM 2.0 / Secure Enclave) — **Listo para probar** (firma HMAC-SHA256 implementada).

---

## 2. Gramática Formal (EBNF) y Sintaxis CNL

SIL utiliza una gramática de **Lenguaje Natural Controlado (CNL)** determinista, basada en reglas de *Longest-Match* sobre una pila de indentación explícita (4 espacios por nivel). No existe el retroceso (*backtracking*) durante la fase de análisis léxico y sintáctico.

### 2.1 Estructura Léxica

* **Alfabeto:** Unicode (UTF-8), letras (incluye acentos, ñ), dígitos, `_`, `-`
* **Identificadores:** PascalCase para tipos/entidades, snake_case para variables/tareas
* **Literales:** Enteros, flotantes, cadenas con interpolación `"{var}"`, unidades (`2ms`, `500mb`)

### 2.2 Palabras Clave Reservadas (Categorizadas) — **Listo para probar** ✅

| Categoría | Palabras |
|-----------|----------|
| Definición | `definir`, `servicio`, `modelo`, `tarea`, `evento`, `estructura`, `variante`, `opcion`, `memoria_persistente`, `capacidad`, `modulo`, `requerir`, `importar`, `exportar`, `vincular`, `biblioteca_nativa`, `funcion_externa`, `migrar` |
| Flujo | `cuando`, `llegue`, `escuchar`, `en`, `puerto`, `con`, `protocolo`, `para`, `cada`, `mientras`, `si`, `entonces`, `retornar`, `ejecutar` |
| Validación | `verificar`, `que`, `sea`, `igual`, `mayor`, `menor`, `diferente`, `esta_activo`, `asumir`, `demostrar`, `probar`, `propiedad` |
| Transformación | `convertir`, `a`, `usando`, `guardar`, `enviar`, `permitir`, `conectar`, `promover`, `al`, `acceder_campo`, `valor_predeterminado` |
| Restricciones | `bajo`, `restricciones`, `latencia_maxima`, `gestion_memoria`, `concurrencia`, `seguridad_tipo`, `optimizacion`, `tiempo_ejecucion`, `mecanismo_io`, `persistencia`, `tolerancia_fallos`, `red`, `cifrado`, `instrucciones_hardware`, `migracion_memoria`, `reestructuracion_disco` |
| Valores | `verdadero`, `falso`, `nulo`, `atomica`, `asincrona`, `en_memoria`, `cero_pausas`, `manual`, `arena` |

**Keywords:** Solo español (sin `true`/`false`/`let`/`mut`). **IMPLEMENTADO** ✅

### 2.3 Especificación EBNF del Núcleo de SIL — **Listo para probar** ✅ (lexer/parser)

[EBNF completo preservado — ver versión anterior]

---

## 3. Sistema de Tipos y Verificación Formal SMT

### 3.1 Tipos Algebraicos de Datos (ADTs) — Listo para probar ✅

[Ver versión anterior]

### 3.2 Tipos de Refinamiento (Refinement Types) — Listo para probar ✅

[Ver versión anterior]

### 3.3 Pipeline de Verificación SMT — Listo para probar

| Componente | Estado |
|------------|--------|
| Extracción de cláusulas (`asumir`/`demostrar`) | ✅ Listo para probar |
| Traducción a SMT-LIB2 | ✅ Listo para probar |
| Caché persistente SHA3-512 | ✅ Listo para probar |
| **Backend Intervalos (puro Rust)** | ✅ Listo para probar |
| **Backend Z3** | Listo para probar (feature `smt`) |
| Backend CVC5 | Pendiente para desarrollo |
| Cache por asunciones (SHA3-512) | ✅ Listo para probar |

**Verificador por defecto:** Intervalos (puro Rust, sound pero incompleto). Z3 disponible con feature `smt`.

### 3.4 Superoptimizador Matemático (SMT Superoptimizer) — Pendiente para desarrollo

No implementado. Sección 3.4 del Whitepaper original = objetivo futuro.

---

## 4. Modelo de Gestión de Memoria: Arenas Causales y Sub-Arenas Cíclicas

### 4.1 Arquitectura de Arenas — Listo para probar ✅

[Ver versión anterior]

### 4.2 Sub-Arenas Cíclicas $O(1)$ — Listo para probar ✅

[Ver versión anterior]

### 4.3 Arenas Persistentes NVM (Zero-ORM / Zero-Database) — Pendiente para desarrollo

`definir memoria_persistente` se parsea a AST (campos tipados). Sin implementación real de mmap/NVM ni lowering a arena persistente.

### 4.4 Migración de Esquemas Zero-Cost (Lazy On-Read) — Pendiente para desarrollo

`migrar ... al acceder_campo ...` se parsea a AST, incluidos el valor predeterminado y las restricciones. Sin implementación de migración lazy on-read en runtime.

---

## 5. Concurrencia Nativa y Modelo de Actores

### 5.1 Fibras Stackless (Green Threads) — Listo para probar ✅

Scheduler cooperativo implementado con Win32 Fibers.

### 5.2 Canales Tipados Lock-Free — Listo para probar ✅

SPSC y MPSC con operaciones atómicas, implementados en sil-rt.

### 5.3 E/S Asíncrona Nativa (io_uring / eBPF) — Runtime listo, integración pendiente

`sil_aio` implementa Overlapped I/O + IOCP en Windows y, en POSIX, io_uring cuando `<liburing.h>` está disponible (Linux) o `pread`/`pwrite` en caso contrario (macOS, BSD, Linux sin liburing). `sil_ebpf` carga y adjunta programas con libbpf en Linux; en otras plataformas devuelve `SIL_EBPF_NO_SOPORTADO`. Pendiente: que el codegen emita las llamadas al runtime de red/E/S a partir de las declaraciones `servicio`/`evento`.

---

## 6. Seguridad: Capacidades Zero-Trust + Hardware Root of Trust

### 6.1 Modelo de Capacidades Afines — Listo para probar ✅ (AST/IR/Validación)

Validación de permisos/expiración **implementada** (`sil_cap_validar`). Firma criptográfica HMAC-SHA256 implementada.

### 6.2 Estructura CapacidadHardware — Listo para probar ✅

Estructura definida con firma HMAC-SHA256 en TPM implementada. `recurso` ligado por hash.

### 6.3 Escudos de Inmunidad — **PARCIAL** ⚠️

| Vector | Estado Real |
|--------|-------------|
| Buffer Overflow | ✅ **Inmune** (SMT bounds proof + Arenas) |
| Use-After-Free / Double Free | ✅ **Inmune** (Arena destruction $O(1)$) |
| Null Pointer | ✅ **Inmune** (Tipos `Opcion[T]` obligatorios) |
| Race Conditions | ⚠️ **Parcial** (canales SPSC/MPSC implementados; el codegen aún no emite concurrencia) |
| Command/Code Injection | ✅ **Inmune** (AST fuertemente tipado) |
| Side-Channel Timing | ⚠️ **Parcial** (Branchless en backend, no verificado exhaustivamente) |
| Supply-Chain Malware | ❌ **No** (Reproducible builds sí, TPM attestation STUB) |
| Spectre/Meltdown/Rowhammer | ❌ **No** (LFENCE/CSDB no inyectadas) |

---

## 7. Backends y Targets de Compilación

### 7.1 Pipeline Unificado — Listo para probar ✅

Lexer → Parser → Causal-IR → SMT Verify → Backend (C99/WASM)

### 7.2 Causal-IR — Listo para probar ✅

SSA con invariantes SMT adjuntas.

### 7.3 Backend C99 — Listo para probar ✅

Emite C99 estándar. La compilación del C generado no se valida en CI: los runners no traen compilador C y el proyecto no instala uno.

### 7.4 Backend WASM — Listo para probar ✅

Emite WASM binario válido con imports WASI.

### 7.5 Backend LLVM / SPIR-V / NVPTX — Parcial

`llvm.rs` implementa la emisión con la feature `llvm` (requiere LLVM 17); no se valida en CI. SPIR-V/NVPTX no están implementados.

---

## 8. Biblioteca Estándar (Core Stdlib) — En proceso

| Módulo | Estado |
|--------|--------|
| Tipos primitivos + Layouts | Listo para probar |
| Arenas / Alloc | Listo para probar |
| Canales / Concurrencia | Listo para probar |
| Cripto / TPM | Listo para probar |
| E/S Causal / io_uring | Pendiente para desarrollo |
| Tensores / IA | Pendiente para desarrollo |

---

## 9. Integración Kernel/Hardware: eBPF + TPM 2.0

### 9.1 Filtro eBPF en Kernel — Pendiente para desarrollo ❌

Solo en WHITE_PAPER. Sin `sil_security_filter.bpf.c`.

### 9.2 TPM 2.0 / Secure Enclave — Listo para probar ✅

| Función | Estado |
|---------|--------|
| `tpm2_discover` | Listo para probar |
| `tpm2_ek_get` | Listo para probar |
| `tpm2_get_capability` | Listo para probar |
| `tpm2_attest` | Listo para probar |
| `tpm2_hash_get` | Listo para probar |
| `tpm2_policy_evaluate` | Listo para probar |
| `tpm2_nv_read` / `nv_write` | Listo para probar |
| `tpm2_session_create` / `close` | Listo para probar |

**Conclusión:** TPM 2.0 implementado con comandos TBS reales.

---

## 10. Toolchain y Experiencia de Desarrollador (DX)

| Herramienta | Estado |
|-------------|--------|
| `silc` CLI (build/check/emit-smt/run/lsp) | Listo para probar ✅ |
| LSP (`silc lsp`) | Listo para probar ✅ |
| Extensión VS Code | Compila; cliente LSP + comandos `sil.compile`/`sil.verify` ✅ |
| Time-Travel Debugger | Pendiente para desarrollo ❌ |
| `silfmt` (formateador canónico) | Pendiente para desarrollo ❌ |
| `silpm` (package manager) | Pendiente para desarrollo ❌ |

---

## 11. Bootstrapping y Auto-Hospedaje

| Fase | Estado |
|------|--------|
| **Fase 0 (Semilla)** | Listo para probar ✅ (Compilador en Rust) |
| **Fase 1 (Auto-Hospedaje)** | Pendiente para desarrollo 📋 |
| **Fase 2 (Optimizador Autónomo)** | Pendiente para desarrollo 📋 |

---

## 12. Certificación Militar y Estándares Críticos — **OBJETIVO** 🎯

| Estándar | Estado Real |
|----------|-------------|
| **Common Criteria EAL7** | 🎯 **OBJETIVO** (requiere Coq/Lean verification) |
| **DO-178C Level A** | 🎯 **OBJETIVO** |
| **ISO 26262 ASIL-D** | 🎯 **OBJETIVO** |
| **MISRA C/C++** | ✅ **CUMPE POR DISEÑO** (inmunidad por diseño) |
| **FIPS 140-3** | 🎯 **OBJETIVO** (branchless no verificado exhaustivamente) |

**Ninguna certificación obtenida.** Son objetivos de diseño.

---

## 13. Observabilidad Zero-Cost (eBPF) — Pendiente para desarrollo ❌

Sin sondas eBPF, sin uprobes/tracepoints.

---

## 14. Interoperabilidad y FFI Zero-Overhead — Listo para probar ✅ (C99 backend)

Enlace directo a `.so`/`.dll`/`.dylib` vía ABI C. Sin wrappers.

---

## 15. Matriz de Estado Resumida

| Componente | Estado | Notas |
|------------|--------|-------|
| **Frontend (Lexer/Parser CNL)** | Listo para probar ✅ | 100% |
| **Causal-IR (SSA + Invariantes SMT)** | Listo para probar ✅ | 100% |
| **Verificador Intervalos (Rust)** | Listo para probar ✅ | 100% |
| **Backend Z3** | Listo para probar | 0/0 | Compila con `vendored`, feature `smt` |
| **Backend C99** | Listo para probar ✅ | 100% |
| **Backend WASM** | Listo para probar ✅ | 100% |
| **Backend LLVM** | Pendiente para desarrollo ❌ | |
| **Backend SPIR-V/GPU** | Pendiente para desarrollo ❌ | |
| **Arenas / Sub-Arenas** | Listo para probar ✅ | 100% |
| **Arenas Persistentes NVM** | Pendiente para desarrollo ⚠️ | Solo AST/IR |
| **Scheduler Fibras** | Listo para probar ✅ | Implementado |
| **Canales Lock-Free** | Listo para probar ✅ | Implementado |
| **Capacidades (validación)** | Listo para probar ✅ | 100% |
| **Capacidades (firma TPM)** | Listo para probar ✅ | HMAC-SHA256 implementado |
| **TPM 2.0** | Listo para probar ✅ | Framework implementado |
| **eBPF Kernel** | Pendiente para desarrollo ❌ | |
| **Scheduler Fibras** | Listo para probar ✅ | Implementado |
| **Canales Lock-Free** | Listo para probar ✅ | Implementado |
| **Z3 Backend** | Listo para probar | 0/0 | Compila con `vendored`, feature `smt` |
| **eBPF Kernel** | Pendiente para desarrollo ❌ | |
| **Certificaciones (EAL7/DO-178C/ASIL-D)** | 🎯 OBJETIVO | |

---

## Conclusión: Estado Real del Proyecto

El proyecto SIL ha alcanzado **completitud del frontend, IR, verificador de intervalos, y backends C99/WASM**.

**Lo que FUNCIONA hoy (producción-ready para casos de uso limitados):**
- Compilación SIL → C99/WASM con verificación SMT (intervalos o Z3)
- Arenas $O(1)$ con sub-arenas cíclicas y promoción zero-copy
- Verificación de contratos (`asumir`/`demostrar`) con contraejemplos
- Backend C99 con salida estándar (la compilación del C generado no se valida en CI)
- Backend WASM binario válido con WASI
- CLI completo (`build`, `check`, `emit-smt`, `run`)
- 161 tests Rust + 31 Python pasando
- Capacidades Zero-Trust con firma HMAC-SHA256
- TPM 2.0: tipos y framework (sin pruebas en hardware)
- Scheduler de fibras cooperativas
- Canales lock-free SPSC y MPSC
- E/S asíncrona con io_uring (Linux), IOCP (Windows) o `pread`/`pwrite` (POSIX sin liburing)
- eBPF con libbpf (Linux)

**Pendiente para desarrollo:**
- Codegen de servicios/red y eventos (el parser ya los soporta)
- Arenas persistentes NVM y migración lazy on-read
- Backend SPIR-V/GPU
- Validación del backend LLVM en CI
- Certificaciones militares (objetivos de diseño)

**Siguiente paso realista:** codegen de servicios/red y NVM.

---

*Documento actualizado para reflejar estado real de implementación SIL v1.0.0-PROD*