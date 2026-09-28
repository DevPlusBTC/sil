//! Emisión WASM/WASI vía wasm-encoder.
//!
//! M0: esqueleto. Implementación completa en M6.

use silc_causal_ir::nodes::TareaIR;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorWasm {
    #[error("Error WASM: {0}")]
    Wasm(String),
}

/// Emite módulo WASM binario. M0: placeholder.
pub fn emitir(_tarea: &TareaIR) -> Result<Vec<u8>, ErrorWasm> {
    // TODO(M6): wasm-encoder Module + WASI imports + memory + funcs.
    Ok(vec![])
}
