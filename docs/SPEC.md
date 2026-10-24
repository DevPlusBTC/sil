# SIL Language Specification (Implemented v0.1.0)

**Versión:** 0.1.0 | **Fecha:** 2026-10-04 | **Estado:** Beta Funcional

---

## 1. Resumen de Implementación Actual

Este documento describe **qué está implementado y funcional** en SIL v0.1.0 vs la especificación completa del [WHITE_PAPER.md](WHITE_PAPER.md).

| Componente | Implementado | Notas |
|------------|--------------|-------|
| Frontend (Lexer/Parser CNL) | Listo para probar | Crate del workspace |
| Causal-IR + SMT | Listo para probar | Intervalos + Z3 |
| Backend C99 | Listo para probar | Emite C99; los tests lo compilan con gcc/clang `-Wall -Wextra -Werror` en Linux/macOS |
| Backend WASM | Listo para probar | Magic `\0asm` válido |
| Runtime FFI | Listo para probar | Arena O(1) + sub-arenas cíclicas (16 niveles), cap. HMAC, TPM 2.0, fibras, canales |
| CLI (`silc`) | Listo para probar | 5 comandos: check, build, emit-smt, run, lsp |
| LSP Server | Listo para probar | `silc lsp`: JSON-RPC sobre stdio, diagnósticos y completion |
| VS Code Extension | Listo para probar | Cliente LSP + comandos `sil.compile`/`sil.verify`; falta `.vsix` |
| Tests | Listo para probar | 163 Rust + 31 Python |

**Pendiente para desarrollo:** Codegen de servicios/red, arenas persistentes NVM,
migración de esquemas lazy, SPIR-V/GPU, eBPF (opt-in `SIL_EBPF=1`; la rama
libbpf 0.x no compila con libbpf 1.x), M:N scheduler, superoptimizer,
certificaciones militares.

---

## 2. Gramática SIL Implementada (Subconjunto CNL)

```ebnf
Programa        ::= { Declaracion } ;
Declaracion     ::= Tarea | Estructura | Variante | Servicio
                  | Modelo | MemoriaPersistente | Capacidad | Modulo | Migracion ;
Tarea           ::= "definir" "tarea" Ident "(" [ Parametros ] ")" [ "->" Tipo ] ":" NL INDENT
                    Cuerpo DEDENT ;
Parametros      ::= Ident ":" Tipo { "," Ident ":" Tipo } ;
Cuerpo          ::= { Sentencia } [ BloqueRestricciones ] ;
Sentencia       ::= "verificar" [ "que" ] ExpresionLogica
                  | "asumir" [ "que" ] ExpresionLogica
                  | "demostrar" [ "que" ] ExpresionLogica
                  | "let" [ "mut" ] Ident [ ":" Tipo ] "=" Expresion
                  | "retornar" [ Expresion ]
                  | "solicitar" "capacidad" Permiso "para" TEXTO
                    [ "con" "ttl" NUM ] [ "como" Ident ]
                  | "validar" Ident
                  | "crear" "canal" [ "con" "capacidad" NUM ] "como" Ident
                  | "enviar" Expresion "en" Ident
                  | "enviar" Expresion "en" "red" TEXTO "con" Ident
                  | "recibir" "de" Ident "como" Ident
                  | "recibir" "de" "red" TEXTO "con" Ident "como" Ident
                  | "generar" Ident
                  | "leer" "archivo" TEXTO "con" Ident "como" Ident
                  | "escribir" Expresion "en" "archivo" TEXTO "con" Ident ;
ExpresionLogica ::= Expresion OperadorRelacional Expresion
                  | Ident ;
OperadorRelacional ::= ">" | "<" | ">=" | "<=" | "==" | "!=" ;
Expresion       ::= Ident | Literal | Expresion OperadorAritmetico Expresion ;
OperadorAritmetico ::= "+" | "-" | "*" | "/" ;
Literal         ::= NUM | TEXTO | "verdadero" | "falso" ;
Tipo            ::= "Entero64" | "Flotante64" | "Booleano" | "Texto"
                  | "USD" | "EUR" | "CapacidadHardware" | "Void"
                  | "Lista" "de" Tipo | "Mapa" "de" Tipo "a" Tipo
                  | "Conjunto" "de" Tipo | "Tupla" "(" [ Tipo { "," Tipo } ] ")" ;
Permiso         ::= "lectura_archivo" | "escritura_archivo" | "red" | "ejecucion" ;
BloqueRestricciones ::= "bajo" "restricciones" ":" NL INDENT { Restriccion } DEDENT ;
Restriccion     ::= Ident ":" Ident ;
```

