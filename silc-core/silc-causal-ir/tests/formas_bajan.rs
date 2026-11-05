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
fn si_sino_baja_a_rama() {
    let ir = baja(
        "definir tarea clasificar(nota: Entero64) -> Entero64:\n    si nota >= 90:\n        retornar 1\n    sino:\n        retornar 0\n",
    );
    // El bloque de entrada debe tener un terminador Rama.
    assert!(
        matches!(
            ir.bloques[ir.entrada].terminador,
            silc_causal_ir::nodes::Terminador::Rama { .. }
        ),
        "entrada no tiene terminador Rama"
    );
    // Debe haber 3+ bloques (entrada, entonces, sino).
    assert!(
        ir.bloques.len() >= 3,
        "esperaba 3+ bloques, got {}",
        ir.bloques.len()
    );
}

#[test]
fn si_sino_si_cascada() {
    let ir = baja(
        "definir tarea clasificar2(nota: Entero64) -> Entero64:\n    si nota >= 90:\n        retornar 1\n    sino si nota >= 80:\n        retornar 2\n    sino:\n        retornar 0\n",
    );
    // Cascada: entrada + 3 niveles de rama + bloque final.
    let ramas: usize = ir
        .bloques
        .values()
        .filter(|b| matches!(b.terminador, silc_causal_ir::nodes::Terminador::Rama { .. }))
        .count();
    assert_eq!(ramas, 2, "se esperan 2 ramas (si + sino si), got {ramas}");
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
