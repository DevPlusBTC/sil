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
    let f = ejemplo("ok", "definir tarea f(x: Entero64) -> Entero64:\n    asumir x > 0\n    demostrar x > 0\n    retornar x\n");
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK"));
}

#[test]
fn check_verifica_let() {
    // Regresión: `let` debe entrar en SMT como definición SSA, no como variable libre.
    let f = ejemplo(
        "let",
        "definir tarea f(monto: Entero64, saldo: Entero64) -> Booleano:\n    asumir que monto > 0\n    asumir que saldo >= monto\n    let nuevo_saldo = saldo - monto\n    demostrar que nuevo_saldo >= 0\n    retornar verdadero\n",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK"));
}

#[test]
fn check_falla_smt() {
    let f = ejemplo(
        "bad",
        "definir tarea f(x: Entero64) -> Entero64:\n    demostrar x > 1000000\n    retornar x\n",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("verificaci"));
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
        "definir tarea s(x: Entero64, y: Entero64) -> Entero64:\n    retornar x + y\n",
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
        "definir tarea s(x: Entero64, y: Entero64) -> Entero64:\n    retornar x + y\n",
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
    let f = ejemplo("t", "definir tarea f() -> Entero64:\n    retornar 1\n");
    Command::cargo_bin("silc")
        .unwrap()
        .args(["build", f.path().to_str().unwrap(), "--target", "llvm"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("target desconocido"));
}

#[test]
fn build_c99_biblioteca_con_varias_tareas() {
    let f = ejemplo(
        "lib",
        "definir tarea sumar_uno(x: Entero64) -> Entero64:\n    retornar x + 1\n\ndefinir tarea doblar(y: Entero64) -> Entero64:\n    retornar y * 2\n",
    );
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
    let f = ejemplo(
        "libw",
        "definir tarea sumar_uno(x: Entero64) -> Entero64:\n    retornar x + 1\n\ndefinir tarea doblar(y: Entero64) -> Entero64:\n    retornar y * 2\n",
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
fn build_c99_generar_sin_principal_falla_limpio() {
    let f = ejemplo(
        "fiblib",
        "definir tarea t() -> Entero64:\n    retornar 1\n\ndefinir tarea f(x: Entero64) -> Entero64:\n    generar t\n    retornar x\n",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["build", f.path().to_str().unwrap(), "--target", "c99"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requiere una tarea principal"));
}

#[test]
fn check_invariante_constante_sin_variables() {
    let f = ejemplo(
        "constok",
        "definir tarea f() -> Entero64:\n    demostrar 1 > 0\n    retornar 1\n",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK"));
}

#[test]
fn check_invariante_constante_falsa_falla() {
    let f = ejemplo(
        "constbad",
        "definir tarea f() -> Entero64:\n    demostrar 1 > 2\n    retornar 1\n",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["check", f.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("verificaci"));
}

#[test]
fn run_biblioteca_falla_limpio() {
    let f = ejemplo(
        "runlib",
        "definir tarea f(x: Entero64) -> Entero64:\n    retornar x\n",
    );
    Command::cargo_bin("silc")
        .unwrap()
        .args(["run", f.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("biblioteca"));
}

#[test]
fn emit_smt_muestra_script() {
    let f = ejemplo("smt", "definir tarea f(x: Entero64) -> Entero64:\n    asumir x > 0\n    demostrar x > 0\n    retornar x\n");
    Command::cargo_bin("silc")
        .unwrap()
        .args(["emit-smt", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("(set-logic QF_LIA)"))
        .stdout(predicate::str::contains("(check-sat)"));
}
