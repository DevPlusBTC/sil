//! Tests de integración CLI (assert_cmd): silc check/build/emit-smt end-to-end.
//!
//! M8: verifica el pipeline completo via binario real.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn ejemplo(nombre: &str, contenido: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new()
        .suffix(&format!("_{nombre}.sil"))
        .tempfile()
        .unwrap();
    f.write_all(contenido.as_bytes()).unwrap();
    f
}

#[test]
fn check_ok() {
    let f = ejemplo(
        "ok",
        "(task f (params (x i64)) (returns i64) (body (assume (> x 0)) (prove (> x 0)) (return x)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn check_verifica_let() {
    // Regresión: `let` debe entrar en SMT como definición SSA, no como variable libre.
    let f = ejemplo(
        "let",
        "(task f
  (params (monto i64) (saldo i64))
  (returns booleano)
  (body
    (assume (> monto 0))
    (assume (>= saldo monto))
    (let nuevo_saldo (- saldo monto))
    (prove (>= nuevo_saldo 0))
    (return verdadero)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn check_falla_smt() {
    let f = ejemplo(
        "bad",
        "(task f (params (x i64)) (returns i64) (body (prove (> x 1000000)) (return x)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invariante violada"));
}

#[test]
fn check_error_sintaxis() {
    let f = ejemplo("syn", "tarea f():\n    retornar 1\n");
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn build_c99_genera_archivo() {
    let f = ejemplo(
        "b",
        "(task s (params (x i64) (y i64)) (returns i64) (body (return (+ x y))))",
    );
    let out = tempfile::Builder::new().suffix(".c").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    // Borrar el tempfile para que silc lo cree (o sobrescriba).
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--out",
            &out_path,
            "--target",
            "c99",
        ])
        .assert()
        .success();
    let c = std::fs::read_to_string(&out_path).unwrap();
    assert!(c.contains("long long s("), "firma C ausente");
    assert!(c.contains("#include <stdio.h>"));
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn build_wasm_genera_binario_valido() {
    let f = ejemplo(
        "w",
        "(task s (params (x i64) (y i64)) (returns i64) (body (return (+ x y))))",
    );
    let out = tempfile::Builder::new().suffix(".wasm").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--out",
            &out_path,
            "--target",
            "wasm",
        ])
        .assert()
        .success();
    let b = std::fs::read(&out_path).unwrap();
    // Magic WASM: \0asm
    assert!(b.starts_with(b"\0asm"), "magic WASM ausente");
    // Validar con wasmparser.
    wasmparser::Validator::new().validate_all(&b).unwrap();
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn build_target_desconocido_falla() {
    let f = ejemplo("t", "(task f (params) (returns i64) (body (return 1)))");
    Command::cargo_bin("silc")
        .unwrap()
        .args(["build", f.path().to_str().unwrap(), "--target", "llvm"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("target desconocido"));
}

const BIBLIOTECA: &str = "(task sumar_uno
  (params (x i64))
  (returns i64)
  (body (return (+ x 1))))

(task doblar
  (params (y i64))
  (returns i64)
  (body (return (* y 2))))";

#[test]
fn build_c99_biblioteca_con_varias_tareas() {
    let f = ejemplo("lib", BIBLIOTECA);
    let out = tempfile::Builder::new().suffix(".c").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--out",
            &out_path,
            "--target",
            "c99",
        ])
        .assert()
        .success();
    let c = std::fs::read_to_string(&out_path).unwrap();
    assert!(c.contains("long long sumar_uno("), "falta sumar_uno");
    assert!(c.contains("long long doblar("), "falta doblar");
    assert!(!c.contains("int main"), "una biblioteca no debe tener main");
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn build_wasm_biblioteca_con_varias_tareas() {
    let f = ejemplo("libw", BIBLIOTECA);
    let out = tempfile::Builder::new().suffix(".wasm").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--out",
            &out_path,
            "--target",
            "wasm",
        ])
        .assert()
        .success();
    let b = std::fs::read(&out_path).unwrap();
    assert!(b.starts_with(b"\0asm"), "magic WASM ausente");
    wasmparser::Validator::new().validate_all(&b).unwrap();
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn build_wasm_biblioteca_varias_tareas_con_si() {
    // Dos tareas con si/sino → valida generación WASM multi-tarea con control de flujo.
    let f = ejemplo(
        "libwsi",
        "(task suma_segura
  (params (a i64) (b i64))
  (returns i64)
  (body (if (> a 0) (body (return (+ a b))) (else (body (return b))))))

(task doble
  (params (x i64))
  (returns i64)
  (body (if (> x 0) (body (return (* x 2))) (else (body (return 0))))))",
    );
    let out = tempfile::Builder::new().suffix(".wasm").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--out",
            &out_path,
            "--target",
            "wasm",
        ])
        .assert()
        .success();
    let b = std::fs::read(&out_path).unwrap();
    assert!(b.starts_with(b"\0asm"), "magic WASM ausente");
    wasmparser::Validator::new().validate_all(&b).unwrap();
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn build_c99_biblioteca_sin_principal_genera_sin_main() {
    let f = ejemplo("fiblib", BIBLIOTECA);
    let out = tempfile::Builder::new().suffix(".c").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args([
            "build",
            f.path().to_str().unwrap(),
            "--out",
            &out_path,
            "--target",
            "c99",
        ])
        .assert()
        .success();
    let c = std::fs::read_to_string(&out_path).unwrap();
    assert!(!c.contains("int main"), "una biblioteca no debe tener main");
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn check_invariante_constante_sin_variables() {
    let f = ejemplo(
        "constok",
        "(task f (params) (returns i64) (body (prove (> 1 0)) (return 1)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn check_invariante_constante_falsa_falla() {
    let f = ejemplo(
        "constbad",
        "(task f (params) (returns i64) (body (prove (> 1 2)) (return 1)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("falsificable"));
}

#[test]
fn run_biblioteca_falla_limpio() {
    let f = ejemplo("runlib", BIBLIOTECA);
    Command::cargo_bin("silc")
        .unwrap()
        .args(["run", f.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("solo 1 archivo"));
}

#[test]
fn emit_smt_muestra_script() {
    let f = ejemplo(
        "smt",
        "(task f (params (x i64)) (returns i64) (body (assume (> x 0)) (prove (> x 0)) (return x)))",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["emit-smt", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("(set-logic QF_LIA)"))
        .stdout(predicate::str::contains("(check-sat)"));
}
