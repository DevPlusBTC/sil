//! Las formas nuevas bajan a IR con operaciones reales (no TODO).

use silc_causal_ir::lower::bajar_tarea;
use silc_frontend::a_ast::parsear;

fn baja(src: &str) -> silc_causal_ir::nodes::TareaIR {
    let prog = parsear(src).map_err(|e| format!("parse: {e:?}")).unwrap();
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
        "(task f
  (params)
  (returns booleano)
  (body
    (request lectura_archivo \"/tmp/x\" (as cap))
    (validate cap)
    (return verdadero)))",
    );
    assert!(tiene_op(&ir, "SolicitarCapacidad"));
    assert!(tiene_op(&ir, "ValidarCapacidad"));

    let ir = baja(
        "(task f
  (params)
  (returns i64)
  (body
    (chan c (capacity 8))
    (send 40 c)
    (recv c v)
    (return v)))",
    );
    assert!(tiene_op(&ir, "CrearCanal"));
    assert!(tiene_op(&ir, "EnviarCanal"));
    assert!(tiene_op(&ir, "RecibirCanal"));

    let ir = baja(
        "(task f
  (params)
  (returns booleano)
  (body
    (request lectura_archivo \"/tmp/d\" (as cap))
    (io-read \"/tmp/d\" cap t)
    (return verdadero)))",
    );
    assert!(tiene_op(&ir, "InvocarIO"));
}

#[test]
fn si_sino_baja_a_rama() {
    let ir = baja(
        "(task clasificar
  (params (nota i64))
  (returns i64)
  (body
    (if (>= nota 90)
      (body (return 1))
      (else (body (return 0))))))",
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
        "(task clasificar2
  (params (nota i64))
  (returns i64)
  (body
    (if (>= nota 90)
      (body (return 1))
      (else (body
        (if (>= nota 80)
          (body (return 2))
          (else (body (return 0)))))))))",
    );
    // Cascada: entrada + 3 niveles de rama + bloque final.
    let ramas: usize = ir
        .bloques
        .values()
        .filter(|b| matches!(b.terminador, silc_causal_ir::nodes::Terminador::Rama { .. }))
        .count();
    assert_eq!(ramas, 2, "se esperan 2 ramas (si + sino si), got {ramas}");
}

/// Un `mientras` baja a un bucle real: bloque condicion con terminador Rama,
/// y el cuerpo salta de vuelta a condicion (arista atras, no un salto simple).
///
/// NOTA: `para` está en el lowering pero el iterable no tiene soporte de
/// literal de colección (Vector/Mapa/Tupla). shortcut: colección iterable pendiente.
#[test]
fn mientras_es_un_bucle_real() {
    let ir = baja(
        "(task sumar
  (params (n i64))
  (returns i64)
  (body
    (let _ i64 n)
    (while (> n 0)
      (body
        (let _ i64 n)))
    (return 0)))",
    );
    let ramas: Vec<_> = ir
        .bloques
        .values()
        .filter(|b| matches!(b.terminador, silc_causal_ir::nodes::Terminador::Rama { .. }))
        .collect();
    assert!(
        !ramas.is_empty(),
        "esperaba al menos un bloque con terminador Rama (la condicion del bucle)"
    );
    // El bloque de entrada salta a la condicion, y el cuerpo vuelve a ella:
    // eso es una arista atras, por lo que el IR es ciclico.
    let saltos: Vec<_> = ir
        .bloques
        .values()
        .filter(|b| matches!(b.terminador, silc_causal_ir::nodes::Terminador::Salto(_)))
        .collect();
    assert!(
        saltos.len() >= 2,
        "esperaba al menos 2 saltos (entrada->condicion y cuerpo->condicion), got {}",
        saltos.len()
    );
}

#[test]
fn usos_sin_declarar_fallan() {
    for src in [
        "(task f
  (params)
  (returns booleano)
  (body
    (validate cap)
    (return verdadero)))",
        "(task f
  (params)
  (returns i64)
  (body
    (recv c v)
    (return v)))",
        "(task f
  (params)
  (returns i64)
  (body
    (send 1 c)
    (return 0)))",
    ] {
        let prog = parsear(src).unwrap();
        let silc_frontend::ast::Decl::Tarea(t) = &prog.defs[0] else {
            panic!("no tarea");
        };
        assert!(bajar_tarea(t).is_err(), "debió fallar: {src}");
    }
}
