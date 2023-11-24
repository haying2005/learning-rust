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




