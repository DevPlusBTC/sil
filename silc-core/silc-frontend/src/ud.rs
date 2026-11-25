//! Parser ultra-densa para SIL-UD.
//!
//! Formato ultra-densa (SIL-UD) para generación por IA:
//! - `fn nombre(params) -> Tipo:` en lugar de `definir tarea nombre(params) -> Tipo:`
//! - `[cond]` para precondiciones (SMT conditions)
//! - `ret` en lugar de `retornar`
//!
//! NOTA: Este parser interno es experimental. Para uso en producción, usa el parser CNL estándar.

use crate::ast::*;
use crate::error::ErrorFrontend;

/// Parsea código SIL-UD → AST estándar de SIL.
pub fn parsear_ud(fuente: &str) -> Result<Program, ErrorFrontend> {
    let _chars = fuente.chars().peekable();
    // TODO: Implementar parser ultra-densa simple
    // Por ahora, delegar al parser CNL estándar
    crate::parser::parsear(&crate::lexer::lexear(fuente))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fn_simple() {
        // Test básico usando el parser CNL estándar
        let fuente = "definir tarea f():\n    retornar 1\n";
        let prog = parsear_ud(fuente).unwrap();
        assert_eq!(prog.defs.len(), 1);
    }
}
