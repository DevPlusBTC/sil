//! silc-frontend: S-exp Reader + AST Builder + AST para SIL.

pub mod a_ast;
pub mod ast;
pub mod error;
pub mod reader;

#[cfg(test)]
mod tests {
    use crate::a_ast::parsear;
    use crate::ast::Decl;

    #[test]
    fn pipeline_publico_sexp_ast() {
        let fuente = "\
(task doblar
  (params (x i64))
  (returns i64)
  (body
    (assume (>= x 0))
    (prove (>= (* x 2) x))
    (return x)))";
        let prog = parsear(fuente).expect("parse OK");
        assert_eq!(prog.defs.len(), 1);
        assert!(matches!(prog.defs[0], Decl::Tarea(_)));
    }
}
