use std::sync::atomic::{
    AtomicU32,
    Ordering,
};

use atomic_wait::{ wait, wake_one, wake_all };

use crate::mutex2::MutexGuard;


pub struct Codvar {
    counter: AtomicU32,
    num_waiters: AtomicU32,
}

impl Codvar {
    pub fn new() -> Self {
        Codvar { counter: AtomicU32::new(0), num_waiters: AtomicU32::new(0) }
    }

    pub fn notify_one(&self) {
        // 复习之前的知识：load操作不保证能读取到其他线程最新修改的值！！
        // 虽然，load操作不能确保读到其他线程修改的最新值，但是有互斥锁的happens-before保障(notify操作必然发生在获取互斥锁之后)
        if self.num_waiters.load(Ordering::Relaxed) > 0 {
            self.counter.fetch_add(1, Ordering::Relaxed);
            wake_one(&self.counter);
        }
        
    }

    pub fn notify_all(&self) {
        if self.num_waiters.load(Ordering::Relaxed) > 0 {
            self.counter.fetch_add(1, Ordering::Relaxed);
            wake_all(&self.counter);
        }
    }

    pub fn wait<'a, T>(&self, mutex_guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        let cur_counter = self.counter.load(Ordering::Relaxed);
        let mutex = mutex_guard.mutex;

        // 此时仍然持有互斥锁，Relaxed就足够了，happend-before保障由互斥锁提供
        self.num_waiters.fetch_add(1, Ordering::Relaxed);

        drop(mutex_guard);
        // 不完美：wait可能因为counter的增加而不进入block状态；但是同时notify_one会导致另一个阻塞线程被唤醒，从而导致两个线程抢互斥锁
        wait(&self.counter, cur_counter);
        // wait之后的操作之可能在wait返回之后才会执行，不可能发生在wait之前读到sub后的结果0，导致wake不触发从而引起上面的wait永远无法唤醒
        self.num_waiters.fetch_sub(1, Ordering::Relaxed);
        mutex.lock()
    }
}

#[cfg(test)]
mod test {

    use std::{thread, time::Duration};

    use crate::mutex2::Mutex;

    use super::Codvar;

    #[test]
    fn test_codvar() {
        let mutex = Mutex::new(0);
        let codvar = Codvar::new();

        thread::scope(|s| {
            s.spawn(|| {
                thread::sleep(Duration::from_secs(1));
                *mutex.lock() = 123;
                codvar.notify_one();
            });
        });

        let mut m = mutex.lock();
        let mut wakeups = 0;
        while *m < 100 {
            m = codvar.wait(m);
            wakeups += 1;
        }

        println!("{}", wakeups);
        // 因为codvar会有虚假唤醒的可能性，所以唤醒次数可能大于1，但不应该太大
        assert!(wakeups < 10);

    }
}