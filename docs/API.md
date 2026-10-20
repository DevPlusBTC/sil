# SIL API Reference

Referencia de lo que existe hoy en el repositorio. Para la gramática completa
ver [SPEC.md](SPEC.md); para el diseño aspiracional, [WHITE_PAPER.md](WHITE_PAPER.md).

---

## 1. CLI `silc`

```text
silc [--smt-timeout <ms>] [-v] <comando>
```

| Comando | Descripción |
|---------|-------------|
| `silc check <archivos...>` | Lexer + parser + lowering + verificación de contratos. Exit 0 si todo es válido. |
| `silc build <archivos...> [--target c99\|wasm] [-o <salida>] [--cc]` | Genera C99 (por defecto) o WASM. Con `--cc` compila el C con `cc`/`gcc`/`clang`. |
| `silc emit-smt <archivos...>` | Imprime el script SMT-LIB2 de las tareas. |
| `silc run <archivos...>` | Compila a C, ejecuta con `cc`/`gcc`/`clang` y muestra la salida. Requiere una tarea principal sin parámetros. |
| `silc lsp` | Servidor LSP por stdin/stdout. |

Opciones globales:

| Opción | Valor |
|--------|-------|
| `--smt-timeout <ms>` | Timeout por consulta SMT (entero >= 1; por defecto 5000). |
| `-v`, `-vv` | Nivel de logs (van a stderr para no romper el LSP). |

Convención de compilación: la **última tarea** es la principal. Si tiene
parámetros, el backend emite una biblioteca (sin `main` en C99 y sin `start`
en WASM).

---

## 2. Superficie del lenguaje

Declaraciones: `tarea`, `estructura`, `variante`, `servicio`, `modelo`,
`memoria_persistente`, `capacidad`, `modulo`, `migracion`.

Sentencias: `verificar`, `asumir`, `demostrar`, `let`, `retornar`, `solicitar
capacidad`, `validar`, `crear canal`, `enviar`, `recibir`, `generar`, `leer
archivo`, `escribir`.

Tipos: `Entero64`, `Flotante64`, `Booleano`, `Texto`, `USD`, `EUR`,
`CapacidadHardware`, `Void`, `Lista de T`, `Mapa de K a V`, `Conjunto de T`,
`Tupla(...)` y tipos nominales.

`silc check` usa Z3 por defecto (feature `smt` del CLI). Con
`--no-default-features` usa el verificador de intervalos. A nivel de librería,
`verificar_tarea` usa intervalos y `VerificadorZ3` requiere la feature `smt`.

---

## 3. Crate `silc-frontend`

```rust
use silc_frontend::lexer::{lexear, lexear_con_errores};
use silc_frontend::parser::parsear;

let tokens = lexear(fuente);                 // Vec<TokenFull>
let programa = parsear(&tokens)?;            // Program

// Variante que además devuelve los errores léxicos con posición.
let (tokens, errores) = lexear_con_errores(fuente);
```

Módulos públicos: `ast`, `error`, `lexer`, `parser`.

---

## 4. Crate `silc-causal-ir`

```rust
use silc_causal_ir::lower::bajar_tarea;
use silc_causal_ir::smt::{script_completo, verificar_tarea};

let ir = bajar_tarea(&tarea)?;               // TareaIR
let smt = script_completo(&ir);              // SMT-LIB2 (String)
verificar_tarea(&ir, 5000)?;                 // intervalos
```

Con la feature `smt`:

```rust
use silc_causal_ir::smt::{verificar_tarea_con, CacheLemas, VerificadorZ3};

let mut backend = VerificadorZ3::new();
let mut cache = CacheLemas::cargar();
verificar_tarea_con(&ir, &mut backend, &mut cache, 5000)?;
```

Módulos públicos: `lower`, `nodes`, `smt`. Reexporta `silc-contracts`.

---

## 5. Crate `silc-backend`

```rust
use silc_backend::{c99, wasm};

let c = c99::emitir(&tarea)?;                // una tarea
let programa = c99::emitir_programa(&tareas)?; // todas + main/biblioteca
let bytes = wasm::emitir(&tarea)?;           // módulo de una tarea
let modulo = wasm::emitir_programa(&tareas)?; // todas + start/biblioteca
```

`llvm::emitir(&tarea)` existe detrás de la feature `llvm` (requiere LLVM 17) y
no se valida en CI.

---

## 6. Crate `silc-contracts`

Funciones de contrato que abortan con un mensaje claro si la condición falla:

```rust
use silc_contracts::*;

require(cond, "mensaje");
ensure(cond, "mensaje");
invariant(cond, "mensaje");
require_valid_ident(nombre, 64, "identificador inválido");
require_safe_path(ruta, "ruta insegura");
```

También incluye `require_non_null`, `require_non_empty`, `require_index`,
`require_range`, `require_ok`, `require_some` y las operaciones comprobadas
`checked_add/sub/mul/div`.

---

## 7. Runtime C (`silc-runtime` / `sil-rt`)

El runtime se compila y enlaza con el código C generado. Familias principales:

| Familia | Ejemplos |
|---------|----------|
| Arenas | `sil_arena_crear`, `sil_arena_asignar`, `sil_arena_reset`, `sil_arena_ciclo_*` |
| Fibras | `sil_sched_crear`, `sil_sched_spawn`, `sil_sched_ejecutar`, `sil_fibra_yield` |
| Canales | `sil_channel_crear/enviar/recibir/cerrar/contar` |
| Canales MPSC | `sil_channel_mpsc_crear/enviar/recibir/cerrar` |
| E/S asíncrona | `sil_aio_ctx_crear`, `sil_aio_leer`, `sil_aio_escribir`, `sil_aio_procesar` |
| eBPF (Linux) | `sil_ebpf_ctx_crear`, `sil_ebpf_ctx_destruir` |

Las firmas exactas están en `silc-core/silc-runtime/src/lib.rs`.

---

## 8. LSP

Protocolo LSP 3.17 sobre JSON-RPC 2.0 con framing `Content-Length`.

Métodos implementados:

- `initialize`, `initialized`, `shutdown`, `exit`
- `textDocument/didOpen`, `didChange`, `didClose`
- `textDocument/publishDiagnostics` (errores léxicos y sintácticos)
- `textDocument/completion` (palabras clave y tipos primitivos)

Pendiente: hover, definición, formateador, debugger.
