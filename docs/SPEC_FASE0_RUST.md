# SPEC_FASE0_RUST.md — Especificación de Diseño: Compilador Semilla SIL en Rust + LLVM

**Versión:** 1.0.0-DRAFT
**Estado:** Diseño técnico para implementación
**Referencia:** `docs/WHITE_PAPER.md` §11 (Fase 0: Compilador Semilla)
**Objetivo:** Binario `silc` nativo, reproducible, que valida CNL y genera código máquina optimizado

---

## 1. Arquitectura General (Workspace Cargo)

```
silc-core/                      # Workspace raíz
├── Cargo.toml                  # Workspace manifest
├── build.rs                    # Build-time: constantes SMT, verificación spec, codegen helpers
├── rust-toolchain.toml         # Pin toolchain para reproducibilidad
├── Cargo.lock                  # Commiteado: reproducibilidad bit-for-bit
├── deny.toml                   # cargo-deny: auditoría supply-chain
│
├── silc-cli/                   # Binary: CLI silc
│   ├── Cargo.toml
│   └── src/main.rs             # clap-based: build/check/run/test/fmt/lsp
│
├── silc-frontend/              # Crate: Lexer + Parser + AST
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── lexer.rs            # logos 0.15
│       ├── parser.rs           # chumsky 0.9 (parser combinators)
│       ├── ast.rs              # AST tipado con spans (miette)
│       └── error.rs            # Diagnósticos humanos (miette + thiserror)
│
├── silc-causal-ir/             # Crate: Causal-IR (SSA) + SMT Pipeline
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── nodes.rs            # Definición nodos IR
│       ├── lower.rs            # AST → Causal-IR lowering
│       ├── smt.rs              # z3 0.12: SMT-LIB2 gen + caché incremental
│       └── verify.rs           # Orquestación: assume/prove → check-sat
│
├── silc-backend/               # Crate: Emisión código nativo
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── llvm.rs             # inkwell 0.4 (LLVM 17): IR emission
│       ├── c99.rs              # Emisión C99 portable (fallback, sin LLVM)
│       ├── wasm.rs             # Emisión WAT/WASM (wasm-encoder)
│       └── targets.rs          # x86_64 / aarch64 / riscv64 / wasm32 / spirv
│
├── silc-runtime/               # Crate: FFI seguro al runtime C
│   ├── Cargo.toml
│   └── src/lib.rs              # Bindings seguros (no unsafe público)
│
├── sil-rt/                     # Runtime C (compilado por build.rs vía cc)
│   ├── include/
│   │   ├── sil_arena.h         # API Arenas O(1)
│   │   ├── sil_fiber.h         # API Fibras 64B + scheduler
│   │   └── sil_cap.h           # API Capabilities TPM/eBPF
│   └── src/
│       ├── arena.c             # Bump allocator, sub-arenas, hoisting
│       ├── fiber.c             # Context switch (asm), M:N scheduler
│       └── cap.c               # Validación tokens (stub TPM, listo para driver real)
│
└── silc-test/                  # Crate: Tests integración (insta snapshots)
    ├── Cargo.toml
    └── tests/
        ├── lexer_snapshots.rs
        ├── parser_snapshots.rs
        ├── causal_ir_snapshots.rs
        └── e2e_build.rs        # Compila .sil → binario → ejecuta → assert output
```

---

## 2. Crates Exactas y Versiones (MSRV: 1.75.0)

### 2.1 CLI y UX

| Crate | Versión | Uso | Justificación |
|-------|---------|-----|---------------|
| `clap` | 4.5 (derive) | CLI `silc build/check/run/test/fmt/lsp` | Estándar de facto, completions shell |
| `miette` | 7.0 | Diagnósticos con spans + colores + sugerencias | Errores humanos CNL (§10.1 Whitepaper) |
| `thiserror` | 2.0 | Definición errores tipados | Ergonomía + `#[from]` chains |
| `tracing` | 0.1 + `tracing-subscriber` 0.3 | Logging estructurado (JSON para CI) | Observabilidad compilador |
| `indicatif` | 0.17 | Progress bars para builds largos | DX |
| `directories` | 5.0 | Rutas XDG (`~/.sil`, caché SMT) | Multi-OS |

### 2.2 Frontend (Lexer + Parser)

| Crate | Versión | Uso | Justificación |
|-------|---------|-----|---------------|
| `logos` | 0.15 | Lexer determinista, longest-match, sin backtracking | Velocidad (SIMD), `#[logos(error)]` custom, callbacks para indentación |
| `chumsky` | 0.9 | Parser combinators con recuperación de errores | Mejor que LALRPOP para CNL (errores humanos, spans precisos), `Parser::recover_with` |
| `ariadne` | 0.4 (vía chumsky) | Reportes de error con múltiples spans | Integrado con miette |

**Alternativa descartada:** `lalrpop 0.22` — más rápido en parse puro, pero mensajes de error peores y gramática LALR(1) menos expresiva para CNL con indentación significativa. `chumsky` permite `indentation-sensitive parsing` con `Parser::delimited_by` + custom `INDENT`/`DEDENT` emitidos por el lexer.

### 2.3 Causal-IR + SMT

