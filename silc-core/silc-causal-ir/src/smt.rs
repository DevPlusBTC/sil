//! Pipeline SMT: traducción a SMT-LIB2 + verificación con Z3 + caché incremental.
//!
//! M0: esqueleto. Implementación completa en M4.

use crate::nodes::{FormulaLogica, TareaIR};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorSMT {
    #[error("Invariante violada (contraejemplo disponible)")]
    InvarianteViolada,
    #[error("Timeout SMT ({0}ms)")]
    Timeout(u64),
}

/// Verifica una tarea. M0: siempre Ok (sin verificación real).
pub fn verificar_tarea(_tarea: &TareaIR, _timeout_ms: u64) -> Result<(), ErrorSMT> {
    // TODO(M4): z3 integration + caché sha3 + contraejemplos.
    Ok(())
}

/// Traduce fórmula a SMT-LIB2 (para debug --emit-smt).
pub fn a_smtlib2(_f: &FormulaLogica) -> String {
    // TODO(M4)
    String::from("(assert true)")
}
