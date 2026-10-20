//! silc-backend: Emisión de código nativo (C99, WASM, LLVM).
//!
//! C99 y WASM están implementados; LLVM requiere la feature `llvm` y LLVM 17.

pub mod c99;
pub mod llvm;
pub mod wasm;

pub use silc_contracts::*;
