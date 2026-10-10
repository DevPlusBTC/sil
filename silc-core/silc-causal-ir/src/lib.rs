//! silc-causal-ir: Representación intermedia Causal (SSA) + pipeline SMT.
//!
//! M0: esqueletos que compilan. Lowering + SMT en M3/M4.

pub mod contracts;
pub mod lower;
pub mod nodes;
pub mod smt;

pub use contracts::*;
