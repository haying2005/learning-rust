use std::{
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{fence, AtomicUsize, Ordering},
};

struct ArcData<T> {
    ref_count: AtomicUsize,
    data: T,
}

pub struct Arc<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

impl<T> Arc<T> {
    pub fn new(val: T) -> Self {
        let arc_data = ArcData {
            ref_count: AtomicUsize::new(1),
            data: val,
        };
        Arc {
            ptr: NonNull::from(Box::leak(Box::new(arc_data))),
        }
    }

    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }

    /// We need to establish a happens-before relationship with every single drop that led to the reference counter being one.
    /// This only matters when the reference counter is actually one; if it’s higher, we’ll not provide a &mut T, and the memory ordering is irrelevant.
    /// it can only be called as Arc::get_mut(&mut a), and not as a.get_mut(). This is advisable for types that implement Deref,
    /// to avoid ambiguity with a similarly named method on the underlying T.
    fn get_mut(arc: &mut Self) -> Option<&mut T> {
        if arc.data().ref_count.load(Ordering::Relaxed) == 1 {
            fence(Ordering::Acquire);
            return unsafe { Some(&mut arc.ptr.as_mut().data) };
        }
        None
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data().data
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        if self.data().ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Arc { ptr: self.ptr }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        println!("dropping Arc");
        if self.data().ref_count.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            // drop the real data
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
                println!("dropping DetectDrops");
                NUM_DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }
        NUM_DROPS.store(0, Ordering::Relaxed);
        let x = Arc::new(("hello", DetectDrops));
        let xx = x.clone();

        let handler = thread::spawn(move || {
            assert_eq!(xx.0, "hello");
        });

        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);
        handler.join().unwrap();
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);
        drop(x);
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 1);
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

        let mut v = vec![];
        for _ in 0..1000 {
            let xx = x.clone();
            v.push(thread::spawn(move || {
                assert_eq!(xx.0, "hello");
            }));
        }
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);
        for handler in v {
            handler.join().unwrap();
        }
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);

        drop(x);
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 1);
    }
}
