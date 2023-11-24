use std::{cell::UnsafeCell, mem::MaybeUninit, sync::{atomic::{AtomicBool, Ordering}, Arc}, thread::{Thread, self}, marker::PhantomData};

// 此通道只能在创建线程receive
pub struct OneShot<T> {
    data: UnsafeCell<MaybeUninit<T>>, // MaybeUninit需要手动释放，并且需要程序员追踪其是否初始化
    ready: AtomicBool,
}

impl<T> OneShot<T> {
    pub fn new() -> Self {
        OneShot { data: UnsafeCell::new(MaybeUninit::uninit()), ready: AtomicBool::new(false) }
    }

    // 不可变引用保证在sender和receiver存活期间，无法进行第二次调用（当他们都drop之后允许第二次调用）
    pub fn split<'a>(&'a mut self) -> (Sender<'a, T>, Receiver<'a, T>) {
        // 多次调用时，drop掉老的对象(已经是脏数据)，创建新的
        *self = Self::new();
        (Sender {
            channel: self, receiving_thread: thread::current(),
        }, Receiver {
            channel: self, _no_send: PhantomData
        })
    }
}

impl<T> Drop for OneShot<T> {
    fn drop(&mut self) {
        // drop方法不需要原子操作。drop被调用说明当前线程独占（拥有所有权且没有其他借用）
        if *self.ready.get_mut() {
            // 只有在有数据且没有被receive的情况下才drop， 否则会double free，引起panic
            println!("onshot data has inited, start dropping...");
            unsafe { (*self.data.get()).assume_init_drop() }
        }
    }
}

unsafe impl<T: Send> Sync for OneShot<T> {}

pub struct Sender<'a, T> {
    channel: &'a OneShot<T>,
    receiving_thread: Thread, // 接收端的线程，用于unpark
}

impl<T> Sender<'_, T> {
    pub fn send(self, val: T) {
        unsafe { (*self.channel.data.get()).write(val) };
        self.channel.ready.store(true, Ordering::Release);
        self.receiving_thread.unpark();
    }
}


pub struct Receiver<'a, T> {
    channel: &'a OneShot<T>,
    _no_send: PhantomData<*const ()>, // !Send, 防止被发送到其他thread，造成sender unpark原始线程无效
}

impl<T> Receiver<'_, T> {
    pub fn is_ready(&self) -> bool {
        self.channel.ready.load(Ordering::Relaxed)
    }
    pub fn receive(self) -> T {
        while !self.channel.ready.swap(false, Ordering::Acquire) {
            thread::park();
        }
        unsafe {
            (*self.channel.data.get()).assume_init_read()
        }
    }
}

