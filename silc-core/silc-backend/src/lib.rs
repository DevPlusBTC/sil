//! silc-backend: Emisión de código nativo (LLVM, C99, WASM).
//!
//! M0: esqueletos que compilan. Implementación completa en M5 (C99) y M6 (LLVM).

pub mod c99;
pub mod llvm;
pub mod targets;
pub mod wasm;

pub use silc_contracts::*;
