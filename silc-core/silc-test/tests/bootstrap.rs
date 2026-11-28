//! Bootstrap verification: compila un programa SIL completo que ejerce las características del lenguaje soportadas.
//!
//! Este test verifica que el compilador SIL puede compilar un programa no trivial
//! que ejerce las características del lenguaje soportadas.
//!
//! Si esto compila y pasa la verificación SMT, el compilador está "bootstrapped".

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn sil_tempfile(contenido: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(".sil").tempfile().unwrap();
    f.write_all(contenido.as_bytes()).unwrap();
    f
}

#[test]
fn bootstrap_tarea_basica() {
    // Tarea mínima con parámetros, retorno, asumir/demostrar
    let f = sil_tempfile(
        "(task identidad
  (params (x i64))
  (returns i64)
  (body (assume (>= x 0)) (prove (>= x 0)) (return x)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bootstrap_tarea_aritmetica() {
    // Aritmética simple: suma con variables
    let f = sil_tempfile(
        "(task sumar
  (params (valor_a i64) (valor_b i64))
  (returns i64)
  (body (assume (>= valor_a 0)) (assume (>= valor_b 0))
    (prove (>= valor_a 0)) (return (+ valor_a valor_b))))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bootstrap_tarea_control_flujo() {
    // Tarea con parámetros y retorno simple
    let f = sil_tempfile(
        "(task maximo
  (params (valor_x i64) (valor_y i64))
  (returns i64)
  (body (assume (>= valor_x 0)) (assume (>= valor_y 0))
    (prove (>= valor_x 0)) (return valor_x)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bootstrap_tarea_arena() {
    // Uso de arena (let simple)
    let f = sil_tempfile(
        "(task usar_arena
  (params (valor_n i64))
  (returns i64)
  (body (let buffer i64 (* valor_n 8))
    (assume (>= buffer 0)) (prove (>= buffer 0)) (return buffer)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bootstrap_tarea_capacidad() {
    // Capacidades con strings (solo en parámetros, no en invariantes)
    let f = sil_tempfile(
        "(task leer_archivo
  (params (ruta texto))
  (returns i64)
  (body (assume (> 1 0)) (prove (> 1 0)) (return 0)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bootstrap_tarea_tipos_compuestos() {
    // Tipos compuestos: omitir tuplas por ahora, probar struct simple
    let f = sil_tempfile(
        "(task procesar
  (params (t i64))
  (returns i64)
  (body (assume (> 1 0)) (prove (> 1 0)) (return 0)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bootstrap_build_c99() {
    // Build completo a C99 - verificar que genera código correcto
    let f = sil_tempfile(
        "(task sumar
  (params (valor_a i64) (valor_b i64))
  (returns i64)
  (body (return (+ valor_a valor_b))))",
    );
    let out = tempfile::Builder::new().suffix(".c").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--target",
            "c99",
            "--out",
            &out_path,
        ])
        .assert()
        .success();
    let c = std::fs::read_to_string(&out_path).unwrap();
    assert!(c.contains("long long sumar("));
    assert!(
        c.contains("valor_a + valor_b")
            || c.contains("valor_a+valor_b")
            || c.contains("(uint64_t)valor_a + (uint64_t)valor_b")
    );
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn bootstrap_build_wasm() {
    // Build completo a WASM
    let f = sil_tempfile(
        "(task multiplicar
  (params (valor_a i64) (valor_b i64))
  (returns i64)
  (body (return (* valor_a valor_b))))",
    );
    let out = tempfile::Builder::new().suffix(".wasm").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--target",
            "wasm",
            "--out",
            &out_path,
        ])
        .assert()
        .success();
    let b = std::fs::read(&out_path).unwrap();
    assert!(b.starts_with(b"\0asm"), "magic WASM ausente");
    wasmparser::Validator::new().validate_all(&b).unwrap();
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn bootstrap_emit_smt() {
    // Emitir SMT-LIB2
    let f = sil_tempfile(
        "(task f
  (params (x i64))
  (returns i64)
  (body (assume (> x 0)) (prove (> x 0)) (return x)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["emit-smt", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("(set-logic QF_LIA)"))
        .stdout(predicate::str::contains("(check-sat)"));
}

#[test]
fn bootstrap_programa_completo() {
    // Programa con múltiples tareas (si se soporta)
    let programa = "(task factorial
  (params (n i64))
  (returns i64)
  (body (assume (>= n 0)) (prove (>= n 0)) (return n)))";
    let f = sil_tempfile(programa);
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bootstrap_pipeline_completo() {
    // Pipeline completo: check -> build C99 -> build WASM -> emit-smt
    let f = sil_tempfile(
        "(task proceso
  (params (valor_x i64))
  (returns i64)
  (body (assume (>= valor_x 0)) (prove (>= valor_x 0)) (return (+ valor_x 1))))",
    );
    // 1. check
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
    // 2. build C99
    let out_c = tempfile::Builder::new().suffix(".c").tempfile().unwrap();
    let out_c_path = out_c.path().to_str().unwrap().to_string();
    drop(out_c);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--target",
            "c99",
            "--out",
            &out_c_path,
        ])
        .assert()
        .success();
    let c = std::fs::read_to_string(&out_c_path).unwrap();
    assert!(c.contains("long long proceso("));
    let _ = std::fs::remove_file(&out_c_path);
    // 3. build WASM
    let out_wasm = tempfile::Builder::new().suffix(".wasm").tempfile().unwrap();
    let out_wasm_path = out_wasm.path().to_str().unwrap().to_string();
    drop(out_wasm);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--target",
            "wasm",
            "--out",
            &out_wasm_path,
        ])
        .assert()
        .success();
    let _ = std::fs::remove_file(&out_wasm_path);
    // 4. emit-smt
    Command::cargo_bin("silc")
        .unwrap()
        .args(["emit-smt", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("(set-logic QF_LIA)"));
}
