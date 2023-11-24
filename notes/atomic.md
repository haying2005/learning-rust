### Data Access问题
1. 默认情况下编译器和硬件并不会在意当前程序是单线程还是多线程
2. 对于编译器来说，数据访问顺序会被编译器随意reorder以进行优化
3. 对于硬件来说，数据修改只在每个cpu核心的缓存中进行，数据修改结果在线程之间的同步是惰性的，无序的（通常只有在本地缓存中不存在时，才会去进行同步）
4. !!!任何数据的修改（哪怕是atomic类型），也不能保证在其他线程立即被普通load读到。即 任何load操作不能保证能读到其他线程最新修改的值
5. 接上一条，但是fetch_add, swap, compare_exchange之类的操作是能保证获取的是最新值的

### Atomic Access
1. 原子访问内存顺序表明该访问与其他原子访问建立的顺序关系
2. 原子访问告诉编译器什么时候不能打乱访问顺序
3. 原子访问告诉cpu什么时候需要同步内存写操作


### 4种内存顺序
#### Relaxed
- 最宽松的内存顺序，在它之前和之后的操作都可以被重排，唯一保障的是操作本身是atomic的，适用于全局计数器

#### Sequentially Consistent 顺序一致
-  SeqCst就像是AcqRel的加强版，它不管原子操作是属于读取还是写入的操作，只要某个线程有用到SeqCst的原子操作，线程中该SeqCst操作前的数据操作绝对不会被重新排在该SeqCst操作之后，且该SeqCst操作后的数据操作也绝对不会被重新排在SeqCst操作前

#### Acquire and Release
##### acquire
- 在其他线程的视角(当建立因果关系)，所有在它之后的访问（包括原子访问与非原子）保持在它之后；但不保证在它之前的仍然保持在它之前
##### release
- 在其他线程的视角(当建立因果关系)，所有在它之前的访问（包括原子访问与非原子）保持在它之前；但不保证在它之后的仍然保持在它之后
##### acquire+release配合使用
- they're perfectly suited for acquiring and releasing locks, and ensuring that critical sections don't overlap.
```rust
// 自旋锁
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

fn main() {
    let lock = Arc::new(AtomicBool::new(false)); // value answers "am I locked?"

    // ... distribute lock to threads somehow ...

    // Try to acquire the lock by setting it to true
    while lock.compare_and_swap(false, true, Ordering::Acquire) { }
    // broke out of the loop, so we successfully acquired the lock!

    // ... scary data accesses ...

    // ok we're done, release the lock
    lock.store(false, Ordering::Release);
}
```
- 在线程A release了一块内存之后，线程B aquire了相同的内存，因果关系便建立了；所有发生在A线程release之前的写操作，在B aquire之后都能被B线程观察到。
```rust
use std::thread::{self, JoinHandle};
use std::sync::atomic::{Ordering, AtomicBool};

static mut DATA: u64 = 0;
static READY: AtomicBool = AtomicBool::new(false);

fn reset() {
    unsafe {
        DATA = 0;
    }
    READY.store(false, Ordering::Relaxed);
}

fn producer() -> JoinHandle<()> {
    thread::spawn(move || {
        unsafe {
            DATA = 100;                                 // A
        }
        READY.store(true, Ordering::Release);           // B: 内存屏障 ↑
    })
}

fn consumer() -> JoinHandle<()> {
    thread::spawn(move || {
        while !READY.load(Ordering::Acquire) {}         // C: 内存屏障 ↓

        assert_eq!(100, unsafe { DATA });               // D
    })
}


fn main() {
    loop {
        reset();

        let t_producer = producer();
        let t_consumer = consumer();

        t_producer.join().unwrap();
        t_consumer.join().unwrap();
    }
}
```


