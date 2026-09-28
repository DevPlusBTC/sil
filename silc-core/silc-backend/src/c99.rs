//! Emisión C99 portable (fallback sin LLVM).
//!
//! M0: esqueleto. Implementación completa en M5.

use silc_causal_ir::nodes::TareaIR;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorC99 {
    #[error("Error de emisión: {0}")]
    Emision(String),
}

/// Emite código C99 para una tarea. M0: placeholder.
pub fn emitir(_tarea: &TareaIR) -> Result<String, ErrorC99> {
    // TODO(M5): emisión real con runtime Arenas embebido.
    Ok(String::from("/* TODO(M5): C99 emission */\n"))
}
