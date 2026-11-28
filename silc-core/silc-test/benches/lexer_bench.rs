//! Benchmarks del frontend: throughput del reader y del AST builder.
//!
//! Usa un programa SIL representativo y mide bytes/s (reader) y formas/s
//! (AST builder) con criterion. Ejecutar con `cargo bench -p silc-test`.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use silc_frontend::a_ast;
use silc_frontend::reader;

const PROGRAMA: &str = "(task proceso
  (params (valor_a i64) (valor_b i64))
  (returns i64)
  (body
    (assume (>= valor_a 0))
    (assume (>= valor_b 0))
    (let suma i64 (+ valor_a valor_b))
    (let producto i64 (* valor_a valor_b))
    (prove (>= suma valor_a))
    (prove (>= producto 0))
    (return (+ suma producto))))";

fn bench_reader(c: &mut Criterion) {
    let mut grupo = c.benchmark_group("frontend");
    grupo.throughput(Throughput::Bytes(PROGRAMA.len() as u64));
    grupo.bench_function("reader", |b| b.iter(|| reader::leer(black_box(PROGRAMA))));
    grupo.finish();
}

fn bench_ast(c: &mut Criterion) {
    let formas = reader::leer(PROGRAMA).expect("programa válido");
    let mut grupo = c.benchmark_group("frontend");
    grupo.throughput(Throughput::Elements(formas.len() as u64));
    grupo.bench_function("sexp_a_ast", |b| {
        b.iter(|| a_ast::programa(black_box(&formas)))
    });
    grupo.finish();
}

criterion_group!(benches, bench_reader, bench_ast);
criterion_main!(benches);
