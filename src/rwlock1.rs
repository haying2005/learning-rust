use std::{sync::atomic::{AtomicU32, Ordering}, cell::UnsafeCell, ops::{DerefMut, Deref}};

use atomic_wait::{wait, wake_one, wake_all};

struct Rwlock<T> {
    /// 0 unlocked
    /// n 表示当前有n个读锁
    /// u32::MAX 代表当前处于写锁状态
    state: AtomicU32,
    /// 自增 写操作唤醒次数
    writer_wake_counter: AtomicU32,
    value: UnsafeCell<T>
}

// 类似mutex，T需要Send（&mut T可以获得所有权，通过mem::swap），
// T还需要Sync，因为多个线程可以同时持有读锁（&T）
unsafe impl<T> Sync for Rwlock<T> where T: Sync + Send {}

impl<T> Rwlock<T> {
    pub const fn new(val: T) -> Self {
        Self { state: AtomicU32::new(0), value: UnsafeCell::new(val), writer_wake_counter: AtomicU32::new(0) }
    }

    pub fn read(&self) -> ReadGuard<T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s < u32::MAX {
                assert!(s < u32::MAX - 1, "too many readers!");
                match self.state.compare_exchange_weak(s, s+1, Ordering::Acquire, Ordering::Relaxed) {
                    Ok(_) => return ReadGuard {
                        rwlock: self
                    },
                    Err(old) => s = old,
                }
            }
            if s == u32::MAX {
                // 当前处于写锁状态
                wait(&self.state, s);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }

    pub fn write(&self) -> WriteGuard<T> {
        // 不希望当前值为0(成功匹配)的时候进行无必要的wait，所以用compare_exchange而不是compare_exchange_weak
        // 因为compare_exchange_weak可能出现值匹配上但依然返回Err的情况
        while let Err(s) = self.state.compare_exchange(
            0, u32::MAX, Ordering::Acquire, Ordering::Relaxed
        ) {
            // acquire和drop方法里的release fetch_add形成happens-before关系
            // 确保下面的state load方法读到的是drop方法内fetch_sub之后(或者store为0之后)的值
            // 否则可能错误的进入wait阻塞，导致错过wake-up从而永远阻塞无法获取锁
            let w = self.writer_wake_counter.load(Ordering::Acquire);
            if self.state.load(Ordering::Relaxed) > 0 {
                // 等待writer_wake_counter避免过多的读操作导致的state频繁变动，从而导致wait一直不block(busy loop)
                wait(&self.writer_wake_counter, w);
            }
        }
        WriteGuard { rwlock: self }
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
        let s = self.rwlock.state.fetch_sub(1, Ordering::Release);
        if s == 1 {
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