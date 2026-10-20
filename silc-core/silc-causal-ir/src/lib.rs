//! silc-causal-ir: Representación intermedia Causal (SSA) + pipeline SMT.
//!
//! Lowering AST → IR (SSA, arenas, invariantes) y verificación SMT
//! (backend de intervalos por defecto; Z3 con feature `smt`).

pub mod lower;
pub mod nodes;
pub mod smt;

pub use silc_contracts::*;
