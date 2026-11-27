//! Puerta de aceptación de la migración S-exp:
//! Verifica que los 14 ejemplos traducidos a S-exp producen un AST **idéntico**
//! al parser CNL histórico (módulo spans).

use silc_frontend::a_ast;
use silc_frontend::ast::Program;
use silc_frontend::lexer::lexear;
use silc_frontend::parser;
use std::fs;
use std::path::Path;

/// Convierte la representación `Debug` de un AST borrando los rangos de `span: N..M`.
fn normalizar_ast(ast: &Program) -> String {
    let debug = format!("{ast:#?}");
    let mut resultado = String::with_capacity(debug.len());

    // `span: N..M` -> `span: 0..0`
    let bytes = debug.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if debug[i..].starts_with("span: ") {
            resultado.push_str("span: 0..0");
            i += 6;
            // Saltar hasta coma, salto de línea o cierre de llave/paréntesis
            while i < bytes.len()
                && bytes[i] != b','
                && bytes[i] != b'\n'
                && bytes[i] != b'}'
                && bytes[i] != b')'
            {
                i += 1;
            }
            continue;
        }
        resultado.push(bytes[i] as char);
        i += 1;
    }
    resultado
}

fn verificar_paridad_archivo(nombre: &str) {
    let base = Path::new("../../");
    let valid_path = base.join(format!("examples/valid/{nombre}.sil"));
    let invalid_path = base.join(format!("examples/invalid/{nombre}.sil"));
    let cnl_path = if valid_path.exists() {
        valid_path
    } else if invalid_path.exists() {
        invalid_path
    } else {
        panic!("No se encontró {nombre}.sil ni en valid/ ni en invalid/");
    };
    let sexp_path = base.join(format!("examples/sexp/{nombre}.sexp"));

    let cnl_src = fs::read_to_string(&cnl_path)
        .unwrap_or_else(|e| panic!("Error leyendo {}: {e}", cnl_path.display()));
    let sexp_src = fs::read_to_string(&sexp_path)
        .unwrap_or_else(|e| panic!("Error leyendo {}: {e}", sexp_path.display()));

    let cnl_toks = lexear(&cnl_src);
    let cnl_ast = parser::parsear(&cnl_toks)
        .unwrap_or_else(|e| panic!("CNL parse fallo en {}: {e:?}", cnl_path.display()));

    let sexp_ast = a_ast::parsear(&sexp_src)
        .unwrap_or_else(|e| panic!("S-exp parse fallo en {}: {e:?}", sexp_path.display()));

    let norm_cnl = normalizar_ast(&cnl_ast);
    let norm_sexp = normalizar_ast(&sexp_ast);

    if norm_cnl != norm_sexp {
        eprintln!("=== DISCREPANCIA DE AST EN {nombre} ===");
        eprintln!("--- CNL AST ---\n{norm_cnl}");
        eprintln!("--- SEXP AST ---\n{norm_sexp}");
        panic!("El AST generado por S-exp no coincide con el del parser CNL para {nombre}");
    }
}

#[test]
fn paridad_ast_14_ejemplos() {
    let ejemplos = [
        "calculadora_segura",
        "descuentos",
        "factorial",
        "inventario",
        "procesar_pago",
        "programa_confianza",
        "sumar",
        "test_calc",
        "validacion",
        "verificacion_z3",
        "verificacion_z3_compleja",
        "falla_smt",
        "fibra_basica",
        "scheduler_real",
    ];

    for e in ejemplos {
        verificar_paridad_archivo(e);
    }
}