| Crate | Versión | Uso | Justificación |
|-------|---------|-----|---------------|
| `z3` | 0.12 | SMT Solver (solver incremental, push/pop) | Bindings oficiales, `Solver::push/pop`, `Model::eval`. Requiere libz3-sys (vendored por defecto) |
| `sha3` | 0.10 | SHA3-512 para hash causal AST | Caché incremental (§3.4 Whitepaper) |
| `blake3` | 1.5 | Hash rápido alternativo para caché local (no criptográfico) | Velocidad caché en memoria |
| `petgraph` | 0.6 | Grafo Causal Semántico (SCG): dependencias, lifetimes | Análisis escape causal, topological sort para lowering |
| `slotmap` | 1.0 | IDs estables para nodos IR (no índices frágiles) | SSA values, arenas de nodos |
| `rustc-hash` | 2.0 | FxHashMap para tablas símbolos (más rápido que SipHash) | Hot path compilador |

### 2.4 Backend (Codegen)

| Crate | Versión | Uso | Justificación |
|-------|---------|-----|---------------|
| `inkwell` | 0.4 (LLVM 17) | Emisión LLVM IR, optimizaciones, object emission | Bindings seguros a LLVM-C. Features: `llvm17-0`, `target-all` |
| `wasm-encoder` | 0.202 | Emisión WASM binario directo (sin LLVM) | Más control que vía LLVM para WASI, output determinista |
| `wasmparser` | 0.202 | Validación WASM generado (round-trip check) | Garantía output válido |
| `object` | 0.36 | Inspección binarios generados (tests) | Verificar secciones, símbolos |
| `gimli` | 0.29 | DWARF debug info (futuro: Time-Travel Debugger) | Base para debugger causal |

**Nota LLVM 17:** Pin exacto vía `llvm-sys 170.*`. CI instala `llvm-17-dev` (apt) / `llvm@17` (brew) / `LLVM-17` (choco). `inkwell` con feature `target-x86_64,target-aarch64,target-riscv64,target-wasm32`.

### 2.5 Runtime FFI

| Crate | Versión | Uso |
|-------|---------|-----|
| `cc` | 1.1 | Compilar `sil-rt/*.c` en `build.rs` con flags deterministas (`-std=c99 -O2 -ffile-prefix-map`) |
| `bindgen` | 0.69 | Generar bindings desde `sil-rt/include/*.h` (solo en build, no commiteado) |
| `libc` | 0.2 | Tipos C (`c_int`, `size_t`) en FFI boundaries |

### 2.6 Tests y Calidad

| Crate | Versión | Uso |
|-------|---------|-----|
| `insta` | 1.38 | Snapshot testing (lexer tokens, AST JSON, Causal-IR, LLVM IR, C99, WAT) |
| `proptest` | 1.5 | Property-based testing (parser round-trip, arena allocator invariants) |
| `criterion` | 0.5 | Benchmarks (lexer throughput, parse time, SMT cache hit rate) |
| `assert_cmd` | 2.0 | Tests CLI end-to-end (invoca `silc` binario) |
| `predicates` | 3.1 | Aserciones sobre output CLI |

### 2.7 Seguridad Supply-Chain

| Herramienta | Uso |
|-------------|-----|
| `cargo-deny` | Licencias (solo MIT/Apache-2.0), advisories (RUSTSEC), ban duplicados |
| `cargo-audit` | RUSTSEC vulnerability scan en CI |
| `cargo-vet` | *Opcional futuro:* auditoría manual dependencias (estilo Mozilla) |

```toml
# deny.toml (resumen)
[licenses]
allow = ["MIT", "Apache-2.0", "Unicode-DFS-2016", "LLVM-exception"]
deny = ["GPL-3.0", "AGPL-3.0"]

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
```

---

## 3. Mapeo EBNF → Parser Combinators (chumsky)

### 3.1 Estrategia General

- **Lexer (`logos`)** emite: `KEYWORD`, `IDENT`, `INT`, `FLOAT`, `STRING`, `SYMBOL`, `INDENT`, `DEDENT`, `NEWLINE`, `EOF`.
- **Indentación:** El lexer mantiene `indent_stack: Vec<usize>`. Al encontrar `\n + espacios`:
  - Si `n > top` → emite `NEWLINE` + `INDENT`, push `n`.
  - Si `n < top` → emite `NEWLINE` + `DEDENT` × (pops hasta `n`). Error si `n` no está en stack.
  - Si `n == top` → emite `NEWLINE`.
- **Parser (`chumsky`)** consume stream de tokens con `Parser::map_with_span` para preservar `Span` en cada nodo AST.
- **Recuperación:** `Parser::recover_with(nested_delimiters(...))` en bloques `INDENT...DEDENT` para múltiples errores por archivo.

### 3.2 Definiciones Chumsky (Pseudocódigo → Real)

