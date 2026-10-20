//! silc-frontend: Lexer + Parser + AST para SIL (CNL).

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;

#[cfg(test)]
mod tests {
    use crate::ast::Decl;
    use crate::lexer::{lexear, Token, TokenFull};
    use crate::parser::parsear;

    #[test]
    fn pipeline_publico_lexer_parser() {
        let fuente = "\
definir tarea doblar(x: Entero64) -> Entero64:
    asumir x >= 0
    demostrar x * 2 >= x
    retornar x
";
        let toks = lexear(fuente);
        assert!(
            toks.iter()
                .any(|t| matches!(t, TokenFull::Tok(s) if s.tok == Token::Definir)),
            "falta el token `definir`"
        );
        let prog = parsear(&toks).expect("parse OK");
        assert_eq!(prog.defs.len(), 1);
        assert!(matches!(prog.defs[0], Decl::Tarea(_)));
    }
}
