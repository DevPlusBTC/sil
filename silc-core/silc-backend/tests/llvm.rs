#[cfg(all(feature = "llvm", target_os = "linux"))]
mod tests {
    use silc_backend::llvm::{emitir, ErrorLLVM};
    use silc_causal_ir::lower::bajar_tarea;
    use silc_frontend::ast::Decl;
    use silc_frontend::lexer::lexear;
    use silc_frontend::parser::parsear;

    #[test]
    fn emit_llvm_simple_task() {
        // Programa simple con una tarea sin parÃ¡metros.
        let src = "definir tarea suma(a: Entero64, b: Entero64) -> Entero64:\n    retornar a + b\n";
        let toks = lexear(src);
        let prog = parsear(&toks).expect("parseo fallÃ³");
        let tarea = match &prog.defs[0] {
            Decl::Tarea(t) => bajar_tarea(t).expect("lower fallÃ³"),
            _ => panic!("se esperaba tarea"),
        };
        let llvm = match emitir(&tarea) {
            Ok(ir) => ir,
            Err(ErrorLLVM::NoDisponible) => {
                eprintln!("LLVM no disponible (feature llvm sin librería instalada), saltando test");
                return;
            }
            Err(e) => panic!("emisión LLVM falló: {e}"),
        };
        // Debug: imprimir IR si falla
        eprintln!("=== LLVM IR generado ===\n{llvm}\n=========================");
        // Debe contener una definición de función con el nombre de la tarea.
        // LLVM IR emite: `define <ret> @nombre(...) { ... }`
        assert!(
            llvm.contains(&format!("@{}", tarea.nombre)),
            "IR no contiene @{}. IR generado:\n{llvm}",
            tarea.nombre
        );
    }
}
