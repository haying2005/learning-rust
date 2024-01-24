use std::{sync::atomic::{AtomicU32, Ordering}, cell::UnsafeCell, ops::{DerefMut, Deref}};

use atomic_wait::{wait, wake_one, wake_all};


// 优化：避免写饥饿，写优先
struct Rwlock<T> {
    /// 0 unlocked
    /// u32::MAX 代表当前处于写锁状态
    /// 最低位代表是否有线程等待写锁，其余位代表读锁数量；即读锁数量 = state / 2
    state: AtomicU32,
    /// 自增 写操作唤醒次数
    writer_wake_counter: AtomicU32,
    value: UnsafeCell<T>
}

// 类似mutex，T需要Send（&mut T可以获得所有权，例如通过mem::swap），
// T还需要Sync，因为多个线程可以同时持有读锁（&T）
unsafe impl<T> Sync for Rwlock<T> where T: Sync + Send {}

impl<T> Rwlock<T> {
    pub const fn new(val: T) -> Self {
        Self { state: AtomicU32::new(0), value: UnsafeCell::new(val), writer_wake_counter: AtomicU32::new(0) }
    }

    pub fn read(&self) -> ReadGuard<T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s % 2 == 0 { // 偶数，说明当前没有线程持有或等待写锁，可以获得读锁
                assert!(s < u32::MAX - 2, "too many readers!");
                match self.state.compare_exchange_weak(s, s+2, Ordering::Acquire, Ordering::Relaxed) {
                    Ok(_) => return ReadGuard {
                        rwlock: self
                    },
                    Err(old) => s = old,
                }
            }
            if s % 2 == 1 { // 奇数，说明当前有线程持有或等待写锁
                // 当前处于写锁状态
                wait(&self.state, s);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }

    pub fn write(&self) -> WriteGuard<T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s < 2 {
                // 当前没有读锁，可尝试获取写锁
                match self.state.compare_exchange(s, u32::MAX, Ordering::Acquire, Ordering::Relaxed) {
                    Ok(_) => return WriteGuard {
                        rwlock: self,
                    },
                    Err(old) => {
                        s = old;
                        continue;
                    }
                }
            }

            // 当前有读锁，尝试进入wait

            if s % 2 == 0 {
                // wait之前先把最低位置为1，阻止写锁的获取
                match self.state.compare_exchange(s, s + 1, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => {
                        // do nothing, 进入下一步：wait
                    },
                    Err(old) => {
                        s = old;
                        continue;
                    }
                }
            }
            // acquire和drop方法里的release fetch_add形成happens-before关系
            // 确保下面的state load方法读到的是drop方法内fetch_sub之后(或者store为0之后)的值
            // 否则可能错误的进入wait阻塞，导致错过wake-up从而永远阻塞无法获取锁
            let w = self.writer_wake_counter.load(Ordering::Acquire);
            if self.state.load(Ordering::Relaxed) > 1  {
                wait(&self.writer_wake_counter, w);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }
}


pub struct ReadGuard<'a, T> {
    rwlock: &'a Rwlock<T>
}

impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        let s = self.rwlock.state.fetch_sub(2, Ordering::Release);
        if s == 3 { // 最低位为1才wake，说明有线程等待写锁
            self.rwlock.writer_wake_counter.fetch_add(1, Ordering::Release);
            // 唤醒1个等待写锁的线程
            wake_one(&self.rwlock.writer_wake_counter);
        }
    }
}

pub struct WriteGuard<'a, T> {
    rwlock: &'a Rwlock<T>
}

impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<T> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rwlock.value.get() }
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.rwlock.state.store(0, Ordering::Release);
        self.rwlock.writer_wake_counter.fetch_add(1, Ordering::Release);
        wake_one(&self.rwlock.writer_wake_counter);
        wake_all(&self.rwlock.state);
    }
}