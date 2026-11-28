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
    let f = ejemplo("ok", "(task f (params (x i64)) (returns i64) (body (assume (> x 0)) (prove (> x 0)) (return x)))");
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

#[test]
fn emit_smt_muestra_script() {
    let f = ejemplo("smt", "(task f (params (x i64)) (returns i64) (body (assume (> x 0)) (prove (> x 0)) (return x)))");
    Command::cargo_bin("silc")
        .unwrap()
        .args(["emit-smt", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("(set-logic QF_LIA)"))
        .stdout(predicate::str::contains("(check-sat)"));
}

#[test]
fn emit_rust_genera_codigo_audit() {
    // Sintaxis correcta: contratos antes de los dos puntos
    let f = ejemplo(
        "rust",
        "(task sumar\n  (params (a i64) (b i64))\n  (returns i64)\n  (requires (>= a 0))\n  (ensures (>= a 0))\n  (body (return (+ a b))))",
    );
    let out = tempfile::Builder::new().suffix(".rs").tempfile().unwrap();
    let out_path = out.path().to_str().unwrap().to_string();
    drop(out);
    Command::cargo_bin("silc")
        .unwrap()
        .args(["emit-rust", f.path().to_str().unwrap(), "--out", &out_path])
        .assert()
        .success();
    let rust = std::fs::read_to_string(&out_path).unwrap();
    assert!(rust.contains("// SIL-UD convertido a Rust para auditoría"));
    assert!(rust.contains("pub fn sumar("));
    let _ = std::fs::remove_file(&out_path);
}
