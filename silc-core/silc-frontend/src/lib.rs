//! silc-frontend: Lexer + Parser + AST para SIL (CNL).
//!
//! Fase M0: esqueletos que compilan. Implementación completa en M1 (lexer) y M2 (parser).

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;

#[cfg(test)]
mod test_z3_lexer {
    #[test]
    fn test_z3_lexer() {
        use crate::lexer::lexear;

        let content = "definir tarea test_z3(x: Entero64) -> Booleano:\n    asumir que x > 0\n    demostrar que x * 2 > x\n    retornar verdadero";

        let tokens = lexear(content);
        for t in &tokens {
            println!("{:?}", t);
        }
    }
}