```rust
// silc-frontend/src/parser.rs

use chumsky::prelude::*;
use crate::{lexer::Token, ast::*};

// Tipo alias para parser con span y error rico
type P<'a, T> = impl Parser<'a, &'a [Spanned<Token>], T, extra::Err<Rich<'a, Token, Span>>>;

// --- Helpers léxicos ---
fn kw<'a>(s: &'static str) -> P<'a, ()> {
    select! { Token::Keyword(k) if k == s => () }
}

fn ident<'a>() -> P<'a, Ident> {
    select! { Token::Ident(name) => Ident(name) }.map_with_span(|n, s| Spanned(n, s))
}

fn newline<'a>() -> P<'a, ()> {
    select! { Token::Newline => () }
}

// --- Programa ---
// Programa ::= { Declaracion }
pub fn programa<'a>() -> P<'a, Program> {
    declaracion()
        .repeated()
        .collect::<Vec<_>>()
        .map(|defs| Program { defs })
}

// --- Declaraciones ---
// Declaracion ::= DefinicionServicio | DefinicionModelo | DefinicionTarea | ...
fn declaracion<'a>() -> P<'a, Decl> {
    choice((
        definicion_servicio().map(Decl::Servicio),
        definicion_tarea().map(Decl::Tarea),
        definicion_estructura().map(Decl::Estructura),
        definicion_variante().map(Decl::Variante),
        definicion_memoria().map(Decl::MemoriaPersistente),
        definicion_capacidad().map(Decl::Capacidad),
    ))
}

// --- Definición de Tarea ---
// DefinicionTarea ::= "definir" "tarea" Identificador "(" [ ListaParametros ] ")" [ "->" TipoDato ] ":" NL INDENT CuerpoIntencion DEDENT
fn definicion_tarea<'a>() -> P<'a, TareaDecl> {
    kw("definir")
        .then(kw("tarea"))
        .then(ident())
        .then(
            just(Token::Symbol(Symbol::LParen))
                .ignore_then(
                    lista_parametros().or_not()
                )
                .then_ignore(just(Token::Symbol(Symbol::RParen)))
        )
        .then(
            // [ "->" TipoDato ]
            just(Token::Symbol(Symbol::Arrow))
                .ignore_then(tipo_dato())
                .or_not()
        )
        .then_ignore(just(Token::Symbol(Symbol::Colon)))
        .then_ignore(newline())
        .then_ignore(just(Token::Indent))
        .then(cuerpo_intencion())
        .then_ignore(just(Token::Dedent))
        .map(|((((nombre, params), ret), cuerpo))| TareaDecl {
            nombre,
            params: params.unwrap_or_default(),
            retorno: ret,
            cuerpo,
            span: Span::default(), // Rellenado por map_with_span en wrapper
        })
        .map_with_span(|t, s| t.with_span(s))
}

// --- Cuerpo de Intención ---
// CuerpoIntencion ::= Sentencia { NL Sentencia } [ NL BloqueRestricciones ]
fn cuerpo_intencion<'a>() -> P<'a, Cuerpo> {
    sentencia()
        .separated_by(newline())
        .allow_trailing()
        .collect::<Vec<_>>()
        .then(
            newline()
                .ignore_then(bloque_restricciones())
                .or_not()
        )
        .map(|(stmts, restr)| Cuerpo { stmts, restricciones: restr.unwrap_or_default() })
}

// --- Sentencias CNL ---
fn sentencia<'a>() -> P<'a, Stmt> {
    choice((
        sentencia_validacion(),
        sentencia_asuncion(),
        sentencia_demostracion(),
        sentencia_asignacion(),
        sentencia_retorno(),
        sentencia_coincidencia(),
        sentencia_efecto(),
    ))
}

// SentenciaValidacion ::= "verificar" "que" ExpresionLogica
fn sentencia_validacion<'a>() -> P<'a, Stmt> {
    kw("verificar")
        .ignore_then(kw("que"))
        .ignore_then(expresion_logica())
        .map(Stmt::Verificar)
        .map_with_span(|s, span| s.with_span(span))
}

// SentenciaAsignacion ::= "let" [ "mut" ] Identificador [ ":" TipoDato ] "=" Expresion
fn sentencia_asignacion<'a>() -> P<'a, Stmt> {
    kw("let")
        .ignore_then(kw("mut").or_not())
        .then(ident())
        .then(
            just(Token::Symbol(Symbol::Colon))
                .ignore_then(tipo_dato())
                .or_not()
        )
        .then_ignore(just(Token::Symbol(Symbol::Eq)))
        .then(expresion())
        .map(|(((is_mut, nombre), tipo), expr)| Stmt::Asignar {
            nombre, tipo, expr, mutable: is_mut.is_some(),
        })
}

// --- Expresiones (con precedencia) ---
// Expresion ::= OrExpr
// OrExpr    ::= AndExpr { "o" AndExpr }
// AndExpr   ::= CmpExpr { "y" CmpExpr }
// CmpExpr   ::= AddExpr [ OperadorRelacional AddExpr ]
// AddExpr   ::= MulExpr { ("+" | "-") MulExpr }
// MulExpr   ::= Unary { ("*" | "/") Unary }
// Unary     ::= [ "-" ] Primary
// Primary   ::= Literal | Identificador [ "." Identificador ] | "(" Expresion ")"
fn expresion<'a>() -> P<'a, Expr> {
    recursive(|expr| {
        let primary = choice((
            literal().map(Expr::Lit),
            ident().then(
                just(Token::Symbol(Symbol::Dot))
                    .ignore_then(ident())
                    .or_not()
            ).map(|(base, prop)| match prop {
                Some(p) => Expr::AccesoProp { base: Box::new(Expr::Var(base)), prop: p },
                None => Expr::Var(base),
            }),
            expr.clone()
                .delimited_by(
                    just(Token::Symbol(Symbol::LParen)),
                    just(Token::Symbol(Symbol::RParen)),
                ),
        ));

        // ... (niveles de precedencia con foldl para asociatividad izquierda)
        // Por brevedad: mostrar solo nivel comparación
        let cmp = primary.clone().then(
            operador_relacional()
                .then(primary)
                .or_not()
        ).map(|(l, r)| match r {
            Some((op, rhs)) => Expr::BinOp { op, lhs: Box::new(l), rhs: Box::new(rhs) },
            None => l,
        });

        cmp // En implementación real: encadenar todos los niveles
    })
}

// OperadorRelacional ::= "sea" "mayor" "a" | "sea" "menor" "a" | "sea" "igual" "a" | "sea" | "esta_activo" | ">" | "<" | "==" | ...
fn operador_relacional<'a>() -> P<'a, BinOp> {
    choice((
        kw("sea").ignore_then(kw("mayor")).ignore_then(kw("a")).to(BinOp::Gt),
        kw("sea").ignore_then(kw("menor")).ignore_then(kw("a")).to(BinOp::Lt),
        kw("sea").ignore_then(kw("igual")).ignore_then(kw("a")).to(BinOp::Eq),
        // ... resto de formas CNL + símbolos directos >, <, ==, etc.
        just(Token::Symbol(Symbol::Gt)).to(BinOp::Gt),
        just(Token::Symbol(Symbol::Lt)).to(BinOp::Lt),
        just(Token::Symbol(Symbol::EqEq)).to(BinOp::Eq),
    ))
}

// --- Pattern Matching ---
// SentenciaCoincidencia ::= "coincidir" Expresion ":" NL INDENT ClausulaCaso { NL ClausulaCaso } [ NL ClausulaPredeterminada ] DEDENT
fn sentencia_coincidencia<'a>() -> P<'a, Stmt> {
    kw("coincidir")
        .ignore_then(expresion())
        .then_ignore(just(Token::Symbol(Symbol::Colon)))
        .then_ignore(newline())
        .then_ignore(just(Token::Indent))
        .then(
            clausula_caso()
                .separated_by(newline())
                .at_least(1)
                .collect::<Vec<_>>()
                .then(
                    newline().ignore_then(clausula_default()).or_not()
                )
        )
        .then_ignore(just(Token::Dedent))
        .map(|(scrut, (casos, def))| Stmt::Coincidir { scrut: Box::new(scrut), casos, defecto: def.map(Box::new) })
}
```

