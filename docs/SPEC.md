# SIL Language Specification (Implemented v0.1.0)

**Versión:** 0.1.0 | **Fecha:** 2026-10-04 | **Estado:** Beta Funcional

---

## 1. Resumen de Implementación Actual

Este documento describe **qué está implementado y funcional** en SIL v0.1.0 vs la especificación completa del [WHITE_PAPER.md](WHITE_PAPER.md).

| Componente | Implementado | Notas |
|------------|--------------|-------|
| Frontend (S-exp Reader + AST Builder) | Listo para probar | Crate del workspace; CNL legacy removido |
| Causal-IR + SMT | Listo para probar | Intervalos + Z3 |
| Backend C99 | Listo para probar | Emite C99; los tests lo compilan con gcc/clang `-Wall -Wextra -Werror` en Linux/macOS |
| Backend WASM | Listo para probar | Magic `\0asm` válido |
| Runtime FFI | Listo para probar | Arena O(1) + sub-arenas cíclicas (16 niveles), cap. HMAC, TPM 2.0, fibras, canales |
| CLI (`silc`) | Listo para probar | 6 comandos: check, build, emit-smt, emit-rust, run, lsp |
| LSP Server | Listo para probar | `silc lsp`: JSON-RPC sobre stdio, diagnósticos y completion |
| VS Code Extension | Listo para probar | Cliente LSP + comandos `sil.compile`/`sil.verify`; falta `.vsix` |
| Tests | Listo para probar | 205 Rust + 31 Python + 52 C |

**Pendiente para desarrollo:** Codegen de servicios/red, arenas persistentes NVM,
migración de esquemas lazy, SPIR-V/GPU, eBPF (opt-in `SIL_EBPF=1`; la rama
libbpf 0.x no compila con libbpf 1.x), M:N scheduler, superoptimizer,
certificaciones militares.

---

## 2. Gramática SIL Implementada (S-expressions)

La sintaxis de SIL v0.1.0 es un subconjunto mínimo de S-expressions.
No hay INDENT/DEDENT, no hay precedencia de operadores, no hay keywords
reservadas fuera de posición de cabeza de forma.

```ebnf
Documento       ::= { Sexp } ;
Sexp            ::= Atomo | Texto | Numero | Lista ;
Lista           ::= "(" { Sexp } ")" ;
Atomo           ::= Identificador | Operador ;
Identificador   ::= [A-Za-z_] [A-Za-z0-9_]* ;
Operador        ::= "+" | "-" | "*" | "/" | ">" | "<" | "==" | "!=" | ">=" | "<=" ;
Numero          ::= Entero | Flotante ;
Entero          ::= [+-]? [0-9]+ ;
Flotante        ::= [+-]? [0-9]+ "." [0-9]+ ;
Texto           ::= '"' { CarTexto | Escape } '"' ;
Comentario      ::= ";" { cualquier-carácter-salvo-NL } ;
```

**Átomos especiales:** `verdadero` / `falso` son literales booleanos **solo en posición de expresión**.
En posición de enlace (parámetro, campo, variable, token de capacidad) son `IdentificadorInvalido`.

### 2.1 Formas de nivel superior (9 declaraciones)

| Decl | Forma S-exp | Notas |
|------|-------------|-------|
| Tarea | `(task NOMBRE (params PARAM*) (returns TIPO)? (requires EXPR)* (ensures EXPR)* (body CUERPO))` | `params` obligatorio; `returns` opcional → Void |
| Struct | `(struct NOMBRE (fields CAMPO*))` | |
| Model | `(model NOMBRE (fields CAMPO*))` | Igual que struct semánticamente |
| Memory | `(memory NOMBRE (fields CAMPO*))` | Igual que struct semánticamente |
| Variant | `(variant NOMBRE (case NOMBRE (fields CAMPO*))+)` | `(fields ...)` obligatorio incluso vacío |
| Service | `(service NOMBRE (net PUERTO (protocol PROTO)?) (handler EVENTO (params PARAM*) CUERPO)* (task TAREA)* (restrictions RESTRICCION*)?)` | |
| Capability | `(capability NOMBRE (resource "HOST:PORT") (port PUERTO) (bandwidth "LIMITE"))` | 3 campos obligatorios |
| Module | `(module NOMBRE (export CLASE NOMBRE)*)` | `CLASE` ∈ `{service,task,struct,variant,capability}` |
| Migration | `(migration ORIGEN (from VDESDE) (to VA) (field "CAMPO") (record REGISTRO) (default EXPR) (restrictions RESTRICCION*)?)` | |

