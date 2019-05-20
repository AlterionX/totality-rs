#[macro_use]
extern crate criterion;

use criterion::{black_box, Criterion};

extern crate totality_sync as sync;
use sync::triple_buffer as tb;

fn tb_swap_read(rv: &mut Option<tb::Reader<Vec<u8>>>) {
    let rv_int = rv.take().unwrap().grab().expect("How does this happend?").release().expect("And this, too.");
    rv.replace(rv_int);
}
fn tb_swap_edit(ev: &mut Option<tb::Editor<Vec<u8>>>) {
    let ev_int = ev.take().unwrap().grab().expect("How does this happen?").commit().expect("And this, too.");
    ev.replace(ev_int);
}
fn tb_light() {
    let (mut r, mut e) = tb::buffer(vec![0u8; 1000]);
    let e_th = std::thread::spawn(move || {
        for _ in 0..255 {
            e = e.grab().unwrap();
            // do some editing
            let tb::RWPair { r: rb, w: wb } = e.fetch_unsafe();
            for (r, w) in rb.iter().zip(wb.iter_mut()) {
                *w = *r + 1;
            }
            e = e.commit().unwrap();
        }
    });
    let r_th = std::thread::spawn(move || {
        let mut scratch = 0;
        loop {
            r = r.grab().expect("Nope");
            // do some reading
            for v in r.fetch_unsafe().iter() {
                scratch = *v;
            }
            r = r.release().expect("Nope");
            if scratch == 255 {
                break;
            }
        }
    });
    e_th.join().expect("Failed to join editing thread.");
    r_th.join().expect("Failed to join reading thread.");
}
fn tb_heavy() {
    let (mut r, mut e) = tb::buffer(vec![0u8; 1_000_000]);
    let e_th = std::thread::spawn(move || {
        for _ in 0..255 {
            e = e.grab().unwrap();
            // do some editing
            let tb::RWPair { r: rb, w: wb } = e.fetch_unsafe();
            for (r, w) in rb.iter().zip(wb.iter_mut()) {
                *w = *r + 1;
            }
            e = e.commit().unwrap();
        }
    });
    let r_th = std::thread::spawn(move || {
        let mut scratch = 0;
        loop {
            r = r.grab().expect("Nope");
            // do some reading
            for v in r.fetch_unsafe().iter() {
                scratch = *v;
            }
            r = r.release().expect("Nope");
            if scratch == 255 {
                break;
            }
        }
    });
    e_th.join().expect("Failed to join editing thread.");
    r_th.join().expect("Failed to join reading thread.");
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("0. Creation", |b| b.iter(|| tb::buffer(vec![0u8; 5000])));
    let (rv, ev) = tb::buffer(vec![0u8; 5000]);
    let (mut rv, mut ev) = (Some(rv), Some(ev));
    c.bench_function("1. Reading Usage", move |b| {
        b.iter(|| tb_swap_read(&mut rv))
    });
    c.bench_function("2. Editing Usage", move |b| {
        b.iter(|| tb_swap_edit(&mut ev))
    });
    c.bench_function("3. Light Usage", |b| b.iter(|| tb_light()));
    // c.bench_function("4. Heavy Usage", |b| b.iter(|| tb_heavy()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