### 3.3 Tabla de Correspondencia EBNF → Chumsky

| Producción EBNF | Combinador chumsky | Notas |
|-----------------|-------------------|-------|
| `{ X }` (repetición) | `x.repeated().collect::<Vec<_>>()` | |
| `[ X ]` (opcional) | `x.or_not()` | Retorna `Option<T>` |
| `A \| B` (alternativa) | `choice((a, b))` | Orden = prioridad (PEG). Poner formas largas CNL antes que cortas |
| `"literal"` | `kw("literal")` o `just(Token::Symbol(...))` | `kw` para keywords, `just` para símbolos |
| `A B` (secuencia) | `a.then(b)` o `a.ignore_then(b)` / `a.then_ignore(b)` | `ignore_then` descarta LHS, `then_ignore` descarta RHS |
| `X { "," X }` | `x.separated_by(just(comma)).collect()` | Con `.allow_trailing()` si se permite coma final |
| Recursión (`Expr → ... → Expr`) | `recursive(\|expr\| { ... expr.clone() ... })` | Necesario para expresiones con paréntesis |
| Precedencia operadores | Niveles anidados con `.foldl()` | Asociatividad izquierda correcta |

---

## 4. Definición Nodos Causal-IR (Rust)

### 4.1 Tipos Fundamentales

```rust
// silc-causal-ir/src/nodes.rs

use slotmap::{SlotMap, new_key_type};
use std::collections::HashMap;

new_key_type! {
    /// ID estable para valores SSA (no índices frágiles).
    pub struct ValueId;
    /// ID estable para bloques básicos.
    pub struct BlockId;
    /// ID estable para arenas.
    pub struct ArenaId;
}

// --- Tipos SIL ---
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SilType {
    Entero64,
    Flotante64,
    Booleano,
    Texto,
    CapacidadHardware,
    Void,
    Lista(Box<SilType>),
    Mapa(Box<SilType>, Box<SilType>),
    Conjunto(Box<SilType>),
    Tupla(Vec<SilType>),
    USD, EUR,
    Tensor(Box<SilType>, Vec<u64>),  // dtype + shape
    Nominal(String),                  // Estructuras/variantes definidas por usuario
}

// --- Capacidades (Zero-Trust) ---
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapacidadReq {
    /// Permiso requerido: lectura archivo, red, exec, etc.
    pub permiso: Permiso,
    /// TTL en nanosegundos (ej. 500_000 = 500µs).
    pub ttl_ns: u64,
    /// Recurso específico (dominio, path, etc.). None = genérico.
    pub recurso: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permiso {
    LecturaArchivo,
    EscrituraArchivo,
    Red,
    Ejecucion,
}

// --- Invariantes SMT ---
#[derive(Debug, Clone)]
pub struct InvarianteSMT {
    /// Fórmula en AST lógico (no string) para traducción robusta a SMT-LIB2.
    pub formula: FormulaLogica,
    /// Hash causal SHA3-512 para caché incremental.
    pub hash: [u8; 64],
    /// Origen: asumir (premisa) o demostrar/verificar (meta).
    pub clase: ClaseInvariante,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseInvariante {
    Asuncion,      // asumir P → (assert P)
    Demostracion,  // demostrar Q → (assert (not Q)) + check-sat, espera UNSAT
    Verificacion,  // verificar Q → igual que Demostracion (alias semántico)
}

// --- Fórmula Lógica (para SMT) ---
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FormulaLogica {
    Var(String),
    ConstInt(i64),
    ConstBool(bool),
    BinOp { op: OpLogico, lhs: Box<FormulaLogica>, rhs: Box<FormulaLogica> },
    No(Box<FormulaLogica>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpLogico { Gt, Lt, Eq, Ne, Ge, Le, Add, Sub, Mul, Div, And, Or }

// --- Instrucción Causal (cuádruplo ortogonal) ---
#[derive(Debug, Clone)]
pub struct InstrCausal {
    pub id: ValueId,                          // ID SSA del resultado (si aplica)
    pub op: Operacion,
    pub capacidad: Option<CapacidadReq>,       // Barrera anti-virus (None = pura)
    pub invariante: Option<InvarianteSMT>,     // Prueba anti-hackeo (None = sin prueba)
    pub span: Span,                            // Para diagnósticos
}

#[derive(Debug, Clone)]
pub enum Operacion {
    // --- Parámetros y constantes ---
    Param { nombre: String, tipo: SilType },
    Const { valor: Constante, tipo: SilType },

    // --- Memoria (Arenas O(1)) ---
    AsignarArena {
        arena: ArenaId,
        nombre: String,
        valor: ValueId,
    },
    Promover {  // Hoisting O(1): arena_origen → arena_destino
        valor: ValueId,
        destino: ArenaId,
    },

    // --- Aritmética (con flags nsw/nuw si SMT probó no-overflow) ---
    BinOpSegura {
        op: OpArit,
        lhs: ValueId,
        rhs: ValueId,
        sin_overflow: bool,  // true → emitir nsw/nuw en LLVM, omitir checks
    },

    // --- Control ---
    Retornar { valor: Option<ValueId> },
    Ramificar { cond: ValueId, entonces: BlockId, sino: BlockId },

    // --- Capacidades (syscalls mediadas por hardware) ---
    SolicitarCapacidad { req: CapacidadReq },
    ValidarCapacidad { token: ValueId },
    InvocarIO {
        operacion: OpIO,       // LeerArchivo, EscribirRed, etc.
        args: Vec<ValueId>,
        token: ValueId,        // Token firmado requerido
    },

    // --- Concurrencia ---
    CrearFibra { entrada: BlockId, arena: ArenaId },
    EnviarCanal { canal: ValueId, valor: ValueId },
    RecibirCanal { canal: ValueId },
}

// --- Bloque Básico ---
#[derive(Debug, Clone)]
pub struct Bloque {
    pub id: BlockId,
    pub instrs: Vec<InstrCausal>,
    pub terminador: Terminador,
}

// --- Función / Tarea ---
#[derive(Debug, Clone)]
pub struct TareaIR {
    pub nombre: String,
    pub params: Vec<(String, SilType)>,
    pub retorno: SilType,
    pub bloques: SlotMap<BlockId, Bloque>,
    pub entrada: BlockId,
    pub arenas: Vec<ArenaId>,
    pub restricciones: Vec<Restriccion>,
}
```

