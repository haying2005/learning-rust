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