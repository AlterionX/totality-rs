use criterion::{black_box, criterion_group, criterion_main, Criterion};
use totality_sync::triple_buffer::{buffer, TripleBuffer};

pub fn benchmark(c: &mut Criterion) {
    let buffer = TripleBuffer::<u8>::raw(0);
    {
        let mut uncontended = c.benchmark_group("uncontended");
        uncontended.bench_function("read output", |b| {
            b.iter(|| {
                black_box(*buffer.reader_r());
            });
        });
        uncontended.bench_function("clean update", |b| {
            b.iter(|| {
                buffer.snatch();
            });
        });
        uncontended.bench_function("clean receive", |b| {
            b.iter(|| {
                buffer.snatch();
                black_box(*buffer.reader_r());
            });
        });
        uncontended.bench_function("write input", |b| {
            b.iter(|| {
                *buffer.editor_w() = black_box(0);
            });
        });
        uncontended.bench_function("publish", |b| {
            b.iter(|| {
                buffer.advance();
            });
        });
        uncontended.bench_function("send", |b| {
            b.iter(|| {
                *buffer.editor_w() = black_box(0);
                buffer.advance();
            });
        });
        uncontended.bench_function("publish + dirty update", |b| {
            b.iter(|| {
                buffer.advance();
                buffer.snatch();
            });
        });
        uncontended.bench_function("transmit", |b| {
            b.iter(|| {
                *buffer.editor_w() = black_box(0);
                buffer.advance();
                buffer.snatch();
                black_box(*buffer.reader_r());
            });
        });
    }

    let rbuf = std::sync::Arc::clone(&buffer);
    let wbuf = buffer;
    {
        let mut read_contended = c.benchmark_group("read contention");
        testbench::run_under_contention(
            || black_box(*rbuf.reader_r()),
            || {
                read_contended.bench_function("write input", |b| {
                    b.iter(|| {
                        *wbuf.editor_w() = black_box(0);
                    })
                });
                read_contended.bench_function("publish", |b| {
                    b.iter(|| {
                        wbuf.advance();
                    })
                });
                read_contended.bench_function("send", |b| {
                    b.iter(|| {
                        *wbuf.editor_w() = black_box(0);
                        wbuf.advance();
                    })
                });
            },
        );
    }

    {
        let mut write_contended = c.benchmark_group("write contention");
        testbench::run_under_contention(
            || {
                *wbuf.editor_w() = black_box(0);
                wbuf.advance();
            },
            || {
                write_contended.bench_function("read output", |b| {
                    b.iter(|| {
                        black_box(*rbuf.reader_r());
                    })
                });
                write_contended.bench_function("update", |b| {
                    b.iter(|| {
                        rbuf.snatch();
                    })
                });
                write_contended.bench_function("receive", |b| {
                    b.iter(|| {
                        rbuf.snatch();
                        black_box(*rbuf.reader_r());
                    })
                });
            },
        );
    }
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