### 4.2 Lowering AST → Causal-IR

```rust
// silc-causal-ir/src/lower.rs

pub struct Lowerer {
    valores: SlotMap<ValueId, (SilType, Span)>,
    bloques: SlotMap<BlockId, Bloque>,
    actual: BlockId,
    arenas: ArenaStack,
}

impl Lowerer {
    /// Punto de entrada: TareaDecl AST → TareaIR.
    pub fn bajar_tarea(&mut self, tarea: &TareaDecl) -> Result<TareaIR, ErrorBajada> {
        // 1. Crear arena local de tarea.
        let arena_local = self.arenas.push(ArenaKind::TareaLocal);

        // 2. Emitir Nodos Param para cada parámetro.
        for (nombre, tipo) in &tarea.params {
            let id = self.nuevo_valor(tipo.clone());
            self.emitir(InstrCausal {
                op: Operacion::Param { nombre: nombre.clone(), tipo: tipo.clone() },
                ..Default::default()
            });
        }

        // 3. Bajar cada sentencia del cuerpo.
        for stmt in &tarea.cuerpo.stmts {
            self.bajar_sentencia(stmt, arena_local)?;
        }

        // 4. Detectar bucles → crear sub-arenas cíclicas automáticamente.
        self.insertar_subarenas_ciclicas()?;

        Ok(...)
    }

    /// `asumir P` → InvarianteSMT clase Asuncion + hash causal.
    /// `demostrar Q` → InvarianteSMT clase Demostracion + hash causal.
    fn bajar_invariante(&mut self, f: &FormulaLogica, clase: ClaseInvariante) -> InvarianteSMT {
        let hash = sha3_512(canonicalizar(f));
        InvarianteSMT { formula: f.clone(), hash, clase }
    }
}
```

---

## 5. Pipeline SMT (z3 crate + Caché Incremental)

### 5.1 Flujo de Verificación

```
Causal-IR (TareaIR con invariantes)
        │
        ▼
┌─────────────────────────┐
│ Extraer asunciones P_i  │ → Contexto Z3: solver.assert(P_i)
│ Extraer metas Q_j       │
└─────────────────────────┘
        │
        ▼
┌─────────────────────────┐
│ Para cada meta Q_j:     │
│  1. Calcular hash(Q_j)  │
│  2. ¿En caché global?   │
│     Sí → SKIP (válido)  │
│     No → solver.push()  │
│          solver.assert(!Q_j) │
│          check-sat      │
│          UNSAT → válido, guardar en caché │
│          SAT → extraer modelo = contraejemplo, ERROR │
│          solver.pop()   │
└─────────────────────────┘
```

### 5.2 Implementación Rust (z3 0.12)

