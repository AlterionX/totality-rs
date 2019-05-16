#[macro_use]
extern crate criterion;

use criterion::{Criterion, black_box, Fun};

extern crate totality_sync as sync;
use sync::triple_buffer as tb;

fn tb_create(n: u64) -> (tb::ReadingView<Vec<u8>>, tb::EditingView<Vec<u8>>) {
    tb::buffer(vec![0; n as usize])
}
fn tb_testing(n: u64) -> () {
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("Creation", |b| b.iter(|| tb_create(black_box(5000))));
    // c.bench_function("Usage", |b| b.iter(|| fibonacci(black_box(20))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

