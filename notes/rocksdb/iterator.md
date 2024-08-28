## Iterator
### 简介
 - iterator可用于range scan（正序或反序）
 - 每次调用it-next()返回一个记录
 - 快照读：返回的所有记录之间具有相互一致性(consistent-point-in-time view),即返回的所有记录来自同一个时间点的快照
### 一致性视图Consistent View
 - 如果ReadOptions.snapshot参数给出，则所返回的记录来自该snapshot，若未给出，则会创建一个当前最新的snapshot

### 资源占用
 - Iterators本身内存占用很少，但是他们会阻止数据资源释放，包括：
    1. memtables and SST files会在flush或compaction之后依然不会被删除
    2. data block cache
 - 所以iterator不应该长时间存活，如果在长时间(例如1秒)内不在使用，应该销毁或刷新迭代器(因为迭代器数据可能会过期)
 - 5.7版本之后，调用Iterator::Refresh()可以刷新Iterator到最新状态，以释放一些过期的资源
 - 5.7版本之前，应该销毁并重建

### 预读Read-ahead
 - 自动预读(默认启用)
  1. 开启的前提是ReadOptions.readahead_size = 0(默认如此)
  2. 对同一个sst文件两次以上io后自动启用，并且从8kb开始逐次增加，最大到BlockBasedTableOptions.max_auto_readahead_size(默认256kb)；
  3. linux系统中，bufferedIO模式使用readahead系统调用，而DIrectIO模式下使用AlignedBuffer

 - 手动指定固定预读
  1. 通过设置ReadOptions.readahead_size不为0启用
  2. 每次读取sst文件都会预读固定大小的数据
  3. 会增加Iterator的固定开销
  4. 何时使用：当读取大范围的数据，且没有其他方式实现时，例如远程存储; 能自动的尽量不要固定
  5. 必须预估预读的内存占用
