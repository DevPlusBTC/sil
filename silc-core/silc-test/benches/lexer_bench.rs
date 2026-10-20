//! Benchmarks del frontend: throughput del lexer y del parser.
//!
//! Usa un programa SIL representativo y mide bytes/s (lexer) y tokens/s
//! (parser) con criterion. Ejecutar con `cargo bench -p silc-test`.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use silc_frontend::lexer::lexear;
use silc_frontend::parser::parsear;

const PROGRAMA: &str = "\
definir tarea proceso(valor_a: Entero64, valor_b: Entero64) -> Entero64:
    asumir valor_a >= 0
    asumir valor_b >= 0
    let suma: Entero64 = valor_a + valor_b
    let producto: Entero64 = valor_a * valor_b
    demostrar suma >= valor_a
    demostrar producto >= 0
    retornar suma + producto
";

fn bench_lexer(c: &mut Criterion) {
    let mut grupo = c.benchmark_group("frontend");
    grupo.throughput(Throughput::Bytes(PROGRAMA.len() as u64));
    grupo.bench_function("lexer", |b| b.iter(|| lexear(black_box(PROGRAMA))));
    grupo.finish();
}

fn bench_parser(c: &mut Criterion) {
    let tokens = lexear(PROGRAMA);
    let mut grupo = c.benchmark_group("frontend");
    grupo.throughput(Throughput::Elements(tokens.len() as u64));
    grupo.bench_function("parser", |b| b.iter(|| parsear(black_box(&tokens))));
    grupo.finish();
}

criterion_group!(benches, bench_lexer, bench_parser);
criterion_main!(benches);