```rust
// silc-causal-ir/src/smt.rs

use z3::{Config, Context, Solver, SatResult, ast::Bool};
use sha3::{Sha3_512, Digest};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct VerificadorSMT<'ctx> {
    ctx: &'ctx Context,
    solver: Solver<'ctx>,
    cache: CacheLemas,           // Hash → Resultado (persistente en disco)
    timeout_ms: u64,
}

impl<'ctx> VerificadorSMT<'ctx> {
    pub fn new(ctx: &'ctx Context, timeout_ms: u64) -> Self {
        let solver = Solver::new(ctx);
        // Timeout por query para evitar explosión combinatoria
        solver.set_param("timeout", &timeout_ms.to_string());
        Self { ctx, solver, cache: CacheLemas::cargar(), timeout_ms }
    }

    /// Verifica una tarea completa. Retorna Ok(()) si todas las metas son UNSAT.
    /// Retorna Err(Contraejemplo) con modelo concreto si alguna es SAT.
    pub fn verificar_tarea(&mut self, tarea: &TareaIR) -> Result<(), ErrorSMT> {
        // 1. Assert todas las asunciones
        for inv in tarea.invariantes(ClaseInvariante::Asuncion) {
            let b = self.traducir(&inv.formula)?;
            self.solver.assert(&b);
        }

        // 2. Para cada meta: push, assert negación, check-sat, pop
        for inv in tarea.invariantes(ClaseInvariante::Demostracion) {
            // Caché incremental: si hash ya probado → skip O(1)
            if self.cache.contiene(&inv.hash) {
                continue;
            }

            let q = self.traducir(&inv.formula)?;
            self.solver.push();
            self.solver.assert(&q.not());

            match self.solver.check() {
                SatResult::Unsat => {
                    // ✅ Meta válida para todo el espacio de entrada
                    self.cache.guardar(&inv.hash);
                    self.solver.pop(1);
                }
                SatResult::Sat => {
                    // ❌ Contraejemplo encontrado
                    let modelo = self.solver.get_model().unwrap();
                    let contra = self.extraer_contraejemplo(&modelo, &inv.formula);
                    self.solver.pop(1);
                    return Err(ErrorSMT::InvarianteViolada { invariante: inv.clone(), contraejemplo: contra });
                }
                SatResult::Unknown => {
                    self.solver.pop(1);
                    return Err(ErrorSMT::Timeout { hash: inv.hash });
                }
            }
        }
        Ok(())
    }

    /// Traduce FormulaLogica → z3::ast::Bool.
    fn traducir(&self, f: &FormulaLogica) -> Result<Bool<'ctx>, ErrorSMT> {
        // ... (mapeo directo: Var → Int::new_const, BinOp → .gt(), ._eq(), etc.)
    }
}

/// Caché persistente de lemas probados: ~/.sil/cache/lemas.blake3
/// Formato: bincode(HashMap<[u8;64], ()>) + fsync.
struct CacheLemas { /* ... */ }
```

### 5.3 Traducción FormulaLogica → SMT-LIB2 (para debug/`--emit-smt`)

```rust
impl FormulaLogica {
    pub fn a_smtlib2(&self) -> String {
        match self {
            FormulaLogica::BinOp { op, lhs, rhs } => {
                format!("({} {} {})", op.a_smtlib2(), lhs.a_smtlib2(), rhs.a_smtlib2())
            }
            // ...
        }
    }
}
```

---

## 6. Backend LLVM (inkwell 0.4 + LLVM 17)

### 6.1 Mapeo de Tipos SIL → LLVM

| Tipo SIL | Tipo LLVM (inkwell) | Atributos / Invariantes |
|----------|---------------------|------------------------|
| `Entero64` | `i64` | `nsw` si `sin_overflow=true` (probado por SMT). Si no, emitir `llvm.sadd.with.overflow` + trap |
| `Flotante64` | `double` | `fast` flags solo si restricciones permiten (`optimizacion: agresiva`) |
| `Booleano` | `i1` | |
| `Texto` | `{ i8*, i64 }` (struct) | `{ptr: noalias readonly, len}`. ptr apunta a Arena (no heap) |
| `CapacidadHardware` | `{ i64, i64, [32 x i8] }` | `{id, expiracion_ns, firma}`. Pasado por valor, nunca por puntero mutable |
| `ArenaHandle` | `i8*` | `noalias nocapture` en cada función que lo recibe |
| `Lista[T]` | `{ T*, i64, i64 }` | `{ptr, len, cap}`. ptr en Arena contigua |

### 6.2 Patrones de Emisión (inkwell)

```rust
// silc-backend/src/llvm.rs

use inkwell::{context::Context, module::Module, builder::Builder, values::*, types::*};

pub struct EmisorLLVM<'ctx> {
    ctx: &'ctx Context,
    modulo: Module<'ctx>,
    builder: Builder<'ctx>,
}

impl<'ctx> EmisorLLVM<'ctx> {
    /// Emite una TareaIR completa como función LLVM.
    pub fn emitir_tarea(&mut self, tarea: &TareaIR) -> Result<FunctionValue<'ctx>, ErrorBackend> {
        // 1. Declarar tipo función: (ArenaHandle, params...) -> ret
        let arena_ty = self.ctx.i8_type().ptr_type(AddressSpace::default());
        // ...

        // 2. Crear función con atributos: mustprogress nofree nosync nounwind willreturn memory(argmem: readwrite)
        let func = self.modulo.add_function(&tarea.nombre, fn_ty, None);
        func.add_attribute(AttributeLoc::Function, self.attr("mustprogress"));
        func.add_attribute(AttributeLoc::Function, self.attr("nofree"));
        // ...

        // 3. Emitir bloques básicos (mapear BlockId → BasicBlock)
        // 4. Para cada InstrCausal:
        //    - AsignarArena → call @sil_arena_alloc (función runtime C, declarada extern)
        //    - BinOpSegura(sin_overflow=true) → build_int_add + nsw flag
        //    - InvocarIO → call @sil_rt_invocar_io con token (validado por eBPF en runtime)
        //    - Retornar → build_return

        Ok(func)
    }

    /// Declara (no define) las funciones del runtime C: sil_arena_alloc, sil_fiber_spawn, etc.
    fn declarar_runtime(&self) {
        // i8* @sil_arena_alloc(i8* %arena, i64 %bytes) nounwind memory(inaccessiblemem: readwrite)
        // void @sil_arena_reset(i8* %arena) nounwind
        // i1 @sil_cap_validar(...) readonly
    }
}
```

