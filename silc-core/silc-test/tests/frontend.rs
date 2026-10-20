//! Integración del frontend: lexer → parser → AST → lowering.
//!
//! Complementa los tests unitarios de cada módulo con el pipeline completo a
//! nivel de biblioteca: programa multi-declaración, spans reales, errores de
//! sintaxis y robustez del lexer/parser ante entradas arbitrarias.

use proptest::prelude::*;
use silc_causal_ir::lower::bajar_tarea;
use silc_causal_ir::nodes::SilType;
use silc_frontend::ast::{Decl, Expr, Stmt};
use silc_frontend::error::ErrorFrontend;
use silc_frontend::lexer::lexear;
use silc_frontend::parser::parsear;

const PROGRAMA: &str = "\
definir estructura Punto:
    x: Entero64
    y: Entero64

definir variante Resultado:
    opcion Ok con datos (valor: Entero64)
    opcion Error

definir tarea sumar(valor_a: Entero64, valor_b: Entero64) -> Entero64:
    asumir valor_a >= 0
    let total: Entero64 = valor_a + valor_b
    demostrar total >= valor_a
    retornar total
";

fn defs(fuente: &str) -> Vec<Decl> {
    parsear(&lexear(fuente)).expect("parse OK").defs
}

#[test]
fn parsea_programa_multideclaracion() {
    let defs = defs(PROGRAMA);
    assert_eq!(defs.len(), 3);
    assert!(matches!(defs[0], Decl::Estructura(_)));
    assert!(matches!(defs[1], Decl::Variante(_)));
    assert!(matches!(defs[2], Decl::Tarea(_)));
}

#[test]
fn spans_declaraciones_cubren_el_fuente() {
    for decl in defs(PROGRAMA) {
        let span = match &decl {
            Decl::Tarea(t) => &t.span,
            Decl::Estructura(e) => &e.span,
            Decl::Variante(v) => &v.span,
            _ => unreachable!("PROGRAMA solo declara tarea/estructura/variante"),
        };
        assert!(span.start < span.end, "span vacío o invertido: {span:?}");
        assert!(
            span.end <= PROGRAMA.len(),
            "span fuera del fuente: {span:?}"
        );
    }
}

#[test]
fn spans_de_sentencia_y_expresion_son_exactos() {
    let defs = defs(PROGRAMA);
    let Decl::Tarea(t) = &defs[2] else {
        panic!("se esperaba tarea")
    };
    let Some(Stmt::Retornar { span, expr }) = t.cuerpo.stmts.last() else {
        panic!("se esperaba retornar")
    };
    assert!(span.start < span.end, "span de retornar inválido: {span:?}");
    let Some(Expr::Var(id)) = expr else {
        panic!("se esperaba variable")
    };
    assert_eq!(&PROGRAMA[id.span.clone()], "total");
}

#[test]
fn binop_abarca_ambos_operandos() {
    let defs = defs(PROGRAMA);
    let Decl::Tarea(t) = &defs[2] else {
        panic!("se esperaba tarea")
    };
    let Some(Stmt::Demostrar { cond, .. }) = t
        .cuerpo
        .stmts
        .iter()
        .find(|s| matches!(s, Stmt::Demostrar { .. }))
    else {
        panic!("se esperaba demostrar")
    };
    let Expr::BinOp { span, .. } = cond else {
        panic!("se esperaba comparación")
    };
    assert_eq!(&PROGRAMA[span.clone()], "total >= valor_a");
}

#[test]
fn error_por_falta_de_dos_puntos() {
    let err = parsear(&lexear("definir tarea f()\n    retornar 1\n")).unwrap_err();
    assert!(matches!(
        err,
        ErrorFrontend::TokenInesperado { .. } | ErrorFrontend::FinInesperado
    ));
}

#[test]
fn error_por_declaracion_desconocida() {
    let err = parsear(&lexear("definir castillo f\n")).unwrap_err();
    assert!(matches!(err, ErrorFrontend::TokenInesperado { .. }));
}

#[test]
fn lowering_preserva_firma_de_tarea() {
    let defs = defs(PROGRAMA);
    let Decl::Tarea(t) = &defs[2] else {
        panic!("se esperaba tarea")
    };
    let ir = bajar_tarea(t).expect("lowering OK");
    assert_eq!(ir.nombre, "sumar");
    assert_eq!(ir.params.len(), 2);
    assert_eq!(ir.retorno, SilType::Entero64);
    assert!(ir.nombres.values().any(|n| n == "total"));
}

const PROGRAMA_EXTENDIDO: &str = r#"definir servicio Puerta:
    escuchar en puerto 8080 con protocolo HTTP
    cuando llegue evento Peticion(ruta: Texto):
        retornar
    definir tarea despachar(ruta: Texto) -> Entero64:
        retornar 0
    bajo restricciones:
        latencia_maxima: baja

definir modelo Cliente:
    id: Entero64
    nombre: Texto

