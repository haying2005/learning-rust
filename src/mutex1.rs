use std::{sync::atomic::{Ordering, AtomicU32}, cell::UnsafeCell, ops::{Deref, DerefMut}};

use atomic_wait::{ wait, wake_one };

pub struct Mutex<T> {
    /// 0: unlocked
    /// 1: locked without waiters
    /// 2: locked with waiters
    locked: AtomicU32,
    data: UnsafeCell<T>, // UnsafeCell for 内部可变性
}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>
}

unsafe impl<T> Sync for Mutex<T> where T: Send {} // T必须满足Send，因为可以从data中获取T的拷贝，例如通过mem::swap

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        Mutex { locked: AtomicU32::new(0), data: UnsafeCell::new(data) }
    }
   pub fn lock(&self) -> MutexGuard<'_, T> {
        if self.locked.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // 加锁失败
            // 不是百分百完美：每次都把状态设置为2，不管有没有其他线程在等待，在这种情况下获得锁之后，unlock的时候会引起一次不必要的wake_one系统调用
            while self.locked.swap(2, Ordering::Acquire) != 0 {
                wait(&self.locked, 2); // 此操作预期值判定与block是原子操作，不存在并发问题
            }
        }
        
        MutexGuard { mutex: self }
   } 

   fn unlock(&self) {
        if self.locked.swap(0, Ordering::Release) == 2 {
            // 只有状态为2的时候，才进行wake，减少系统调用
            wake_one(&self.locked);
        }
        
   }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe {
            & *self.mutex.data.get()
        }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut *self.mutex.data.get()
        }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.unlock();
    } 
}


#[cfg(test)]
mod test {
    use std::time::Instant;

    use super::Mutex;
    #[test]
    fn test_bench() {
        let m = Mutex::new(0);
        std::hint::black_box(&m);
        let start = Instant::now();
        for _ in 0..5000000  {
            *m.lock() += 1;
        }
        assert_eq!(*m.lock(), 5000000);
        let duration = start.elapsed();
        println!("time used {:?}", duration);
    }
}