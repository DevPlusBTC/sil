# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working in this repository.

## SIL — Semantic Intention Language

Systems language in Spanish (CNL) with compile-time SMT verification. Compiles SIL → C99 / WASM / LLVM IR. Zero-cost arena memory, zero-trust capabilities (HMAC), fiber scheduler, lock-free channels, optional io_uring/eBPF.

## Commands

All Rust commands from `silc-core/`:

```bash
cargo build --workspace
cargo test --workspace                              # default features (interval SMT)
cargo test --workspace --features smt              # Z3 backend
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -p silc-cli -- check examples/sexp/sumar.sexp
cargo run -p silc-cli -- build examples/sexp/sumar.sexp --target c99 -o out.c
```

Single test: `cargo test -p silc-frontend -- test_name`

Python prototype tests (root): `python3 test_sil_compiler.py`, `test_sil_stdlib.py`, `test_sil_wasm.py`

**Test-count sync** — CI `--check` fails on drift:
```bash
python3 scripts/test_counts.py            # rewrites README, SPEC.md, WHITE_PAPER.md
python3 scripts/test_counts.py --check    # fails if docs are stale
```

VS Code ext (`silc-vscode/`): `npm install && npm run compile`

Toolchain pinned to 1.97.1 (`rust-toolchain.toml`), MSRV 1.88.0. Use `--locked`.

## Pipeline

```
.sil source
  │ silc-frontend  — reader S-exp + AST builder → AST
  ▼ silc-causal-ir — lower to SSA IR (slotmap); SMT verify contracts
      intervalos (default, pure Rust, sound/incomplete) | Z3 (feature `smt`, vendored)
      SHA3-512 cache skips re-verification
  ▼ silc-backend — codegen: c99.rs, wasm.rs, llvm.rs (feature `llvm`)
  ▼ silc-runtime + sil-rt (C) — arenas, capabilities, fibers, channels, aio, ebpf
```

## Crate roles

| Crate | Role |
|-------|------|
| `silc-frontend` | Lexer, parser, AST. `ast.rs`, `error.rs`, `lexer.rs`, `parser.rs` |
| `silc-causal-ir` | SSA lowering (`lower.rs`), IR nodes (`nodes.rs`), SMT (`smt.rs`) |
| `silc-backend` | C99, WASM, LLVM codegen |
| `silc-runtime` | Rust FFI to C runtime (`sil-rt/`) |
| `silc-cli` | `silc` binary: check, build, emit-smt, run, lsp |
| `silc-contracts` | `require`/`ensure`/`invariant` — used across all crates |
| `silc-test` | Integration tests, proptests, benches |

`sil-rt/` is plain C compiled via `silc-runtime/build.rs`. The C99 backend embeds its own arena runtime copy.

## Known gaps (verified in code, not docs)

- **WASM loops broken**: `Terminador::Salto` in `wasm.rs` is a no-op (`let _ = (destino, &frame_stack);`). `mientras`/`para` don't work in WASM. `if/else` works.
- **ADTs**: `estructura`/`variante`/`modelo` parse but are not lowered or codegen'd — backend returns `TipoNoSoportado`.
- **LLVM**: feature-gated, separate CI job (`ci-heavy.yml`), not in main build.
- **Z3**: requires `--features smt` (vendored build needs Python+CMake on Linux).
- **Services**: `servicio` parses, codegen emits bootstrap + stubs only.

## CI (`.github/workflows/`)

- `rust.yml` — fmt, build, test, clippy, C99 example validation (gcc/clang `-Werror`). Per-crate with `-j 1` to avoid OOM.
- `ci-heavy.yml` — SMT (Z3) tests, fuzzing (nightly + ASan), LLVM test, reproducibility. Nightly cron.
- `release.yml` — build + test + publish.

## Conventions

- Comments: Spanish for SIL semantics, English for implementation details.
- Rust: `snake_case`, `UpperCamelCase`, 4-space indent. Public APIs documented.
- Conventional Commits: `feat:`, `fix:`, `test:`, `docs:`.
- Generated C must compile with `gcc -std=c99 -Wall -Wextra -Werror`.

## Docs

`docs/WHITE_PAPER.md` (aspirational + status matrix), `docs/SPEC.md` (implemented grammar/API), `docs/API.md`, `docs/USER_GUIDE.md`. Note: `WHITE_PAPER.md` still has stale test counts and some overstated claims — `scripts/test_counts.py` syncs the counts; the prose is not auto-checked.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- When graphify-out/graph.json exists, run `graphify query "<question>"` before your first grep, search or source-file read for a codebase question, and put this rule in every subagent prompt that explores code. Read raw files only for lines the graph or an earlier read already located. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. When the answer is under a Markdown heading, `graphify section "<heading>"` prints that section's text, so the .md need not be opened. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
- A committed graphify-out/graph.json is rewritten whole on every build, so a large diff to it in a pull request is expected. Do not flag it as churn or suggest skipping the graph rebuild or its hooks.