**Restricciones soportadas:** clave y valor son identificadores de un solo token
(p. ej. `gestion_memoria: arena`, `tiempo_ejecucion: constante`,
`latencia_maxima: baja`). Unidades compuestas (`200us`) todavía no se aceptan.

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
3. Resolución con Z3 (por defecto en el CLI) o con verificación de intervalos
   (`--no-default-features`), con caché SHA3-512
4. **UNSAT** → Compilación OK | **SAT** → Contraejemplo + Error

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

**Códigos de salida:** 0=OK, 1=Error (sintaxis, SMT o E/S)

---

## 6. LSP Server (`silc lsp`)

**Protocolo:** LSP 3.17 (JSON-RPC 2.0 sobre stdio, framing `Content-Length`)

**Capacidades implementadas:**
- `textDocument/didOpen` / `didChange` / `didClose` → sincronización completa y diagnósticos léxicos y de sintaxis del frontend real
- `textDocument/completion` → palabras clave y tipos primitivos
- `textDocument/publishDiagnostics` → mensajes del frontend en lenguaje natural
- `initialize`, `initialized`, `shutdown`, `exit`

**Iniciar:** `silc lsp` (stdio; los logs van a stderr)

**Pendiente:** hover, definición, formateador, Time-Travel Debugger.

---

## 7. VS Code Extension (`silc-vscode` v0.1.0)

**Instalación:** `npm install && npm run compile`, luego abrir la carpeta del repositorio como extensión (F5). Requiere `silc` en el `PATH`.

**Features:**
- Cliente LSP contra `silc lsp`: diagnósticos en tiempo real y completion de palabras clave/tipos
- Comando `SIL: Compilar archivo actual a C99` (`sil.compile`)
- Comando `SIL: Verificar invariantes SMT del archivo actual` (`sil.verify`)

**Pendiente:** hover, formateador, empaquetado `.vsix`.

---

## 7. Limitaciones Conocidas v0.1.0

| Limitación | Workaround |
|------------|------------|
| La última tarea con parámetros se emite como biblioteca (sin `main`) | Añadir una tarea principal sin parámetros al final |
| `generar` en C99 requiere tarea principal sin parámetros | Usar WASM (llamada directa) o una principal sin parámetros |
| SMT timeout 5s por defecto en postcondiciones complejas | Ajustar `--smt-timeout`; simplificar `demostrar` |
| Sin stdlib/prelude | Definir tipos base en cada archivo |
| Sin `silpm` / `silfmt` / `silup` | Usar `cargo build` + `silc` directo |
| LSP sin hover, definición ni formateador | Usar completion y diagnósticos |

---

## 8. Versionado y Compatibilidad

- **SemVer:** v0.1.0 = Beta (breaking changes esperados)
- **MSRV:** 1.88.0 (mínimo declarado por las dependencias); CI usa 1.97.1
- **Targets:** x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu, aarch64-apple-darwin (CI)
- **Publicación:** 4 crates en crates.io (`silc-frontend`, `silc-causal-ir`, `silc-backend`, `silc-runtime` v0.1.0); `silc-cli` y `silc-contracts` se instalan desde el repositorio

---

*Ver [WHITE_PAPER.md](WHITE_PAPER.md) para especificación completa aspiracional (EAL7, TPM, eBPF, GPU, etc.)*