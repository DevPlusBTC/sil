//! Formas sintácticas nuevas: cada programa parsea sin error.

use silc_frontend::{lexer::lexear, parser::parsear};

fn parsea(src: &str) {
    parsear(&lexear(src))
        .map_err(|e| format!("parse: {e:?}"))
        .unwrap();
}

#[test]
fn formas_nuevas() {
    for src in [
        "definir tarea f() -> Booleano:\n    solicitar capacidad lectura_archivo para \"/tmp/x\" como cap\n    validar cap\n    retornar verdadero\n",
        "definir tarea f() -> Booleano:\n    solicitar capacidad red para \"h:1\" con ttl 99 como cap\n    validar cap\n    retornar verdadero\n",
        "definir tarea f() -> Entero64:\n    crear canal con capacidad 8 como c\n    enviar 40 en c\n    recibir de c como v\n    retornar v\n",
        "definir tarea f() -> Booleano:\n    solicitar capacidad lectura_archivo para \"/tmp/d\" como cap\n    leer archivo \"/tmp/d\" con cap como t\n    retornar verdadero\n",
        "definir tarea f() -> Booleano:\n    solicitar capacidad escritura_archivo para \"/tmp/d\" como cap\n    escribir 1 en archivo \"/tmp/d\" con cap\n    retornar verdadero\n",
        "definir tarea f() -> Booleano:\n    solicitar capacidad red para \"h:2\" como cap\n    enviar 1 en red \"h:2\" con cap\n    retornar verdadero\n",
        "definir tarea f() -> Booleano:\n    solicitar capacidad red para \"h:2\" como cap\n    recibir de red \"h:2\" con cap como r\n    retornar verdadero\n",
    ] {
        parsea(src);
    }
}

#[test]
fn formas_mal_uso_fallan_en_parse() {
    // `solicitar` sin permiso válido.
    assert!(
        parsear(&lexear(
            "definir tarea f() -> Booleano:\n    solicitar capacidad volar para \"/x\" como cap\n    retornar verdadero\n",
        ))
        .is_err()
    );
}
