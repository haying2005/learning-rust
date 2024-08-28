## 标准库缺失的组件
1. executors
2. tasks
3. reactors （对外部提供异步订阅机制，例如async I/O, IPC, and timers ）
4. combinators
5. low-level I/O futures and traits(异步io，定时器等)

## futures create提供的组件（最终可能会被并入标准库）
1. Stream, Sink, AsyncRead, and AsyncWrite traits
2. utilities 例如combinators
3. 阉割版excutor，（因为没有提供reactor，例如异步io和定时器等）
4. 可以使用futures create的utilities和第三方excutor


## 异步生态兼容性（不是所有异步框架都互相兼容，或者兼容任何操作系统和平台）
1. 异步io，timers，IPC或task通常依赖与特定的excutor或reactor
2. 异步表达式，combinators, synchronization types, and streams通常不依赖于特定的异步生态
3. Tokio使用mio作为recator，并且定义它自己的异步io 特征，例如AsyncRead/AsyncWrite
4. async-std 和 smol依赖于async-executor crate，和future crate里的AsyncRead/AsyncWrite特征
5. 以上，Tokio与async-std 和 smol不兼容
6. 库暴露的异步api不应依赖特定的executor or reactor，除非需要spawn task或定义自己的异步io或定时器future
7. 理想情况下，只有binaries负责调度和执行task

## 单线程执行器 vs 多进程执行器
1. 任何执行器都支持单进程或多进程
2. 跨进程spawn任务，task必须实现Send
3. 一些运行时支持spawn non-Send tasks，保证任务只在当前线程执行
4. 一些运行时支持spawn阻塞任务，任务在专用线程执行（不会影响异步任务的执行），适用于执行阻塞的同步代码

### 深入异步自己的见解
#### 关于future
- future是一个状态机，保存着当前执行到哪一步的所有状态
- future是懒惰的，只有对他执行poll，它的状态才会推进
- future返回ready表示完成，.await操作也会返回相应的值，此时future被moved，将变得不再可用(不能对一个已经完成的future调用poll)
- future内部需要保存并记录此次poll传递的waker，以便适当时机触发wake
- waker通常满足Send + Sync，以便于future在不同线程之间传递
- 除了第一次的poll，后续的poll操作都必须future内部调用waker才会触发，因为只有内部才知道何时该poll(资源、数据准备就绪)，否则只会是无用的poll
- 接上一条，当poll一个future返回pending时，该future必须负责保证将来某一个时刻触发waker(包含在poll的参数Context中)，否则该future会被无限挂起
- 只有手写future时(手动实现Future特征)才需要考虑waker，更简单的实现一个future对象的方法是直接使用async函数，无需手动实现future特征，更不用考虑waker

