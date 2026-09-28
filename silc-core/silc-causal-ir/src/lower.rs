//! Lowering AST → Causal-IR.
//!
//! M0: esqueleto. Implementación completa en M3.

use crate::nodes::TareaIR;
use silc_frontend::ast::TareaDecl;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorBajada {
    #[error("Tipo no soportado: {0}")]
    TipoNoSoportado(String),
}

/// Baja una tarea AST a IR. M0: retorna IR vacía.
pub fn bajar_tarea(_tarea: &TareaDecl) -> Result<TareaIR, ErrorBajada> {
    // TODO(M3): implementación completa con arenas, SSA, invariantes.
    Ok(TareaIR { nombre: _tarea.nombre.nombre.clone() })
}