### 6.3 Optimizaciones LLVM (Pass Pipeline)

```rust
// Orden de pases (via inkwell PassManager):
// 1. mem2reg (promote allocas)
// 2. instcombine + reassociate + gvn (limpieza algebraica)
// 3. loop-vectorize + slp-vectorizer (SIMD auto, guiado por restricciones)
// 4. inline (agresivo para funciones pequeñas, guiado por Causal-IR hot paths)
```

Si restricciones exigen `vectorizacion: simd_obligatoria` y el loop no vectoriza → **error de compilación** (no warning silencioso).

---

## 7. Runtime C API (para FFI Rust → C)

### 7.1 `sil-rt/include/sil_arena.h`

```c
#ifndef SIL_ARENA_H
#define SIL_ARENA_H

#include <stdint.h>
#include <stddef.h>

#define SIL_ARENA_PAGE_SIZE 65536  // 64 KB páginas contiguas

typedef struct SilArenaBloque {
    uint8_t *memoria;
    size_t capacidad;
    size_t usado;
    struct SilArenaBloque *siguiente;
} SilArenaBloque;

typedef struct {
    SilArenaBloque *inicio;
    SilArenaBloque *actual;
    size_t asignado_total;
} SilArena;

// O(1) amortizado. Thread-local por defecto (no locks).
SilArena sil_arena_crear(size_t capacidad_inicial);
void* sil_arena_asignar(SilArena *arena, size_t tamanio, size_t alineacion);
void sil_arena_destruir(SilArena *arena);  // Libera TODOS los bloques O(n_bloques), típicamente 1
void sil_arena_reset(SilArena *arena);     // O(1): usado=0 en bloque actual, conserva bloques

// Hoisting: transfiere ownership de puntero entre arenas sin copiar.
void sil_arena_promover(SilArena *origen, SilArena *destino, void *ptr, size_t tamanio);

#endif
```

### 7.2 `sil-rt/include/sil_fiber.h`

```c
#ifndef SIL_FIBER_H
#define SIL_FIBER_H

#include <stdint.h>
#include <stdbool.h>

// Fibra stackless: 64 bytes exactos (1 línea caché L1).
typedef struct __attribute__((aligned(64))) {
    void *ip;              // 8B: instruction pointer (continuación)
    void *sp_arena;        // 8B: puntero a Arena de la fibra
    uint64_t estado;       // 8B: LISTO=0, SUSPENDIDO=1, FINALIZADO=2
    uint8_t cap_token[32]; // 32B: token TPM (copia local, validado por eBPF)
    uint8_t _pad[8];       // 8B: padding a 64B
} SilFibra;
_Static_assert(sizeof(SilFibra) == 64, "Fibra debe ser exactamente 64 bytes");

// Scheduler M:N work-stealing.
typedef struct SilScheduler SilScheduler;
SilScheduler* sil_sched_crear(unsigned n_hilos_os);  // n = num_cpus
void sil_sched_destruir(SilScheduler *s);
bool sil_sched_spawn(SilScheduler *s, void (*entrada)(void*), void *arg, SilArena *arena);
void sil_sched_ejecutar(SilScheduler *s);  // Loop hasta que todas las fibras finalicen
void sil_fibra_yield(void);                 // Cede control (4 ciclos, sin syscall)

#endif
```

### 7.3 `sil-rt/include/sil_cap.h`

```c
#ifndef SIL_CAP_H
#define SIL_CAP_H

#include <stdint.h>
#include <stdbool.h>

typedef enum {
    SIL_CAP_NINGUNO = 0x00,
    SIL_CAP_LEER_ARCHIVO = 0x01,
    SIL_CAP_ESCRIBIR_ARCHIVO = 0x02,
    SIL_CAP_RED = 0x04,
    SIL_CAP_EXEC = 0x08,
} SilPermiso;

typedef struct {
    uint64_t id;
    uint32_t permisos;
    uint64_t expiracion_ns;  // Tiempo monotónico (clock_gettime CLOCK_MONOTONIC)
    uint8_t firma[32];       // HMAC-SHA256 (clave en TPM, no accesible)
    bool valida;
} SilCapacidad;

// Validación O(1) en userspace (verificación criptográfica completa en eBPF/kernel).
// Retorna false si expirada o permisos insuficientes.
bool sil_cap_validar(const SilCapacidad *cap, SilPermiso requerido, uint64_t ahora_ns);

// Solicita capacidad al driver TPM (stub en Fase 0, driver real en Fase 1+).
// En Fase 0: genera token autofirmado para desarrollo (NO SEGURO, solo testing).
SilCapacidad sil_cap_solicitar(const char *recurso, uint32_t permisos, uint64_t ttl_ns);

#endif
```

### 7.4 Bindings Rust Seguros (`silc-runtime/src/lib.rs`)

