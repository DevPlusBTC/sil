//! Las formas nuevas bajan a IR con operaciones reales (no TODO).

use silc_causal_ir::lower::bajar_tarea;
use silc_frontend::{lexer::lexear, parser::parsear};

fn baja(src: &str) -> silc_causal_ir::nodes::TareaIR {
    let prog = parsear(&lexear(src))
        .map_err(|e| format!("parse: {e:?}"))
        .unwrap();
    let silc_frontend::ast::Decl::Tarea(t) = &prog.defs[0] else {
        panic!("solo tareas en este test");
    };
    bajar_tarea(t).map_err(|e| format!("lower: {e:?}")).unwrap()
}

fn tiene_op(ir: &silc_causal_ir::nodes::TareaIR, nombre: &str) -> bool {
    ir.bloques
        .values()
        .flat_map(|b| &b.instrs)
        .any(|i| format!("{:?}", i.op).contains(nombre))
}

#[test]
fn formas_nuevas_bajan() {
    let ir = baja(
        "definir tarea f() -> Booleano:\n    solicitar capacidad lectura_archivo para \"/tmp/x\" como cap\n    validar cap\n    retornar verdadero\n",
    );
    assert!(tiene_op(&ir, "SolicitarCapacidad"));
    assert!(tiene_op(&ir, "ValidarCapacidad"));

    let ir = baja(
        "definir tarea f() -> Entero64:\n    crear canal con capacidad 8 como c\n    enviar 40 en c\n    recibir de c como v\n    retornar v\n",
    );
    assert!(tiene_op(&ir, "CrearCanal"));
    assert!(tiene_op(&ir, "EnviarCanal"));
    assert!(tiene_op(&ir, "RecibirCanal"));

    let ir = baja(
        "definir tarea f() -> Booleano:\n    solicitar capacidad lectura_archivo para \"/tmp/d\" como cap\n    leer archivo \"/tmp/d\" con cap como t\n    retornar verdadero\n",
    );
    assert!(tiene_op(&ir, "InvocarIO"));
}

#[test]
fn usos_sin_declarar_fallan() {
    for src in [
        "definir tarea f() -> Booleano:\n    validar cap\n    retornar verdadero\n",
        "definir tarea f() -> Entero64:\n    recibir de c como v\n    retornar v\n",
        "definir tarea f() -> Entero64:\n    enviar 1 en c\n    retornar 0\n",
    ] {
        let prog = parsear(&lexear(src)).unwrap();
        let silc_frontend::ast::Decl::Tarea(t) = &prog.defs[0] else {
            panic!("no tarea");
        };
        assert!(bajar_tarea(t).is_err(), "debió fallar: {src}");
    }
}
