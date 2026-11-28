//! Formas S-exp nuevas: cada programa parsea sin error.

use silc_frontend::a_ast::parsear;

fn parsea(src: &str) {
    parsear(src).map_err(|e| format!("parse: {e:?}")).unwrap();
}

#[test]
fn formas_nuevas() {
    for src in [
        "(task f
  (params)
  (returns booleano)
  (body
    (request lectura_archivo \"/tmp/x\" (as cap))
    (validate cap)
    (return verdadero)))",
        "(task f
  (params)
  (returns booleano)
  (body
    (request red \"h:1\" (ttl 99) (as cap))
    (validate cap)
    (return verdadero)))",
        "(task f
  (params)
  (returns i64)
  (body
    (chan c (capacity 8))
    (send 40 c)
    (recv c v)
    (return v)))",
        "(task f
  (params)
  (returns booleano)
  (body
    (request lectura_archivo \"/tmp/d\" (as cap))
    (io-read \"/tmp/d\" cap t)
    (return verdadero)))",
        "(task f
  (params)
  (returns booleano)
  (body
    (request escritura_archivo \"/tmp/d\" (as cap))
    (io-write 1 \"/tmp/d\" cap)
    (return verdadero)))",
        "(task f
  (params)
  (returns booleano)
  (body
    (request red \"h:2\" (as cap))
    (io-send 1 \"h:2\" cap)
    (return verdadero)))",
        "(task f
  (params)
  (returns booleano)
  (body
    (request red \"h:2\" (as cap))
    (io-recv \"h:2\" cap r)
    (return verdadero)))",
        "(task clasificar
  (params (nota i64))
  (returns texto)
  (body
    (if (>= nota 90)
      (body (return \"A\"))
      (else (body (return \"B\"))))))",
        "(task clasificar2
  (params (nota i64))
  (returns texto)
  (body
    (if (>= nota 90)
      (body (return \"A\"))
      (else (body
        (if (>= nota 80)
          (body (return \"B\"))
          (else (body (return \"C\")))))))))",
    ] {
        parsea(src);
    }
}

#[test]
fn formas_mal_uso_fallan_en_parse() {
    // `request` con permiso inválido.
    assert!(parsear(
        "(task f
  (params)
  (returns booleano)
  (body
    (request volar \"/x\" (as cap))
    (return verdadero)))"
    )
    .is_err());
}