**Parámetro / Campo:** lista plana de pares `(NOMBRE TIPO)`. TIPO = átomo primitivo o compuesto.

### 2.2 Tipos primitivos y compuestos

| Primitivo | S-exp |
|-----------|-------|
| Entero64 | `i64` |
| Flotante64 | `f64` |
| Booleano | `booleano` |
| Texto | `texto` |
| CapacidadHardware | `capacidad` |
| Void | `void` |
| USD / EUR | `USD` / `EUR` |

| Compuesto | S-exp |
|-----------|-------|
| Lista | `(list TIPO)` |
| Mapa | `(map TIPO_CLAVE TIPO_VALOR)` |
| Conjunto | `(set TIPO)` |
| Tupla | `(tuple TIPO...)` |
| Nominal | `Ident` (p.ej. `MiTipo`) |

### 2.3 Cuerpo y sentencias (23 formas)

`Cuerpo` = `(body STMT* (restrictions RESTRICCION*)?)`. `restrictions` **debe ser el último hijo**.

| Stmt | Forma | Aridad |
|------|-------|--------|
| Si | `(if COND CUERPO (else CUERPO)?)` | 3 o 4 |
| Verificar | `(assert COND)` | 2 |
| Asumir | `(assume COND)` | 2 |
| Demostrar | `(prove COND)` | 2 |
| InvarianteBucle | `(invariant COND)` | 2 |
| Mientras | `(while COND CUERPO)` | 3 |
| Para | `(for VAR ITER CUERPO)` | 4 |
| Asignar (sin tipo, sin mut) | `(let NOMBRE EXPR)` | 3 |
| Asignar (con tipo, sin mut) | `(let NOMBRE TIPO EXPR)` | 4 |
| Asignar (sin tipo, mut) | `(let-mut NOMBRE EXPR)` | 3 |
| Asignar (con tipo, mut) | `(let-mut NOMBRE TIPO EXPR)` | 4 |
| Retornar (con expr) | `(return EXPR)` | 2 |
| Retornar (sin expr) | `(return)` | 1 |
| SolicitarCapacidad | `(request PERMISO "RECURSO" (ttl N)? (as NOMBRE)?)` | 3..5 |
| ValidarCapacidad | `(validate NOMBRE)` | 2 |
| EnviarCanal | `(send EXPR CANAL)` | 3 |
| RecibirCanal | `(recv CANAL NOMBRE)` | 3 |
| GenerarFibra | `(spawn TAREA)` | 2 |
| CrearCanal | `(chan NOMBRE (capacity N)?)` | 2 o 3 |
| OperarIO LeerArchivo | `(io-read "RUTA" CAP NOMBRE)` | 4 |
| OperarIO EscribirArchivo | `(io-write EXPR "RUTA" CAP)` | 4 |
| OperarIO EnviarRed | `(io-send EXPR "DESTINO" CAP)` | 4 |
| OperarIO RecibirRed | `(io-recv "DESTINO" CAP NOMBRE)` | 4 |

**Detalles normativos:**
- `let` vs `let-mut`: dos cabezas distintas, sin átomo `mut` posicional.
- `return` sin expresión: solo válido si la tarea contenedora es Void.
- `if` con `else si`: se anida `(else (body (if ...)))`.
- `request`: orden fijo `ttl` antes de `as`. `PERMISO` ∈ `{lectura_archivo, escritura_archivo, red, ejecucion}`.
- `chan`: `(capacity N)` opcional; ausente → canal sin buffer declarado.
- `io-*`: cuatro cabezas con aridad fija 4. `CAP` = átomo que nombra la capacidad ya solicitada.
- `restrictions`: lista de pares `(clave valor)`; ambos átomos identificadores.

### 2.4 Expresiones (7 formas)

Prefijo uniforme: `(op a b)`. Operandos son sexps.

| Expr | Forma | Aridad |
|------|-------|--------|
| Var | `NOMBRE` (átomo) | — |
| Lit(Entero) | `42`, `-7` | — |
| Lit(Flotante) | `1.5` | — |
| Lit(Texto) | `"hola"` | — |
| Lit(Booleano) | `verdadero`, `falso` | — |
| AccesoProp | `(get BASE PROP)` | 3 |
| BinOp | `(OP LHS RHS)` | 3 |