#### 关于Task调度
- Task是excutor spawn出来的
- Task完全拥有它其中的数据(T:'static),它会在不同的线程之间传递
- Task通常内部包着一个future(spawn的参数)，但是这个future是最外层的future，它内部会包含若干个future，多层级future树
- Task约等于最外层future
- 最外层future(Task)是由异步运行时(waker+excutor)调度并poll的
- 内存future是由它的父层future负责poll的
- waker和Task(最外层future)是一一对应的
- 接上一条，因此内层future触发了waker的wake方法，导致的是整个Task(最外层future)被excutor调度，再由最外层future一层一层poll内部的future
- 接上一条，最外层future(Task)通过poll方法调用，一层一层的通过Context把waker传递到最底层future，因此最底层future能够触发Task被调度执行
- 接上一条，需要注意的是，内层future能够在不同的Task之间互相传递，在poll调用时,(如果返回pending)waker也应该要变成新的Task对应的waker(手写future时需要注意)

#### select
- future代表着异步计算，且future是懒惰的，因此取消一个异步任务只需要把对应的future drop掉
- select可以同时await多个future(branch)，并且获取第一个完成的future的值，其他future将会被drop掉
- 语法：<pattern> = <async expression> => <handler>,
```rust
use tokio::sync::oneshot;

async fn some_operation() -> String {
    // Compute value here
}

#[tokio::main]
async fn main() {
    let (mut tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        // Select on the operation and the oneshot's
        // `closed()` notification.
        tokio::select! {
            val = some_operation() => {
                let _ = tx1.send(val);
            }
            _ = tx1.closed() => {
                // `some_operation()` is canceled, the
                // task completes and `tx1` is dropped.
            }
        }
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
}
```

- select内部实现类似于一个future，它的各个branch对应的多个future被包含在内，每次对它进行poll时，它会依次(或随机顺序)poll内部各个branch的future
- 接上一条，当其中一个branch返回ready时，整个select future返回ready(且该future的结果符合对应的pattern)，其他branch将会被drop掉，不再对其进行poll
- select每个branch对应的future会并发执行，直到其中一个完成，且该future的结果符合对应的pattern,则该future的结果会绑定到对应的pattern上
- 接上一条，对应的branch的handler将会被执行，且handler内部可以访问对应pattern绑定的值
- 接上一条，即使某个branch的future完成，但返回的结果不满足对应的pattern，则其他branch依然会继续被poll，直到其中一个完成并满足对应的pattern
- 接上一条，可以使用else branch，当所有branch都完成但都不满足pattern时，else branch对应的hendler将会被执行

```rust
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        tx.send(()).unwrap();
    });

    let mut listener = TcpListener::bind("localhost:3465").await?;

    tokio::select! {
        x = async {
            loop {
                let (socket, _) = listener.accept().await?;
                // tokio::spawn(async move { process(socket) });
            }
            // Help the rust type inferencer out
            // 以下代码永不会执行
            Ok::<_, io::Error>(())
        } => {
            // accept错误
            println!("accept error {:?}", x);
        }
        _ = rx => {
            println!("terminating accept loop");
        }
    }

    Ok(())
}
```

- select的返回值等于对应branch的handler的返回值，必须保证每个handler的返回值类型一致
- branch除了可以绑定返回值外，还能使用模式匹配，当一个branch完成但是返回值不满足模式匹配，select将会继续await其他branch
- 接上一条，当
#### select中使用?传播错误
- 在handler中使用? 将错误E传播到select表达式外层函数的返回，此时外层函数的返回类型为Result<T,E>;
- 在branch中的async block中使用? 将错误传播到async block的返回值，async block的返回值类型为Result<T,E>;
```rust
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    // [setup `rx` oneshot channel]

    let listener = TcpListener::bind("localhost:3465").await?;

    tokio::select! {
        res = async {
            loop {
                // async block内部使用?，错误将被传播到async block的返回值，此时res为Err(E)
                let (socket, _) = listener.accept().await?;
                tokio::spawn(async move { process(socket) });
            }

            // Help the rust type inferencer out
            Ok::<_, io::Error>(())
        } => {
            // handler内部使用?, 错误将会传播到main函数的返回值，此时返回值为Err(E)
            res?;
        }
        _ = rx => {
            println!("terminating accept loop");
        }
    }

    Ok(())
}
```

#### select对外部数据的借用规则
- 允许多个branch对应的async block不可变借用外部数据，但是不能同时可变借用(因为一个async代表着一个future，有自己的生命周期, 并且多个branch是会并发执行)
- 接上一条，handler中没有此限制。因为只会有一个handler会被执行
- 接上一条，允许async block和handler内同时对一个外部变量进行不可变借用，因为他们不会交叉执行


#### 循环中多次select同一个future(引用)
- 多次select同一个future必须是一个future引用(该引用类型自身也必须实现Future，一般是不可变引用)，否则第一次select它就被moved进到select内部的结构体中
- 接上一条，await一个引用，该引用指向的对象必须被pinned或者满足Unpin
- 补充：如果T:Future, 则&mut T也实现了Future，但是&T没有实现(个人猜测是因为会造成重复await)
```rust
async fn action() {
    // Some asynchronous logic
}

#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = tokio::sync::mpsc::channel(128);    
    
    let operation = action();
    tokio::pin!(operation);
    // 循环中不停的select同一个异步任务operation，直到它完成或者rx接收到一个偶数
    loop {
        tokio::select! {
            _ = &mut operation => break,
            Some(v) = rx.recv() => {
                if v % 2 == 0 {
                    break;
                }
            }
        }
    }
}
```

#### select vs tokio::spawn
- 它们都可以让future并发执行（注意区分并发和并行的区别）
- tokio::spawn会产生一个新的task(异步运行时调度的基本单位)，而select的各个branch只会在一个Task内部
- tokio::spawn产生的Task可能会在多个线程中并行执行，因此他们会和产生一个新线程有相同的限制：必须拥有数据(no borrowing)
- select的各个branch不会并行执行，也不会有no borrowing的限制(多个branch引用同一个外部变量只能是不可变引用)


#### Streams特征
- 需要实现poll_next方法，能够产生多个值，当下一个值还没准备好时返回Poll::Pending, 当准备好时返回Poll::Ready(Some(T)),当不再产生新的值时返回Poll::Ready(None)
- 代表能够异步产生多个值的对象，是std::iter::Iterator的异步版本，能够在async函数中被迭代
- 类似std::iter::Iterator，它们能够通过迭代适配器(iterator adaptor)产生新的Stream或者被消费适配器消费(consuming adaptors)
- 返回Poll::Ready(None)时代表不再产生新的值，理论上来说再调用poll_next时应该报错

#### 迭代Stream
- 通常不直接调用poll_next方法，而是使用tokio_stream::StreamExt中的next方法
- 调用next方法时需要UnPin，如果不满足则需要先Pin住
- 通常通过while let Some(val) = stream.next().await方式迭代，所以只能在async中

#### 实现一个stream
1. 可以通过手动实现特征的poll_next方法，和future类似，也需要保存Waker并负责在值准备好时触发wake；当然也可以把waker直接传给内部的steam或future，由他们负责触发
2. 也可以通过generator语法: async + yield, rust语言目前不支持，可以使用过渡方案：async-stream的stream!宏
