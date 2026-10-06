# SIL API Reference (v0.1.0)

Auto-generado con `cargo doc --no-deps --workspace`.

---

## Crates Publicadas

| Crate | Versión | Descripción |
|-------|---------|-------------|
| `silc-frontend` | 0.1.0 | Lexer, Parser CNL, AST |
| `silc-causal-ir` | 0.1.0 | Causal-IR, SMT encoding, Z3 bridge |
| `silc-backend` | 0.1.0 | Codegen C99, WASM, LLVM IR |
| `silc-runtime` | 0.1.0 | Arena O(1), tipos FFI, capacidades |

---

## silc-frontend

### Módulos Principales

```rust
use silc_frontend::{lexer, parser, ast};

/// Lexer: tokeniza CNL determinista (sin backtracking)
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError>;

/// Parser: genera AST desde tokens
pub fn parse(tokens: Vec<Token>) -> Result<Ast, ParseError>;

/// AST Nodos principales
pub enum Stmt {
    Assume(Expr),      // asumir
    Prove(Expr),       // demostrar
    Return(Expr),      // retornar
    Let { name, ty, value }, // let x: T = e
}
```

### Uso Básico
```rust
let tokens = silc_frontend::lexer::tokenize(source)?;
let ast = silc_frontend::parser::parse(tokens)?;
```

---

## silc-causal-ir

### Causal-IR (SSA Causal)

```rust
use silc_causal_ir::{CausalIr, InstrCausal, FormulaLogica};

/// Genera Causal-IR desde AST
pub fn lower_to_causal_ir(ast: &Ast) -> Result<CausalIr, LowerError>;

/// Nodo canónico: ⟨ID_SSA, Operación, Capacidad, Invariante⟩
pub struct InstrCausal {
    pub id: SSAId,
    pub op: OpCausal,
    pub cap: Option<CapToken>,
    pub inv: FormulaLogica,
}
```

### SMT Encoding (Z3/CVC5)

```rust
use silc_causal_ir::smt::{SmtContext, prove, check_sat};

/// Codifica invariantes a SMT-LIB2 QF_LIA
pub fn encode_to_smt(ir: &CausalIr) -> String;

/// Prueba incremental con caché SHA3-512
pub async fn prove_incremental(ctx: &SmtContext, inv: &FormulaLogica) -> SmtResult;
```

---

## silc-backend

### Codegen Targets

```rust
use silc_backend::{Codegen, Target};

/// Emite C99 portable
pub fn emit_c99(ir: &CausalIr) -> Result<String, CodegenError>;

/// Emite WASM (magic \0asm)
pub fn emit_wasm(ir: &CausalIr) -> Result<Vec<u8>, CodegenError>;

/// Emite LLVM IR
pub fn emit_llvm(ir: &CausalIr) -> Result<String, CodegenError>;
```

### Runtime Embedded (C99)
```c
/* Generado automáticamente en cada .c */
typedef struct SilArena_ { ... } SilArena_;
static void *sil_arena_asignar_(SilArena_*, size_t, size_t);
static void sil_arena_destruir_(SilArena_*);

/* Tipos SIL → C */
typedef int64_t Entero64;
typedef struct { const char* ptr; int64_t len; } SilTexto_;
```

---

## silc-runtime

### Arena O(1)

```rust
use silc_runtime::arena::{SilArena, SilArenaBloque};

/// Arena principal de tarea
pub struct SilArena {
    pub inicio: *mut SilArenaBloque,
    pub actual: *mut SilArenaBloque,
    pub asignado_total: usize,
}

/// Sub-arena cíclica O(1) reset
pub fn sil_arena_asignar(arena: &mut SilArena, size: usize, align: usize) -> *mut u8;
pub fn sil_arena_reset(arena: &mut SilArena);
```

### Capacidades Hardware

```rust
use silc_runtime::cap::{SilCap, CapToken};

/// Capacidad firmada TPM 2.0 (32 bytes)
#[repr(C)]
pub struct SilCap {
    pub id: u64,
    pub perms: u32,
    pub exp_ns: u64,
    pub firma: [u8; 32],
}

/// Validador micro-temporal (< 500ns)
pub fn validar_cap(cap: &SilCap, ahora_ns: u64) -> bool;
```

---

## CLI (`silc`)

```bash
silc check <file.sil>              # Verifica sintaxis + SMT
silc build <file.sil> --target c99|wasm -o <out>
silc emit-smt <file.sil>           # Emite SMT-LIB2 QF_LIA
```

**Exit codes:** 0=OK, 1=Error, 2=I/O

---

## LSP Server (`silc-lsp`)

```bash
silc-lsp  # JSON-RPC 3.17 sobre stdio
```

**Capabilities:**
- `textDocument/didOpen` `didChange` → Diagnostics (2ms)
- `textDocument/completion` → trigger `.`
- `textDocument/hover` → Tipo + contratos
- `textDocument/publishDiagnostics` → Errores en lenguaje natural
- Time-Travel Debugger (core): Navegación SCG

---

## VS Code Extension (`silc-vscode`)

```json
{
  "silc.diagnostics.enabled": true
}
```

**Comandos:** `SIL: Toggle Diagnostics` (Ctrl+Shift+P)

---

## Testing

```bash
cargo test -p silc-cli          # 7/7 tests integración
cargo test -p silc-test         # Tests unitarios
cargo test --workspace          # Full suite
```

---

*Generado: `cargo doc --no-deps --workspace --open`*