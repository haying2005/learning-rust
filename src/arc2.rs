use std::{
    cell::UnsafeCell,
    mem::ManuallyDrop,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{fence, AtomicUsize, Ordering}, hint,
};

struct ArcData<T> {
    /// Number of `Arc`s.
    strong_ref_count: AtomicUsize,
    /// Number of `Arc`s and `Weak`s combined.
    /// 强弱引用之和, 所有强引用加起来等于1个弱引用，当data_ref_count变为0时，alloc_ref_count减1
    weak_ref_count: AtomicUsize,
    // UnsafeCell for 内部可变性。当data_ref_count变味0时，data设置为None
    data: UnsafeCell<ManuallyDrop<T>>,
}

pub struct Weak<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Weak<T> {}
unsafe impl<T: Send + Sync> Sync for Weak<T> {}

pub struct Arc<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

impl<T> Arc<T> {
    pub fn new(val: T) -> Self {
        let arc_data = ArcData {
            strong_ref_count: AtomicUsize::new(1),
            weak_ref_count: AtomicUsize::new(1),
            data: UnsafeCell::new(ManuallyDrop::new(val)),
        };
        Arc {
            ptr: NonNull::from(Box::leak(Box::new(arc_data))),
        }
    }
    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }

    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        // weak_ref_count设置为usize::MAX可以暂时阻止downgrade(weak_ref_count原始值为1说明当前只有Arc，没有Weak)
        // weak_ref_count当前值为1说明没有weak ref（因为arc是strong ref，如果有weak ref的话，weak_ref_count至少为2）
        if arc
            .data()
            .weak_ref_count
            .compare_exchange(1, usize::MAX, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return None;
        }
        // strong_ref_count为1说明当前arc为唯一引用
        let is_unique = arc.data().strong_ref_count.load(Ordering::Relaxed) == 1;
        // 恢复weak_ref_count为原始值1
        arc.data().weak_ref_count.store(1, Ordering::Release);
        if !is_unique {
            return None;
        }
        // 此屏障作用于前面的strong_ref_count.load 读取到strong_ref_count从2变为1的Arc Drop的release操作
        // 确保此arc drop之前对数据的操作不会出现在屏障之后（因为屏障之后返回独占借用&mut T）
        fence(Ordering::Acquire);
        unsafe { Some(&mut *arc.data().data.get()) }
    }

    pub fn downgrade(arc: &Self) -> Weak<T> {
        let mut weak_ref_count = arc.data().weak_ref_count.load(Ordering::Relaxed);
        loop {
            if weak_ref_count == usize::MAX {
                hint::spin_loop();
                // 当前处于get_mut锁定阶段
                weak_ref_count = arc.data().weak_ref_count.load(Ordering::Relaxed);
                continue;
            }
            assert!(weak_ref_count < usize::MAX / 2);
            if let Err(e) = arc.data().weak_ref_count.compare_exchange_weak(
                weak_ref_count,
                weak_ref_count + 1,
                Ordering::Acquire, // 配对get_mut里面解锁操作
                Ordering::Relaxed,
            ) {
                weak_ref_count = e;
                continue;
            }
            return Weak { ptr: arc.ptr };
        }
    }
}

impl<T> Weak<T> {
    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }
    pub fn upgrade(&self) -> Option<Arc<T>> {
        let mut data_ref_count = self.data().strong_ref_count.load(Ordering::Relaxed);
        loop {
            if data_ref_count == 0 {
                return None;
            }
            assert!(data_ref_count < usize::MAX / 2);
            if let Err(e) = self.data().strong_ref_count.compare_exchange_weak(
                data_ref_count,
                data_ref_count + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                data_ref_count = e;
                continue;
            }
            return Some(Arc { ptr: self.ptr });
        }
    }
}
impl<T> Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let ptr = self.data().data.get();
        // Safety: Since there's an Arc to the data,
        // the data exists and may be shared.
        unsafe { &*ptr }
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        if self.data().weak_ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Weak { ptr: self.ptr }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        if self.data().strong_ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Arc { ptr: self.ptr }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        let data_ref_count = self.data().strong_ref_count.fetch_sub(1, Ordering::Release);
        if data_ref_count == 1 {
            fence(Ordering::Acquire);
            // there is no strong pointer now
            let ptr = self.data().data.get();
            unsafe {
                let x = &mut *ptr;
                ManuallyDrop::drop(x)
            };
            // 把weak_ref_count 减1
            drop(Weak { ptr: self.ptr });
        }
    }
}

impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        let alloc_ref_count = self.data().weak_ref_count.fetch_sub(1, Ordering::Release);
        if alloc_ref_count == 1 {
            fence(Ordering::Acquire);
            // drop ArcData
            // 使用acquire屏障 和之前每一次drop的fetch_sub操作建立happens-before关系，
            // 确保释放操作发生在ref_count最后一次fetch_sub之后
            let b = unsafe { Box::from_raw(self.ptr.as_ptr()) };
            drop(b);
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        sync::atomic::{AtomicU8, Ordering},
        thread,
    };

    use super::Arc;

    #[test]
    fn test() {
        static NUM_DROPS: AtomicU8 = AtomicU8::new(0);
        struct DetectDrops;
        impl Drop for DetectDrops {
            fn drop(&mut self) {
                NUM_DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }
        let mut x = Arc::new(("xxx", DetectDrops));
        let ref_x = Arc::get_mut(&mut x).unwrap();
        ref_x.0 = "hello";
        let y = Arc::downgrade(&x);
        let z = Arc::downgrade(&x);
        assert!(Arc::get_mut(&mut x).is_none());
        let xx = x.clone();
        let handler = thread::spawn(move || {
            let mut yy = y.upgrade().unwrap();
            assert_eq!(yy.0, "hello");
            assert!(Arc::get_mut(&mut yy).is_none());
            assert_eq!(xx.0, "hello");
        });

        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);
        handler.join().unwrap();
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);
        drop(x);
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 1);

        // arc ref count has been 0
        assert!(z.upgrade().is_none())
    }

    #[test]
    fn test_multiple_threads() {
        static NUM_DROPS: AtomicU8 = AtomicU8::new(0);
        struct DetectDrops;
        impl Drop for DetectDrops {
            fn drop(&mut self) {
                NUM_DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }
        let x = Arc::new(("hello", DetectDrops));
        let y = Arc::downgrade(&x);
        let z = Arc::downgrade(&x);

        let mut v = vec![];
        for _ in 0..1000 {
            let xx = x.clone();
            v.push(thread::spawn(move || {
                assert_eq!(xx.0, "hello");
            }));
        }
        for _ in 0..1000 {
            let xx = y.clone();
            v.push(thread::spawn(move || {
                assert_eq!(xx.upgrade().unwrap().0, "hello");
            }));
        }
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);
        for handler in v {
            handler.join().unwrap();
        }
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);

        drop(x);
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 1);

        assert!(z.upgrade().is_none());
    }
}
