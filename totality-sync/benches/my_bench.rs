#[macro_use]
extern crate criterion;
extern crate totality_sync as sync;

use sync::triple_buffer as tb;

use std::{time::Duration, thread::sleep, sync::{Arc, atomic::{AtomicU64, Ordering}}};

use criterion::{black_box, Criterion, BatchSize};

fn tb_read(rv: tb::Reader<()>) -> tb::Reader<()>{
    rv.grab_always().release_always()
}
fn tb_edit(e: tb::Editor<()>) -> tb::Editor<()> {
    e.grab_always().commit_always()
}

fn tb_raw_read(r: &Arc<tb::TripleBuffer<()>>) -> () {
    r.snatch();
    // &*r.reader_r()
}
fn tb_raw_edit(e: &Arc<tb::TripleBuffer<()>>) -> () {
    e.advance();
    // &*e.editor_r()
}

fn tb_counter(i: usize) {
    let (mut r, mut e) = tb::buffer(vec![0u8; i]);
    let counter = Arc::new(AtomicU64::new(0));
    let counter_st = counter.clone();
    let counter_ld = counter.clone();
    let e_th = std::thread::spawn(move || {
        for _ in 0..255 {
            e = e.grab_always();
            // do some editing
            let tb::RWPair { r: rb, w: wb } = e.fetch_unsafe();
            for (r, w) in rb.iter().zip(wb.iter_mut()) {
                *w = *r + 1;
            }
            counter_st.fetch_add(1, Ordering::AcqRel);
            e = e.commit_always();
        }
    });
    let scratch = AtomicU64::new(0);
    let r_th = std::thread::spawn(move || {
        let mut scratch;
        loop {
            r = r.grab_always();
            // do some reading
            for v in r.fetch_unsafe().iter() {
                scratch = *v;
            }
            r = r.release_always();
            if counter_ld.load(Ordering::Acquire) == 255 {
                break;
            }
        }
    });
    e_th.join().expect("Failed to join editing thread.");
    r_th.join().expect("Failed to join reading thread.");
}

fn tb_counter_raw(i: usize) {
    let e = tb::TripleBuffer::raw(vec![0u8; i]);
    let r = e.clone();
    let counter = Arc::new(AtomicU64::new(0));
    let counter_st = counter.clone();
    let counter_ld = counter.clone();
    let e_th = std::thread::spawn(move || {
        for _ in 0..255 {
            // do some editing
            let rb = e.editor_r();
            let wb = e.editor_w();
            for (r, w) in rb.iter().zip(wb.iter_mut()) {
                *w = *r + 1;
            }
            e.advance();
            counter_st.fetch_add(1, Ordering::AcqRel);
        }
    });
    let scratch = AtomicU64::new(0);
    let r_th = std::thread::spawn(move || {
        let mut scratch;
        loop {
            r.snatch();
            for v in r.reader_r().iter() {
                scratch = *v;
            }
            if counter_ld.load(Ordering::Acquire) == 255 {
                break;
            }
        }
    });
    e_th.join().expect("Failed to join editing thread.");
    r_th.join().expect("Failed to join reading thread.");
}

fn criterion_benchmark(c: &mut Criterion) {
    {
        c.bench_function("00. Creation", |b| b.iter(|| tb::buffer(())));
        let (r, _) = tb::buffer(());
        c.bench_function(
            "01. Reading",
            move |b| b.iter_batched(
                || tb::buffer(()).0,
                |e| tb_read(e),
                BatchSize::SmallInput,
            ),
        );
        c.bench_function(
            "02. Reading /w Interleaved Edits",
            move |b| b.iter_batched(
                || {
                    let (r, e) = tb::buffer(());
                    tb_edit(e);
                    r
                },
                |r| tb_read(r),
                BatchSize::SmallInput,
            ),
        );
        let (_, e) = tb::buffer(());
        c.bench_function(
            "03. Editing",
            move |b| b.iter_batched(
                || tb::buffer(()).1,
                |e| tb_edit(e),
                BatchSize::SmallInput,
            ),
        );
        c.bench_function(
            "04. Editing w/ Interleaved Reads",
            move |b| b.iter_batched(
                || {
                    let (r, e) = tb::buffer(());
                    tb_read(r);
                    e
                },
                |e| tb_edit(e),
                BatchSize::SmallInput,
            ),
        );
        c.bench_function_over_inputs(
            "05. 2-Threaded Usage With Varying Editing Workloads",
            |b, i| b.iter(|| tb_counter(**i)),
            &[0, 1, 5, 10, 50, 100, 1000]
        );
    }
    {
        c.bench_function("06. Raw Creation", |b| b.iter(|| tb::buffer(vec![0u8; 5000])));
        {
            let e = tb::TripleBuffer::raw(());
            tb_raw_read(&e);
            c.bench_function("07. Raw Reading", move |b| b.iter(|| tb_raw_read(&e)));
        }
        c.bench_function(
            "08. Raw Reading w/ Interleaved Edits",
            move |b| b.iter_batched_ref(
                || {
                    let e = tb::TripleBuffer::raw(());
                    tb_raw_edit(&e);
                    e
                },
                |e| tb_raw_read(e),
                BatchSize::SmallInput,
            ),
        );
        {
            let e = tb::TripleBuffer::raw(());
            tb_raw_edit(&e);
            c.bench_function("09. Raw Editing", move |b| b.iter(|| tb_raw_edit(&e)));
        }
        c.bench_function(
            "10. Raw Editing w/ Interleaved Reads",
            move |b| b.iter_batched_ref(
                || {
                    let e = tb::TripleBuffer::raw(());
                    tb_raw_read(&e);
                    e
                },
                |e| tb_raw_edit(e),
                BatchSize::SmallInput,
            ),
        );
        c.bench_function_over_inputs(
            "11. Raw 2-Threaded Usage With Varying Editing Workloads",
            |b, i| b.iter(|| tb_counter_raw(**i)),
            &[0, 1, 5, 10, 50, 100, 1000]
        );
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
