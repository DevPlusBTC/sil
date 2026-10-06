# SIL Language Specification (Implemented v0.1.0)

**Versión:** 0.1.0 | **Fecha:** 2026-10-04 | **Estado:** Beta Funcional

---

## 1. Resumen de Implementación Actual

Este documento describe **qué está implementado y funcional** en SIL v0.1.0 vs la especificación completa del [WHITE_PAPER.md](WHITE_PAPER.md).

| Componente | Implementado | Notas |
|------------|--------------|-------|
| Frontend (Lexer/Parser CNL) | ✅ 95% | `silc-frontend` publicado |
| Causal-IR + SMT | ✅ 90% | `silc-causal-ir` publicado, Z3 incremental |
| Backend C99 | ✅ 85% | `silc-backend` publicado, emite C99 válido |
| Backend WASM | ✅ 80% | `silc-backend` publicado, magic `\0asm` válido |
| Runtime FFI | ✅ 70% | `silc-runtime` publicado, arena O(1) básica |
| CLI (`silc`) | ✅ 95% | 4 comandos: check, build, emit-smt |
| LSP Server | ✅ 85% | JSON-RPC 3.17, diagnostics, completion, hover, TTD |
| VS Code Extension | ✅ 80% | Diagnostics, completion, hover, toggle |
| Tests Integración | ✅ 100% | 7/7 passing |

**NO implementado aún (ver WHITE_PAPER.md):** Arenas persistentes NVM, Capabilities TPM, eBPF kernel, SPIR-V/GPU, M:N scheduler, migración esquemas lazy, superoptimizer, certificaciones militares.

---

## 2. Gramática SIL Implementada (Subconjunto CNL)

```ebnf
Programa        ::= { DefinicionTarea } ;
DefinicionTarea ::= "definir" "tarea" Identificador "(" [ ListaParametros ] ")" [ "->" TipoDato ] ":" NL INDENT
                    CuerpoTarea DEDENT ;
ListaParametros ::= Identificador [ ":" TipoDato ] { "," Identificador [ ":" TipoDato ] } ;
CuerpoTarea     ::= { Sentencia } [ BloqueRestricciones ] ;
Sentencia       ::= "asumir" ExpresionLogica
                  | "demostrar" ExpresionLogica
                  | "retornar" Expresion ;
ExpresionLogica ::= Expresion OperadorRelacional Expresion
                  | Identificador ;
OperadorRelacional ::= ">" | "<" | ">=" | "<=" | "==" | "!=" ;
Expresion       ::= Identificador | Literal | Expresion OperadorAritmetico Expresion ;
OperadorAritmetico ::= "+" | "-" | "*" | "/" ;
Literal         ::= NUM ;
TipoDato        ::= "Entero64" | "Flotante64" | "Booleano" | "Texto" ;
BloqueRestricciones ::= "bajo" "restricciones" ":" NL INDENT { Restriccion } DEDENT ;
Restriccion     ::= Identificador ":" ValorRestriccion ;
```

**Restricciones soportadas:**
- `latencia_maxima: NUMms`
- `gestion_memoria: arena`
- `concurrencia: modelo_actores_masivo`
- `seguridad_tipo: estricta`

---

## 3. Semántica de Contratos (SMT)

### 3.1 `asumir` / `demostrar`
```sil
definir tarea f(x: Entero64) -> Entero64:
    asumir x > 0           // Premisa (precondición)
    demostrar x > 0        // Meta (postcondición)
    retornar x
```

### 3.2 Pipeline SMT
1. Extracción cláusulas: `asumir` → premisas, `demostrar` → metas
2. Traducción SMT-LIB2: `assert (P ∧ ¬Q)`
3. Resolución Z3/CVC5 incremental con caché SHA3-512
3. **UNSAT** → Compilación OK | **SAT** → Contraejemplo + Error

### 3.3 Tipos de Refinamiento
```sil
definir tarea descuento(precio: Entero64, pct: Entero64) -> Entero64:
    asumir precio > 0
    asumir pct >= 0
    asumir pct <= 50
    let final = precio * (100 - pct) / 100
    demostrar final <= precio
    demostrar final >= precio / 2
    retornar final
```
Si `pct = 60` → Error: `SMT SAT: contraejemplo pct=60 rompe final >= precio/2`

---

## 4. Backends de Compilación

### 4.1 Target C99 (`--target c99`)
```bash
silc build prog.sil --target c99 -o prog.c
```
- Emite C99 estricto (`-std=c99 -Wall -Werror`)
- Runtime embebido: `sil_arena_*`, tipos `SilTexto_`, `SilCap_`, `Entero64`
- Compilable: GCC, Clang, MSVC
- Salida: ~2.3KB para tarea simple

### 4.2 Target WASM (`--target wasm`)
```bash
silc build prog.sil --target wasm -o prog.wasm
```
- Emite WASM válido (magic `\0asm`)
- Validador `wasmparser` integrado en tests
- Sin dependencias WASI (standalone)
- Salida: ~158 bytes para tarea simple

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

**Códigos de salida:** 0=OK, 1=Error sintaxis/SMT, 2=Error I/O

---

## 6. LSP Server (`silc-lsp`)

**Protocolo:** JSON-RPC 3.17 sobre stdio

**Capabilities:**
- `textDocument/didOpen` + `didChange` → Diagnósticos incrementales (2ms)
- `textDocument/completion` → Trigger `.` → Variables, tareas, tipos
- `textDocument/hover` → Tipo inferido, contratos, invariantes
- `textDocument/publishDiagnostics` → Errores en lenguaje natural
- **Time-Travel Debugger** (core): Navegación grafo causal semántico

**Iniciar:** `silc-lsp` (stdio) → Configurar en VS Code `settings.json`

---

## 7. VS Code Extension (`silc-vscode` v0.1.0)

**Instalación:** `.vsix` o marketplace

**Features:**
- Diagnostics en tiempo real (subrayado rojo/amarillo)
- Completion trigger `.` (variables, tareas, tipos)
- Hover → Tipo inferido + contratos
- Comando: `SIL: Toggle Diagnostics` (Ctrl+Shift+P)

---

## 7. Limitaciones Conocidas v0.1.0

| Limitación | Workaround |
|------------|------------|
| `build` solo 1 tarea por archivo | Usar 1 archivo = 1 tarea; `check` soporta multi-tarea |
| SMT timeout 5s en postcondiciones complejas | Simplificar `demostrar`; dividir en tareas menores |
| Sin stdlib/prelude | Definir tipos base en cada archivo |
| Solo Linux/Windows x64 probado | macOS/ARM no validado |
| Sin `silpm` / `silfmt` / `silup` | Usar `cargo build` + `silc` directo |

---

## 8. Versionado y Compatibilidad

- **SemVer:** v0.1.0 = Beta (breaking changes esperados)
- **MSRV:** Rust 1.75.0
- **Targets:** x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu
- **Publicación:** 4 crates en crates.io (`silc-frontend`, `silc-causal-ir`, `silc-backend`, `silc-runtime` v0.1.0)

---

*Ver [WHITE_PAPER.md](WHITE_PAPER.md) para especificación completa aspiracional (EAL7, TPM, eBPF, GPU, etc.)*