**Los 10 BinOp:** `>`, `<`, `==`, `!=`, `>=`, `<=`, `+`, `-`, `*`, `/`.

**Reglas:**
- Asociatividad explícita: `a + b + c` → `(+ (+ a b) c)`; `a - b - c` → `(- (- a b) c)`; `a * b / c` → `(/ (* a b) c)`; `a + b * c` → `(+ a (* b c))`.
- Menos unario: `-x` → `(- 0 x)`. Nunca `(- x)` (aridad 2 inválida).
- Comparaciones encadenadas: sintácticamente válidas; rechazo en type-checker.
- Profundidad: límite 256 niveles de listas anidadas en posición de expresión.
- Sin llamadas: `(f x)` es `forma_desconocida`.

---

## 3. Semántica de Contratos (SMT)

### 3.1 `assume` / `prove`
```lisp
(task f
  (params (x i64))
  (returns i64)
  (body
    (assume (> x 0))      ; Premisa (precondición)
    (prove (> x 0))       ; Meta (postcondición)
    (return x)))
```

### 3.2 Pipeline SMT
1. Extracción cláusulas: `assume` → premisas, `prove` → metas
2. Traducción SMT-LIB2: `assert (P ∧ ¬Q)`
3. Resolución con Z3 (por defecto en el CLI) o con verificación de intervalos
   (`--no-default-features`), con caché SHA3-512
4. **UNSAT** → Compilación OK | **SAT** → Contraejemplo + Error

### 3.3 Tipos de Refinamiento
```lisp
(task descuento
  (params (precio i64) (pct i64))
  (returns i64)
  (body
    (assume (> precio 0))
    (assume (>= pct 0))
    (assume (<= pct 50))
    (let final i64 (/ (* precio (- 100 pct)) 100))
    (prove (<= final precio))
    (prove (>= final (/ precio 2)))
    (return final)))
```
Si `pct = 60` → Error: `SMT SAT: contraejemplo pct=60 rompe final >= precio/2`

---

## 4. Backends de Compilación

### 4.1 Target C99 (`--target c99`)
```bash
silc build prog.sil --target c99 -o prog.c
```
- Emite C99. `silc build --cc` compila con `-std=c99 -O2 -Wall`; `silc run` con `-std=c99 -O2`
- Runtime embebido: `sil_arena_*`, tipos `SilTexto_`, `SilCap_`, `Entero64`
- Pensado para GCC/Clang; los tests compilan el C emitido con `-std=c99 -Wall -Wextra -Werror` en Linux y macOS (en Windows se saltan: no hay `gcc`)
- Sin `main` si la última tarea tiene parámetros: se emite una biblioteca

### 4.2 Target WASM (`--target wasm`)
```bash
silc build prog.sil --target wasm -o prog.wasm
```
- Emite WASM válido (magic `\0asm`)
- Validador `wasmparser` integrado en tests
- Importa `sil.memory` y las funciones WASI `fd_write`/`proc_exit`
- Sin `start` si la última tarea tiene parámetros: se emite una biblioteca

### 4.3 Verificación SMT (`emit-smt`)
```bash
silc emit-smt prog.sil
```
Emite SMT-LIB2 (QF_LIA):
```smt
(set-logic QF_LIA)
(declare-fun x () Int)
(assert (> x 0))
(push)
(assert (not (> x 0)))
(check-sat)
(pop)
```

---

## 5. CLI Reference

| Comando | Descripción |
|---------|-------------|
| `silc check <file.sil>` | Verifica sintaxis + contratos SMT |
| `silc build <file.sil> --target c99\|wasm -o <out>` | Genera C99 o WASM |
| `silc emit-smt <file.sil>` | Emite script SMT-LIB2 QF_LIA |
| `silc emit-rust <file.sil> -o <out.rs>` | Emite código Rust legible para auditoría humana |
| `silc run <file.sil>` | Compila a C y ejecuta (requiere tarea principal sin parámetros) |
| `silc lsp` | Inicia servidor LSP (JSON-RPC 2.0 por stdio) |

