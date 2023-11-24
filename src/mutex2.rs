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
    pub(crate) mutex: &'a Mutex<T>
}

unsafe impl<T> Sync for Mutex<T> where T: Send {} // T必须满足Send，因为可以从data中获取T的拷贝，例如通过mem::swap

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        Mutex { locked: AtomicU32::new(0), data: UnsafeCell::new(data) }
    }
   pub fn lock(&self) -> MutexGuard<'_, T> {
        if self.locked.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // 初次尝试加锁失败
            lock_contended(&self.locked);
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

#[cold]
fn lock_contended(state: &AtomicU32) {
    let mut spin_count = 0;
    // 自旋100次，当state为1的时候
    // state为0表示已经解锁，进入到下一阶段尝试解锁
    // state为2表示有其他线程已经放弃spin并进入到等待阶段，大概率会自旋失败
    // 此处用load而不用compare_exchange,因为compare_exchange会对cache_line进行独占访问，会降低性能
    while state.load(Ordering::Relaxed) == 1 && spin_count < 100 {
        std::hint::spin_loop();
        spin_count += 1;
    }

    // 最后一次尝试解锁并把state设置为1；因为进入到下一阶段只能把state设置为2；
    // 此处即使抢在其他等待线程被唤醒之前获取到锁，也不会因为state被设置为1而造成后续线程错过wake，因为后续线程抢锁失败后会负责把state设置为2
    if state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
        return;
    }

    // 进入等待阶段
    // 不是百分百完美：每次都把状态设置为2，因为有可能没有其他线程在等待，在这种情况下获得锁之后，unlock的时候会引起一次不必要的wake_one系统调用
    while state.swap(2, Ordering::Acquire) != 0 {
        wait(&state, 2); // 此操作预期值判定与进入休眠是原子操作，不存在并发问题
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