use std::thread;
use std::{sync::atomic::AtomicU64, time::Instant};
use std::hint::black_box;
use std::sync::atomic::Ordering::{Relaxed};

// only takes ref types bounded by 'a
fn t_ref<'a, T: 'a>(t: &'a T) {}

// takes any types bounded by 'a
fn t_bound<'a, T: 'a>(t: T) {}

// owned type which contains a reference
struct Ref<'a, T: 'a>(&'a T);

// type XX<'a> = &'a Ref<'a, String>;

#[repr(align(64))] // This struct must be 64-byte aligned.
struct Aligned(AtomicU64);

static A: [Aligned; 3] = [
    Aligned(AtomicU64::new(0)),
    Aligned(AtomicU64::new(0)),
    Aligned(AtomicU64::new(0)),
];

fn main() {
    black_box(&A);
    thread::spawn(|| {
        loop {
            A[0].0.store(1, Relaxed);
            A[2].0.store(1, Relaxed);
        }
    });
    let start = Instant::now();
    for _ in 0..1_000_000_000 {
        black_box(A[1].0.load(Relaxed));
    }
    println!("{:?}", start.elapsed());
}