use std::{sync::atomic::{AtomicBool, Ordering}, cell::UnsafeCell, hint, ops::{Deref, DerefMut}};

pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>, // UnsafeCell for 内部可变性
}

pub struct SpinLockGuard<'a, T> {
    spin_lock: &'a SpinLock<T>
}

unsafe impl<T> Sync for SpinLock<T> where T: Send {}

impl<T> SpinLock<T> {
    pub fn new(data: T) -> Self {
        SpinLock { locked: AtomicBool::new(false), data: UnsafeCell::new(data) }
    }
   pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) == true {
            hint::spin_loop();
        }
        SpinLockGuard { spin_lock: self }
   } 

   fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
   }
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe {
            & *self.spin_lock.data.get()
        }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut *self.spin_lock.data.get()
        }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.spin_lock.unlock();
    } 
}