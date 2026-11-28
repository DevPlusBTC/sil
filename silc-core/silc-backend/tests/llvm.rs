#[cfg(all(feature = "llvm", target_os = "linux"))]
mod tests {
    use silc_backend::llvm::{emitir, ErrorLLVM};
    use silc_causal_ir::lower::bajar_tarea;
    use silc_frontend::a_ast::parsear;
    use silc_frontend::ast::Decl;

    #[test]
    fn emit_llvm_simple_task() {
        // Programa simple con una tarea sin parámetros.
        let src = "(task suma (params (a i64) (b i64)) (returns i64) (body (return (+ a b))))";
        let prog = parsear(src).expect("parseo falló");
        let tarea = match &prog.defs[0] {
            Decl::Tarea(t) => bajar_tarea(t).expect("lower falló"),
            _ => panic!("se esperaba tarea"),
        };
        let llvm = match emitir(&tarea) {
            Ok(ir) => ir,
            Err(ErrorLLVM::NoDisponible) => {
                eprintln!(
                    "LLVM no disponible (feature llvm sin librería instalada), saltando test"
                );
                return;
            }
            Err(e) => panic!("emisión LLVM falló: {e}"),
        };
        // Debe contener una definición de función con el nombre de la tarea.
        // LLVM IR emite: `define <ret> @nombre(...) { ... }`
        assert!(
            llvm.contains(&format!("@{}", tarea.nombre)),
            "IR no contiene @{}. IR generado:\n{llvm}",
            tarea.nombre
        );
    }
}
