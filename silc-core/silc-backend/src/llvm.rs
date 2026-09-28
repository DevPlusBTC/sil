//! Emisión LLVM IR vía inkwell (LLVM 17).
//!
//! Requiere feature `llvm` + LLVM 17 instalado. Implementación completa en M6.

use silc_causal_ir::nodes::TareaIR;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorLLVM {
    #[error("Error LLVM: {0}")]
    Llvm(String),
    #[error("Backend LLVM no compilado (activa feature `llvm` + instala LLVM 17)")]
    NoDisponible,
}

/// Emite módulo LLVM. M0: placeholder.
pub fn emitir(_tarea: &TareaIR) -> Result<String, ErrorLLVM> {
    // TODO(M6): inkwell Context/Module/Builder + nsw attrs + runtime decls.
    Err(ErrorLLVM::NoDisponible)
}
