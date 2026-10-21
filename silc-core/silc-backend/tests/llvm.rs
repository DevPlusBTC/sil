#[cfg(all(feature = "llvm", target_os = "linux"))]
mod tests {
    use silc_backend::llvm::emitir;
    use silc_causal_ir::lower::bajar_tarea;
    use silc_frontend::parser::parsear;
    use silc_frontend::lexer::lexear;
    use silc_frontend::ast::Decl;

    #[test]
    fn emit_llvm_simple_task() {
        // Programa simple con una tarea sin parámetros.
        let src = "definir tarea suma(a: Entero64, b: Entero64) -> Entero64:\n    retornar a + b\n";
        let toks = lexear(src);
        let prog = parsear(&toks).expect("parseo falló");
        let tarea = match &prog.defs[0] {
            Decl::Tarea(t) => bajar_tarea(t).expect("lower falló"),
            _ => panic!("se esperaba tarea"),
        };
        let llvm = emitir(&tarea).expect("emisión LLVM falló");
        // Debe contener una definición de función con el nombre de la tarea.
        assert!(llvm.contains(&format!("define i64 @{}", tarea.nombre)));
    }
}
