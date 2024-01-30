### 为什么async需要Pin
async函数内部用到引用类型(被引用的值也是async内部)的话，在底层会被编译为自引用类型：
例如：
```rust
async {
    let mut x = [0; 128];
    let read_into_buf_fut = read_into_buf(&mut x);
    read_into_buf_fut.await;
    println!("{:?}", x);
}
```
会被编译为类似：
```rust
struct ReadIntoBuf<'a> {
    buf: &'a mut [u8], // points to `x` below
}

struct AsyncFuture {
    x: [u8; 128],
    read_into_buf_fut: ReadIntoBuf<'what_lifetime?>,
}
```
如果AsyncFuture被移动，那么read_into_buf_fut.buf将会失效

### Unpin/!Unpin特征
1. 绝大多数类型自动实现了Unpin特征（不在意是否被移动），而!Unpin需要手动实现（例如通过 _marker: PhantomPinned）
2. 接上一条，其中一个例外就是：async/await 生成的 Future 默认就是!Unpin
2. Pin<P>不是特征，而是一个结构体,P是一个指针类型/或智能指针,例如Pin<&mut T> , Pin<&T> , Pin<Box<T>> ，都能确保T不会被移动。
3. Pin<P>被Pin住的值指的是指针P指向的值（确保P指向的值不会被移动），而不是指针P本身；
4. 被Pin住的值必须实现!Unpin特征才有意义；
5. 接上一条，例如：Pin<&mut u8> 跟 &mut u8 实际上并无区别，可以通过Pin<&mut u8>获取&mut u8。因为u8类型并没有实现!Unpin特征
6. 实现了!Unpin特征的类型被Pin住之后，编译器利用类型系统禁止某些操作(只对T:!Unpin进行约束)，例如获得 T和&mut T，但是&T是允许的（它无法造成T被move）
7. 上一条中被禁止的行为可以通过unsafe被允许
7. Pin<P>本身是Unpin的(几乎)

### Unpin特征对比Sync/Send
1. 都是标记特征( marker trait )，该特征未定义任何行为，非常适用于标记
2. 都可以通过!语法去除实现
3. 绝大多数情况都是自动实现, 无需我们的操心

### 总结
- 若 T: Unpin ( Rust 类型的默认实现)，那么 Pin<'a, T> 跟 &'a mut T 完全相同，也就是 Pin 将没有任何效果, 该移动还是照常移动
- 绝大多数标准库类型都实现了 Unpin ，事实上，对于 Rust 中你能遇到的绝大多数类型，该结论依然成立 ，其中一个例外就是：async/await 生成的 Future 没有实现 Unpin
- 你可以通过以下方法为自己的类型添加 !Unpin 约束：
    - 使用文中提到的 std::marker::PhantomPinned
    - 使用nightly 版本下的 feature flag
- 可以将值固定到栈上，也可以固定到堆上:
    - 将 !Unpin 值固定到栈上需要使用 unsafe
    - 将 !Unpin 值固定到堆上无需 unsafe ，可以通过 Box::pin 来简单的实现
- 当固定类型 T: !Unpin 时，你需要保证数据从被固定到被 drop 这段时期内，其内存不会变得非法或者被重用


### 自己的理解
- pin在stack上: Pin<&mut T>/Pin<& T>, 通过unsafe Pin::new_unchecked或者通过pin_utils宏或者tokio::pin!宏
- pin在heap上: Pin<Box<T>>, 通过Box::pin获得，不需要unsafe
- 为什么pin在stack上需要unsafe: 因为Pin<&'a mut T>只能确保在生命周期'a内T被pin住，而无法保证'a结束之后T是否被移动
- 接上一条，杜绝此问题的方式是用Pin<&'a mut T>遮蔽原始的T(或直接使用一些宏),例如：
```rust
// test1 is safe to move before we initialize it
let mut test1 = Test::new("test1");
// Notice how we shadow `test1` to prevent it from being accessed again
let mut test1 = unsafe { Pin::new_unchecked(&mut test1) };
```
- 接上一条，Pin<Box<T>>拥有T的所有权，不存在此问题

- 如果F:Future, 那么Pin<&mut T>或Pin<Box<F>>也满足Future，并且满足Unpin
- 接上一条，所以某些异步场景下，要求参数是Future+Unpin的，但是async block生成的future默认是!Unpin的，此时就需要把该future pin到stack或heap上

### 为什么await一个future的可变引用(&mut F)，必须要先pin
- 按照future crate中定义，如果F:Future, &mut F也满足Future的前提是 F: Unpin
- 接上一条，async block生成的future默认是!Unpin的，所以要先pin住，让它变成Unpin
- 这么做的目的是?? 个人理解(不百分百确定): 
    1. poll参数的Pin只能确保在await的过程中future不被移动，因为Pin的生命周期是poll方法内部
    2. 但是其无法保证在await之后future不被移动，因此需要先把future pin住
    3. 如果是await future本身而不是引用，则不存在此问题，await会消耗该future
    4. 如果future本身是unpin的，那它也不在意是否被移动

### 说future被pin到stack上是否表示future存储在stack上？
- 我认为不是的，最外层的future(属于task)一般存储在heap上(且被pin住)，所以它内部的future也存储在heap上
- 如果说某个future被pin在stack上，并且它真的存储在stack上，那么在它顶层task被调度的时候，它必然会被移动，因为每个线程都有自己的stack，这是违背Pin原则的
- 所以我认为的pin到stack和heap的区别是，pin到heap需要额外的申请一片堆内存来放置需要被pin住的对象，而pin到stack上则不需要，因为它已经存在于它所谓的“stack”内存上了

### Box::pin从栈上移动到堆，这个过程也会move，是否会导致自引用future失效？？？
- 目前我也不知道，只是猜测编译器做了特殊处理，让future移动到box中后自引用依然有效

