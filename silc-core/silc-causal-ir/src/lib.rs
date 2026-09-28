//! silc-causal-ir: Representación intermedia Causal (SSA) + pipeline SMT.
//!
//! M0: esqueletos que compilan. Lowering + SMT en M3/M4.

pub mod nodes;
pub mod lower;
pub mod smt;
pub mod verify;
