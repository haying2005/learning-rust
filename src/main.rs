use std::cell::{RefCell, UnsafeCell, Cell};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{sync::atomic::AtomicBool, thread};
use std::sync::atomic::Ordering::{Relaxed, Acquire, Release, self};
use std::sync::atomic::{AtomicPtr, AtomicUsize, compiler_fence};

use hello_world::one_shot1::OneShot;
use hello_world::spin_lock::SpinLock;




fn get_data() -> &'static Vec<i32> {
    static PTR: AtomicPtr<Vec<i32>> = AtomicPtr::new(std::ptr::null_mut());

    let mut p = PTR.load(Acquire); // acquire防止读取到p，但是指向的数据读不到

    if p.is_null() {
        p = Box::into_raw(Box::new(vec![]));
        if let Err(e) = PTR.compare_exchange(
            std::ptr::null_mut(), p, Release, Acquire
        ) {
            // Safety: p comes from Box::into_raw right above,
            // and wasn't shared with any other thread.
            drop(unsafe { Box::from_raw(p) }); // 释放之前初始化的数据（指针指向的数据）
            p = e;
        }
    }

    // Safety: p is not null and points to a properly initialized value.
    unsafe { 
        &*p 
    }
}

struct A {
    a: Option<Vec<i32>>,
    b: MaybeUninit<Vec<i32>>,
    c: Vec<i32>,
}

fn main() {
    let locked = AtomicBool::new(false);
    let counter = AtomicUsize::new(0);

    thread::scope(|s| {
        // Spawn four threads, that each iterate a million times.
        for _ in 0..4 {
            s.spawn(|| for _ in 0..1_000_000 {
                // Acquire the lock, using the wrong memory ordering.
                while locked.swap(true, Relaxed) {}
                compiler_fence(Acquire);

                // Non-atomically increment the counter, while holding the lock.
                let old = counter.load(Relaxed);
                let new = old + 1;
                counter.store(new, Relaxed);

                // Release the lock, using the wrong memory ordering.
                compiler_fence(Release);
                locked.store(false, Relaxed);
            });
        }
    });

    println!("{}", counter.into_inner());

}