```rust
//! Bindings 100% seguros al runtime C. Cero `unsafe` en API pública.

use std::ffi::{CStr, CString};

pub struct Arena { inner: *mut ffi::SilArena }
pub struct Capacidad { inner: ffi::SilCapacidad }

impl Arena {
    pub fn nueva(capacidad_inicial: usize) -> Self { /* llama sil_arena_crear */ }
    pub fn asignar<T>(&mut self, valor: T) -> &mut T { /* bump alloc + place */ }
    pub fn reset(&mut self) { /* O(1) */ }
}

impl Drop for Arena {
    fn drop(&mut self) { /* sil_arena_destruir */ }
}

// Capacidad: Copy + !Send (ligada al hilo que la solicitó, previene exfiltración accidental)
#[derive(Clone, Copy)]
pub struct TokenCapacidad { /* ... */ }
```

---

## 8. CLI (`silc`) — Comandos y Flags

```
silc 1.0.0 — Compilador SIL (Fase 0)

USAGE:
    silc <COMANDO> [OPCIONES] [ARCHIVOS]

COMANDOS:
    build      Compila .sil → binario nativo (o --target)
    check      Solo verifica sintaxis + SMT (sin codegen)
    run        Compila y ejecuta (modo desarrollo)
    test       Ejecuta tests + verificación SMT de propiedades
    fmt        Formatea código (silfmt canónico)
    lsp        Inicia servidor LSP (para editores)
    emit-smt   Emite SMT-LIB2 para inspección manual (debug)

FLAGS GLOBALES:
    -v, --verbose          Logging detallado (tracing DEBUG)
        --smt-timeout <MS> Timeout por query SMT [default: 5000]
        --cache-dir <PATH> Directorio caché lemas [default: ~/.sil/cache]

BUILD:
    silc build main.sil -o mi_binario --target native --opt max
    --target <T>   native | wasm32-wasi | c99 | spirv [default: native]
    --opt <N>      none | speed | size | max [default: speed]
    --vectorizar <M> auto | simd | gpu | none [default: auto]
```

---

## 9. Estrategia de Reproducibilidad (Bit-for-Bit)

1. **Toolchain pineado:** `rust-toolchain.toml` con `channel = "1.75.0"`, componentes exactos.
2. **Lockfile commiteado:** `Cargo.lock` en repo. CI verifica `--locked --frozen`.
3. **Flags deterministas:**
   - Rust: `RUSTFLAGS="--remap-path-prefix=$HOME=~ -C link-arg=-Wl,--build-id=none"`
   - C: `CFLAGS="-std=c99 -O2 -ffile-prefix-map=$PWD=. -Wdate-time -D_FORTIFY_SOURCE=0"`
   - LLVM: `-C llvm-args=-opaque-pointers` (evita nondeterminismo versiones)
4. **Timestamps:** `SOURCE_DATE_EPOCH` desde `git log -1 --format=%ct`. Todos los artefactos usan este timestamp.
5. **Verificación CI:** Compila 2× en runners distintos → compara `sha256sum` → deben ser idénticos.

---

## 10. Plan de Implementación por Hitos

| Hito | Entregable | Criterio Aceptación |
|------|------------|---------------------|
| **M0: Esqueleto** | Workspace Cargo + crates vacías + CI verde | `cargo build --workspace` OK, `cargo test` 0 tests |
| **M1: Lexer** | `logos` CNL completo + 50 snapshot tests | `cargo insta test` pasa, throughput >1M líneas/seg (criterion) |
| **M2: Parser + AST** | `chumsky` EBNF completa + errores humanos | Todos los ejemplos Whitepaper parsean, errores con sugerencias |
| **M3: Causal-IR** | Lowering AST→IR + petgraph SCG | IR dumps estables (insta), sub-arenas detectadas |
| **M4: SMT** | `z3` integration + caché + contraejemplos | `verificar que false` falla con modelo concreto |
| **M5: Backend C99** | Emisión C99 + compila con gcc `-std=c99 -Werror` | E2E: `.sil` → `.c` → binario → output correcto |
| **M6: Backend LLVM** | `inkwell` emisión + optimizaciones | Binario nativo >2× más rápido que C99 `-O0` |
| **M7: Runtime C** | `arena.c` + `fiber.c` + `cap.c` + tests C | `proptest` invariants: no leaks (valgrind/ASan clean) |
| **M8: CLI + LSP** | `silc build/check/run` + LSP básico | VS Code extension conecta y muestra diagnósticos |
| **M9: Reproducibilidad** | Builds bit-identical en CI matrix | `sha256sum` idéntico en ubuntu/macos |
| **M10: Release v0.1.0** | Binarios `silc-*` en GitHub Releases | `silup.sh` instala y `silc --version` funciona |

---

## 11. Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| `inkwell` rompe con LLVM 18 | Media | Alto | Pin `llvm-sys 170.*`, CI con `llvm-17` fijo, `cargo update -p` controlado |
| `z3` libz3-sys compilación lenta (~5min) | Alta | Medio | Feature `vendored` + sccache en CI, documentar `apt install libz3-dev` para build rápido |
| Explosión SMT en programas grandes | Alta | Alto | Timeout 5s + caché + `verificar` solo en funciones marcadas (no todo el programa por defecto) |
| `chumsky 0.9` API inestable | Media | Medio | Pin exacto `=0.9.4`, tests snapshot exhaustivos, wrapper trait para migrar a 0.10 |
| Fibras ASM portabilidad (x86_64/ARM64/RISC-V) | Media | Alto | Fase 0: solo x86_64 + aarch64 (ASM separado por `#[cfg]`). RISC-V en Fase 1 |

---

*Fin de SPEC_FASE0_RUST.md — Listo para implementación. Siguiente: crear `silc-core/` workspace esqueleto (M0).*
