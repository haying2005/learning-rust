use std::{cell::UnsafeCell, mem::MaybeUninit, sync::atomic::{AtomicBool, Ordering}};

pub struct OneShot<T> {
    data: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
    in_use: AtomicBool,
}

impl<T> OneShot<T> {
    pub fn new() -> Self {
        OneShot { data: UnsafeCell::new(MaybeUninit::uninit()), ready: AtomicBool::new(false), in_use: AtomicBool::new(false) }
    }
    pub fn send(&self, val: T) {
        if self.in_use.swap(true, Ordering::Acquire) {
            panic!("already in use")
        }
        unsafe { (*self.data.get()).write(val) };
        self.ready.store(true, Ordering::Release);
    }

    pub fn receive(&self) -> T {
        if !self.ready.swap(false, Ordering::Acquire) {
            panic!("not ready yet")
        }
        unsafe {
            (*self.data.get()).assume_init_read()
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }
}

impl<T> Drop for OneShot<T> {
    fn drop(&mut self) {
        println!("onshot start dropping...");
        if *self.ready.get_mut() {
            // 只有在有数据且没有被receive的情况下才drop， 否则会double free，引起panic
            unsafe { (*self.data.get()).assume_init_drop() }
        }
    }
}

unsafe impl<T: Send> Sync for OneShot<T> {}

