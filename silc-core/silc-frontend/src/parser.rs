//! Parser CNL usando `chumsky` (parser combinators).
//!
//! M0: esqueleto. Implementación completa EBNF → chumsky en M2.

use crate::{ast::*, lexer::SpannedToken};
use crate::error::ErrorFrontend;

/// Parsea tokens → AST. M0: retorna error no-implementado.
pub fn parsear(_toks: &[SpannedToken]) -> Result<Program, ErrorFrontend> {
    // TODO(M2): implementación completa según SPEC (mapeo EBNF → chumsky).
    Err(ErrorFrontend::FinInesperado)
}