**Extensiones de archivo:** `.sil` (legacy CNL, deprecated) y `.sexp` (nativo S-exp). El frontend detecta el formato por el primer carácter no-whitespace: `(` → S-exp, otro → error (CNL removido).

**Códigos de salida:** 0=OK, 1=Error (sintaxis, SMT o E/S)

---

## 6. LSP Server (`silc lsp`)

**Protocolo:** LSP 3.17 (JSON-RPC 2.0 sobre stdio, framing `Content-Length`)

**Capacidades implementadas:**
- `textDocument/didOpen` / `didChange` / `didClose` → sincronización completa y diagnósticos léxicos y de sintaxis del frontend real
- `textDocument/completion` → formas S-exp, tipos primitivos y compuestos
- `textDocument/publishDiagnostics` → mensajes del frontend en lenguaje natural
- `initialize`, `initialized`, `shutdown`, `exit`

**Iniciar:** `silc lsp` (stdio; los logs van a stderr)

**Pendiente:** hover, definición, formateador, Time-Travel Debugger.

---

## 7. VS Code Extension (`silc-vscode` v0.1.0)

**Instalación:** `npm install && npm run compile`, luego abrir la carpeta del repositorio como extensión (F5). Requiere `silc` en el `PATH`.

**Features:**
- Cliente LSP contra `silc lsp`: diagnósticos en tiempo real y completion de formas/tipos S-exp
- Comando `SIL: Compilar archivo actual a C99` (`sil.compile`)
- Comando `SIL: Verificar invariantes SMT del archivo actual` (`sil.verify`)

**Grammar:** `syntaxes/sil.tmLanguage.json` (S-exp), `syntaxes/language-configuration.json` (comentarios `;`, brackets `()[]{}`).

**Pendiente:** hover, formateador, empaquetado `.vsix`.

---

## 8. Limitaciones Conocidas v0.1.0

| Limitación | Workaround |
|------------|------------|
| La última tarea con parámetros se emite como biblioteca (sin `main`) | Añadir una tarea principal sin parámetros al final |
| `generar` en C99 requiere tarea principal sin parámetros | Usar WASM (llamada directa) o una principal sin parámetros |
| Los canales tienen capacidad acotada: `enviar` falla con el canal lleno | Dimensionar con `crear canal con capacidad N` o drenar antes de enviar |
| El C emitido no emite `cerrar canal`: el pool es de 16 canales por programa | Evitar crear canales dentro de bucles largos (ver AUDITORIA.md R-02) |
| Los canales aceptan `void *` no nulo; `enviar NULL` devuelve `false` | Enviar un puntero a un valor (el canal no transporta `nulo`) |
| SMT timeout 60s por defecto en postcondiciones complejas | Ajustar `--smt-timeout`; simplificar `prove` |
| Sin stdlib/prelude | Definir tipos base en cada archivo |
| Sin `silpm` / `silfmt` / `silup` | Usar `cargo build` + `silc` directo |
| LSP sin hover, definición ni formateador | Usar completion y diagnósticos |

---

## 9. Versionado y Compatibilidad

- **SemVer:** v0.1.0 = Beta (breaking changes esperados)
- **MSRV:** 1.88.0 (mínimo declarado por las dependencias); CI usa 1.97.1
- **Targets:** x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu, aarch64-apple-darwin (CI)
- **Publicación:** 4 crates en crates.io (`silc-frontend`, `silc-causal-ir`, `silc-backend`, `silc-runtime` v0.1.0); `silc-cli` y `silc-contracts` se instalan desde el repositorio

---

## 10. Tests por crate (autogenerado)

Conteo generado con `python3 scripts/test_counts.py`; la CI ejecuta
`--check` y falla si los documentos quedan desfasados.

<!-- generado-por: scripts/test_counts.py -->
| Crate | Tests |
|---|---|
| `silc-backend` | 30 |
| `silc-causal-ir` | 41 |
| `silc-cli` | 18 |
| `silc-contracts` | 17 |
| `silc-frontend` | 42 |
| `silc-runtime` | 16 |
| `silc-test` | 41 |
| `sil-rt (C, casos)` | 52 |
| **Total** | **257** |
<!-- /generado -->

---

*Ver [WHITE_PAPER.md](WHITE_PAPER.md) para especificación completa aspiracional (EAL7, TPM, eBPF, GPU, etc.)*