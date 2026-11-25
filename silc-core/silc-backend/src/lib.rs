//! silc-backend: Emisión de código nativo (C99, WASM, LLVM, Rust).
//!
//! C99, WASM y Rust están implementados; LLVM requiere la feature `llvm` y LLVM 17.
//! El backend Rust es opcional: solo para auditoría humana, no para producción.

pub mod c99;
pub mod llvm;
pub mod rust;
pub mod wasm;

pub use silc_contracts::*;
