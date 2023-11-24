use std::{
    cell::UnsafeCell,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{fence, AtomicUsize, Ordering},
};

struct ArcData<T> {
    /// Number of `Arc`s.
    data_ref_count: AtomicUsize,
    /// Number of `Arc`s and `Weak`s combined.
    alloc_ref_count: AtomicUsize, // 强弱引用之和
    // UnsafeCell for 内部可变性。当data_ref_count变味0时，data设置为None
    data: UnsafeCell<Option<T>>,
}

pub struct Weak<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Weak<T> {}
unsafe impl<T: Send + Sync> Sync for Weak<T> {}

pub struct Arc<T> {
    weak: Weak<T>,
}

impl<T> Arc<T> {
    pub fn new(val: T) -> Self {
        let arc_data = ArcData {
            data_ref_count: AtomicUsize::new(1),
            alloc_ref_count: AtomicUsize::new(1),
            data: UnsafeCell::new(Some(val)),
        };
        Arc {
            weak: Weak {
                ptr: NonNull::from(Box::leak(Box::new(arc_data))),
            },
        }
    }

    /// We need to establish a happens-before relationship with every single drop that led to the reference counter being one.
    /// This only matters when the reference counter is actually one; if it’s higher, we’ll not provide a &mut T, and the memory ordering is irrelevant.
    /// it can only be called as Arc::get_mut(&mut a), and not as a.get_mut(). This is advisable for types that implement Deref,
    /// to avoid ambiguity with a similarly named method on the underlying T.
    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        // 只检查strong pointer数量不行，因为weak pointer可以随时upgrade为strong pointer
        if arc.weak.data().alloc_ref_count.load(Ordering::Relaxed) == 1 {
            fence(Ordering::Acquire);
            let prt: *mut Option<T> = arc.weak.data().data.get();
            // 因为self是Arc，所以data不可能为None
            return unsafe { Some((&mut *prt).as_mut().unwrap()) };
        }
        None
    }

    pub fn downgrade(arc: &Self) -> Weak<T> {
        arc.weak.clone()
    }
}

impl<T> Weak<T> {
    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }
    pub fn upgrade(&self) -> Option<Arc<T>> {
        let mut data_ref_count = self.data().data_ref_count.load(Ordering::Relaxed);
        loop {
            if data_ref_count == 0 {
                return None;
            }
            assert!(data_ref_count < usize::MAX / 2);
            if let Err(e) = self.data().data_ref_count.compare_exchange_weak(data_ref_count, data_ref_count + 1, Ordering::Relaxed, Ordering::Relaxed) {
                data_ref_count = e;
                continue;
            }
            return Some(Arc { weak: self.clone() })
        }
    }
}
impl<T> Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let ptr = self.weak.data().data.get();
        // Safety: Since there's an Arc to the data,
        // the data exists and may be shared.
        unsafe { (&*ptr).as_ref().unwrap() }
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        if self
            .data()
            .alloc_ref_count
            .fetch_add(1, Ordering::Relaxed)
            > usize::MAX / 2
        {
            std::process::abort();
        }
        Weak { ptr: self.ptr }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        let weak = self.weak.clone();
        if self
            .weak
            .data()
            .data_ref_count
            .fetch_add(1, Ordering::Relaxed)
            > usize::MAX / 2
        {
            std::process::abort();
        }
        Arc { weak }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        let data_ref_count = self
            .weak
            .data()
            .data_ref_count
            .fetch_sub(1, Ordering::Release);
        if data_ref_count == 1 {
            // there are still weak pointer
            // set Option to None
            fence(Ordering::Acquire);
            let ptr = self.weak.data().data.get();
            unsafe { *ptr = None };
        }
    }
}

impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        let alloc_ref_count = self.data().alloc_ref_count.fetch_sub(1, Ordering::Release);
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
        NUM_DROPS.store(0, Ordering::Relaxed);
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
        NUM_DROPS.store(0, Ordering::Relaxed);
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
