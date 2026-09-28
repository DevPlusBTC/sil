//! Orquestación de verificación: extrae asunciones/metas y las verifica.
//!
//! M0: esqueleto. Implementación completa en M4.

use crate::{nodes::TareaIR, smt::ErrorSMT};

/// Verifica todas las invariantes de una tarea.
pub fn verificar(_tarea: &TareaIR) -> Result<(), ErrorSMT> {
    // TODO(M4)
    Ok(())
}
