#[macro_use]
extern crate criterion;

use criterion::{Criterion, black_box};

extern crate totality_sync as sync;
use sync::triple_buffer as tb;

fn tb_create(n: u64) -> (tb::ReadingView<Vec<u8>>, tb::EditingView<Vec<u8>>) {
    tb::buffer(vec![0; n as usize])
}
fn tb_swap_read(rv: &mut Option<tb::ReadingView<Vec<u8>>>) {
    let rv_int = rv.take().unwrap().read().release();
    rv.replace(rv_int);
}
fn tb_swap_edit(ev: &mut Option<tb::EditingView<Vec<u8>>>) {
    let ev_int = ev.take().unwrap().edit().release();
    ev.replace(ev_int);
}
fn tb_light() {
    let (rv, ev) = tb_create(1_000);
    let e_th = std::thread::spawn(move || {
        let mut ev = Some(ev);
        let mut e = None;
        for _ in 0..255 {
            e.replace(ev.take().unwrap().edit());
            // do some editing
            let e_int = e.as_ref().unwrap();
            for (rv, ev) in e_int.r().iter().zip(e_int.w().iter_mut()) {
                *ev = *rv + 1;
            }
            ev.replace(e.take().unwrap().release());
        }
    });
    let r_th = std::thread::spawn(move || {
        let mut rv = Some(rv);
        let mut r = None;
        let mut scratch = 0;
        loop {
            r.replace(rv.take().unwrap().read());
            // do some reading
            for v in r.as_ref().unwrap().r().iter() {
                scratch = *v;
            }
            rv.replace(r.take().unwrap().release());
            if scratch == 255 {
                break
            }
        }
    });
    e_th.join().expect("Failed to join editing thread.");
    r_th.join().expect("Failed to join reading thread.");
}
fn tb_heavy() {
    let (rv, ev) = tb_create(1_000_000_000);
    let e_th = std::thread::spawn(move || {
        let mut ev = Some(ev);
        let mut e = None;
        for _ in 0..7 {
            e.replace(ev.take().unwrap().edit());
            // do some editing
            let e_int = e.as_ref().unwrap();
            for (rv, ev) in e_int.r().iter().zip(e_int.w().iter_mut()) {
                *ev = *rv + 1;
            }
            ev.replace(e.take().unwrap().release());
        }
    });
    let r_th = std::thread::spawn(move || {
        let mut rv = Some(rv);
        let mut r = None;
        let mut scratch = 0;
        loop {
            r.replace(rv.take().unwrap().read());
            // do some reading
            for v in r.as_ref().unwrap().r().iter() {
                scratch = *v;
            }
            rv.replace(r.take().unwrap().release());
            if scratch == 7 {
                break
            }
        }
    });
    e_th.join().expect("Failed to join editing thread.");
    r_th.join().expect("Failed to join reading thread.");
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("0. Creation", |b| b.iter(|| tb_create(black_box(5000))));
    let (rv, ev) = tb_create(1_000_000_000);
    let (mut rv, mut ev) = (Some(rv), Some(ev));
    c.bench_function("1. Reading Usage", move |b| b.iter(|| tb_swap_read(&mut rv)));
    c.bench_function("2. Editing Usage", move |b| b.iter(|| tb_swap_edit(&mut ev)));
    c.bench_function("3. Light Usage", |b| b.iter(|| tb_light()));
    // c.bench_function("4. Heavy Usage", |b| b.iter(|| tb_heavy()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