definir memoria_persistente Cache:
    clave: Entero64
    valor: Texto

definir capacidad RedSalida:
    permitir conectar a "api.ejemplo.com" en puerto 443
    limite_ancho_banda: "100mb"

modulo Principal:
    exportar servicio Puerta
    exportar tarea despachar

migrar Archivo desde v1 a v2:
    al acceder_campo "notas" en registro Archivo:
        retornar valor_predeterminado "sin notas"
        bajo restricciones:
            migracion_memoria: lazy_on_read
            reestructuracion_disco: cero_copia
"#;

#[test]
fn parsea_servicio_completo() {
    let defs = defs(PROGRAMA_EXTENDIDO);
    let Decl::Servicio(s) = &defs[0] else {
        panic!("se esperaba servicio")
    };
    assert_eq!(s.nombre.nombre, "Puerta");
    let red = s.red.as_ref().expect("configuración de red");
    assert_eq!(red.puerto, 8080);
    assert_eq!(red.protocolo.as_ref().unwrap().nombre, "HTTP");
    assert_eq!(s.manejadores.len(), 1);
    assert_eq!(s.manejadores[0].evento.nombre, "Peticion");
    assert_eq!(s.manejadores[0].params.len(), 1);
    assert_eq!(s.tareas.len(), 1);
    assert_eq!(s.tareas[0].nombre.nombre, "despachar");
    assert_eq!(s.restricciones.len(), 1);
    assert!(s.span.start < s.span.end);
}

#[test]
fn parsea_modelo_memoria_capacidad_modulo_migracion() {
    let defs = defs(PROGRAMA_EXTENDIDO);
    assert_eq!(defs.len(), 6);

    let Decl::Modelo(m) = &defs[1] else {
        panic!("modelo")
    };
    assert_eq!(m.nombre.nombre, "Cliente");
    assert_eq!(m.campos.len(), 2);

    let Decl::MemoriaPersistente(mem) = &defs[2] else {
        panic!("memoria_persistente")
    };
    assert_eq!(mem.nombre.nombre, "Cache");
    assert_eq!(mem.campos.len(), 2);

    let Decl::Capacidad(cap) = &defs[3] else {
        panic!("capacidad")
    };
    assert_eq!(cap.nombre.nombre, "RedSalida");
    assert_eq!(cap.recurso, "api.ejemplo.com");
    assert_eq!(cap.puerto, 443);
    assert_eq!(cap.limite_ancho_banda, "100mb");

    let Decl::Modulo(mod_) = &defs[4] else {
        panic!("módulo")
    };
    assert_eq!(mod_.nombre.nombre, "Principal");
    assert_eq!(mod_.exportaciones.len(), 2);

    let Decl::Migracion(mig) = &defs[5] else {
        panic!("migración")
    };
    assert_eq!(mig.origen.nombre, "Archivo");
    assert_eq!(mig.version_desde.nombre, "v1");
    assert_eq!(mig.version_a.nombre, "v2");
    assert_eq!(mig.campo, "notas");
    assert_eq!(mig.registro.nombre, "Archivo");
    assert_eq!(mig.restricciones.len(), 2);
}

#[test]
fn spans_de_declaraciones_extendidas_son_validos() {
    for decl in defs(PROGRAMA_EXTENDIDO) {
        let span = match &decl {
            Decl::Servicio(s) => &s.span,
            Decl::Modelo(m) => &m.span,
            Decl::MemoriaPersistente(m) => &m.span,
            Decl::Capacidad(c) => &c.span,
            Decl::Modulo(m) => &m.span,
            Decl::Migracion(m) => &m.span,
            _ => unreachable!(),
        };
        assert!(span.start < span.end, "span inválido: {span:?}");
        assert!(span.end <= PROGRAMA_EXTENDIDO.len());
    }
}

#[test]
fn servicio_con_tareas_se_baja_a_ir() {
    let defs = defs(PROGRAMA_EXTENDIDO);
    let Decl::Servicio(s) = &defs[0] else {
        panic!("se esperaba servicio")
    };
    let ir = bajar_tarea(&s.tareas[0]).expect("lowering de tarea anidada");
    assert_eq!(ir.nombre, "despachar");
    assert_eq!(ir.params.len(), 1);
    assert_eq!(ir.retorno, SilType::Entero64);
}

#[test]
fn error_si_capacidad_no_declara_ancho_de_banda() {
    let err = parsear(&lexear(
        r#"definir capacidad C:
    permitir conectar a "x" en puerto 1
    otra_clave: "v"
"#,
    ))
    .unwrap_err();
    assert!(matches!(err, ErrorFrontend::TokenInesperado { .. }));
}

proptest! {
    /// El lexer y el parser deben devolver un resultado (no entrar en pánico)
    /// para cualquier entrada, incluso si no es SIL válido.
    #[test]
    fn lexer_y_parser_no_entran_en_panico(fuente in any::<String>()) {
        let toks = lexear(&fuente);
        let _ = parsear(&toks);
    }
}
