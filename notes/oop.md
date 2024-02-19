### rust面向对象特性
1. 封装：rust的struct和enum类型能够拥有数据，并且能够通过impl代码块实现method
2. 通过pub关键字定义可在外部访问的数据或方法，默认为private
3. 继承：rust没有继承，但是可以通过其他手段实现类似的目的
    - 如果是为了代码复用，可以使用default trait method implementations，并且实现了该trait的类型可以override默认实现
    - 如果是为了实现多态，rust可以使用泛型类型+trait bound。这被称作：有界参数多态（bounded parametric polymorphism）
    - 运行时多态：实现一个集合中存储不同type的数据，可以用enum类型（当type set是已知且固定的）或者trait object（当type set是可扩展的）

### trait object
- 相较于struct和enum，特征对象更接近传统oop语言中的对象，因为其数据和行为是绑定在一起的；
- 特征对象既指向实现特征的类型的实例，又指向用于在运行时查找该类型的特征方法的table
- 区别于传统oop语言中的对象，我们无法往特征对象中添加数据；因为它的具体意义是对通用行为(method)进行抽象
- duck type: 只关心值的响应消息（行为），而不关心其具体类型
- trait object不需要在运行时判断一个值是否实现了特征方法；所有判断都在编译时
- trait object属于动态派发：运行时确定调用的方法。牺牲了性能获得了灵活性；
- 泛型类型+trait bound属于静态派发：编译时确定类型以及调用的方法
