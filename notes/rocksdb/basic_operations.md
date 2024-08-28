### opening db

### closing db

### reads

#### point lookup流程(忽略merge operations)：
 1. 先查找mutable memtable：如果有bloom filter，先检测再查找；
 2. 如果mutable memtable为找到，同样的方式查找immutable memtables；
 3. 如果上述步骤未找到，则开始从L0开始查找sstfile
  - L0: 按时间倒序查找各个sstfile(L0级的数据组织方式和L1及以上不同)
  - L1+: 每一级都有一个vector of SST file metadata objects，通过对其二分查找定位到其中一个sstfile；L1级以及后续的每一层级都有一个辅助索引，可以缩小后续层级的SST file metadata objects二分查找的范围
  - 定位到sstfile之后，先加载该sstfile的bloom filter block(如果有的话，从block cache或磁盘)，通常最底层的sstfile未配置bloomfilter
  - bloomfilter为阳性，则加载sstfile内的index block进行二分查找，定位到数据所在的data block;
  - 加载data block并对其进行二分查找，最终找到所需的key
 4. 对L1以后的每一个level重复步骤3，直到找到key或到最后一个level为止

 #### MultiGet性能优化(相比多次Get)
  1. 当options.cache_index_and_filter_blocks=true，filter block和index block会从block cache中读取，这个过程会有LRU mutex冲突，而multiGet一批只会读取一次，从而减少LRU mutex冲突；
  2. 针对point lookup中各步骤里的bloom filter block的cpu缓存miss，缓存行的访问可以管道化，从而隐藏了cpu缓存miss造成的延迟(这句话不是很懂。。。)
  3. MultiGet可以时对同一个sstfile的多个data block的磁盘IO进行并发读取，从而减少延迟

### writes
#### 原子更新
 - WriteBatch可以保证多个写操作的原子性, 避免出现部分成功的现象
 - 同时还能提高写性能
#### 同步写
 - 默认情况下是异步写：仅提交wal写操作到操作系统，不保证刷盘
 - 通过配置同步写，可以确保写操作刷盘完成后才返回，例如调用fsync、fdatasync...

#### 非同步写
  - 仅提交wal写操作到操作系统，不保证刷盘
  - 进程崩溃不会造成数据丢失，除非系统崩溃才可能会造成数据丢失

### 并发
 - 一个db同一时刻只能由1个进程打开
 - rocksdb采用操作系统提供的锁来阻止误操作
 - 同一个rocksdb实例在进程内可安全得被多个线程共享，通过内部实现的线程同步，无需额外的同步
 - Iterator 和 WriteBatch如果需要被多个线程共享，则需额外实现线程同步

### merge operations

### 迭代器iteration

### 快照snapshot
 - 提供了一个一致性只读视图，反应了数据库某一时刻的状态
 - 读操作中ReadOptions::snapshot如果不为空，则说明需要读取该快照的内容，如果为空，则会隐式创建一个最新的snapshot用于读取
 - snapshot会锁定资源(即使某一个key已经被改写或删除)，如果snapshot不再需要，则应该调用ReleaseSnapshot，释放该快照所需的资源

### slice切片
 - 避免内存拷贝，可以使用rocksdb::Slice接收读取的数据

### 列族Column Families
 - 每个rocksdb可以有多个列族，作为数据的逻辑分区
 - 每个rocksdb数据库有一个default列族，如果记录没有指定列族，则会被分配到default列族；
 - 支持跨列族的原子写入
 - 支持跨列族的一致性视图
 - 支持各列族独立配置
 - 支持高效的动态添加/删除列族
 - 多个列族可以共享wal文件(便于跨列族的原子写)，但是拥有各自独立的memtables、sst文件(便于独立配置，高效删除整个列族数据)
 - 当某个列族的memtable flush到sst文件后，会创建新的wal，此时所有列族将会往新的wal写入，但是老的wal还不能被删除，直到老的wal中所有列族都被flush

### IO
 - 默认情况下磁盘IO利用操作系统的页缓存，可以通过设置Rate Limiter限制写操作频率，来给读操作腾出空间
 - 也可以选择Direct IO来绕过页缓存
 - Rate Limiter: todo:
 - 写IO控制
  - 范围同步range sync: 通过配置options.bytes_per_sync和options.wal_bytes_per_sync控制rocksdb周期性的调用sync_file_range部分刷盘，避免一次性同步太多的页缓存造成延迟
  - Rate Limiter：通过配置options.rate_limiter限制写操作频率给读操作腾出空间
  - Write Max Buffer：当appending a file时，在写入文件系统之前，rocksdb会先写入自己的cache；options.writable_file_max_buffer_size可以修改这个缓冲区的最大值，Direct IO模式下这个配置尤为重要，默认模式下建议保持默认
  - 文件删除：通过配置delete scheduler可以限制过期文件的删除频率，在闪存设备上特别有用，能够降低由于大量删除而造成的读延迟峰值

 - 读IO控制
  - fadvise: 默认状态下options.advise_random_on_open = true(此时会调用fadvise，让操作系统减少预读)，此时适合Get(随机读取)和小范围的iterating操作为主的场景，因为此时不需要预读，否则将其设置成false(此时不会调用fadvise)以便更好的利用操作系统的预读机制；
  - Compaction输入，以下专门针对Compaction操作：
   - fadvise hint：options.access_hint_on_compaction_start，可以覆盖advise_random_on_open，从而专门针对compaction调用fadvise
   - 通过options.new_table_reader_for_compaction_inputs = true，对compaction input使用不同的文件描述符，便于设置不同的fadvise设置，缺点是占用一点额外的内存空间
   - compaction inputs预读：通过配置options.compaction_readahead_size，实现rocksdb自己内部的预读；对于Direct IO模式以及不支持预读的文件系统特别重要

### Memory Mapping
 - options.allow_mmap_reads 和 options.allow_mmap_writes分别设置读/写的mmap,减少内存拷贝

### 避免阻塞IO Avoid Blocking IO
 - Iterator以及列族的清理任务默认情况下会在当前线程上下文中删除过期文件，且会受制于deletion rate limits, 可能会造成较大的延迟，为避免延迟可将删除操作推迟到后台线程中
 - ReadOptions::background_purge_on_iterator_cleanup在创建Iterator时配置，可以是Iterator清理任务中的过期文件删除在后台线程中进行；
 - DBOptions::avoid_unnecessary_blocking_io：Iterator以及ColumnFamilyHandle清理任务中的过期文件删除配置到后台线程中进行；

### MemTable
 - memTable在内存中负责数据的读、写；
 - 写操作会先写入memtable，当memtable满了之后，会变成不可变memtable，然后创建一个新的可变memtable负责新的写操作
 - 读操作也会先从memtable中读取，未找到的情况下才会读取sst；
 - 后台线程负责定期将不可变memtable flush到sst, 之后才可以被删除

#### MemTable配置项
 - AdvancedColumnFamilyOptions::memtable_factory：默认是SkipListFactory，也可以是vector等。。。
 - ColumnFamilyOptions::write_buffer_size：单个memtable的大小，默认64M
 - DBOptions::db_write_buffer_size: 所有列族的memtable总大小(每个列族都有各自独立的memtable)
 - DBOptions::write_buffer_manager：用户提供自己的write buffer manager来控制总的memtable内存大小，来覆盖DBOptions::db_write_buffer_size配置；
 - AdvancedColumnFamilyOptions::max_write_buffer_number：内存中最大memtable数，超过则flush到sst，默认2
 - AdvancedColumnFamilyOptions::max_write_buffer_size_to_maintain: 内存中最大memtable大小，包括已flush以及为flush的之和。rocksdb会尽量不删除已经flush的memtable，除非超过这个阈值；默认0 

#### Skiplist MemTable
 - 基于跳表的memtable对读、写、随机访问、顺序扫描都有较好的性能
 - 此外还提供了一些其他类型的memtable不具备的特性：例如并发插入、insert with hint(就地更新);
 - 并发插入：DBOptions::allow_concurrent_memtable_write：允许多线程并发的插入数据，默认开启
 - 就地更新：通过bool inplace_update_support flag开启，因为与并发插入不可兼容，所以默认关闭

#### HashSkiplist MemTable
 - 哈希跳表memtable外层是一个hash table，每一个hash bucket是一个跳表；One good use case is to combine them with PlainTable SST format and store data in RAMFS.
 - 查找时用前缀定位到hash bucket，然后再从其中的跳表中查找完整的key;
 - 缺点是跨多个前缀扫描时，需要拷贝/排序，耗费性能与内存

#### memtable flush：以下3中情况会触发flush:
 - 单个memtable大小超过ColumnFamilyOptions::write_buffer_size
 - 所有memtable总大小超过db_write_buffer_size或触发BOptions::write_buffer_manager的flush信号，此时会flush其中最大的memtable；
 - 总的wal文件大小超过max_total_wal_size，此时会flush其中拥有最老数据的memtable，以便于清理记录着该memtable中数据的wal；

#### block size
 - rocksdb会把相邻的记录放到一个block中，block也是与持久化文件系统之间传输以及缓存的最小单位，并且压缩也是针对单个block单独压缩；
 - 较大的block size适合范围扫描为主的场景，同时具备更高的压缩效率；
 - 较小的block size适合随机读取为主的场景
 - 通过Options::block_size设置，默认大约为4096(未压缩大小)，不建议设置为小于1k或大于几M

#### Write buffer 控制memtable的数量与大小
 - Options::write_buffer_size 参考memtable
 - Options::max_write_buffer_number 参考memtable
 - Options::min_write_buffer_number_to_merge：默认为1，此时所有memtable会单独flush到L0的sst，此时会加重读放大,且会浪费更多的磁盘空间，因为他们中间会有重复的key；如果设置为2，则代表至少会将两个memtable合并成一个然后再flush到L0；
 
#### 缓存Cache 包括block cache与文件系统的page cache
 - options.block_cache用于缓存未压缩的block
 - 文件系统的页缓存用来缓存原始的经过压缩的sst文件内容
 - 当我们进行大量数据的读取时(例如Iterator)，有时候我们不希望读取的内容替换现有的block cache, 可以通过在读操作设置options.fill_cache = false来实现

#### block cache
 - block cache用于缓存sst中的block(未压缩)，仅用于读操作
 - 通过Cache对象来配置需要的大小，并且同一进程下多个db实例可共享一个Cache对象，用于控制多个db的总block cache大小；
 - 可选地，用户可以配置一个额外的block cache用于缓存压缩过的block，在direct-io模式下可替代操作系统页缓存；
 - rocksdb有两种缓存实现：LRUCache和ClockCache，他们都是分片的以减少锁竞争；
 - 默认情况下使用基于LRU的缓存，容量为32M，仅适用于不严重依赖rocksdb的读取性能并希望保持一个相对较低的内存占用的场景(意思就是想提高读性能，可适当加大缓存大小)

##### LRU Cache
 - 每个分片维护着各自的LRU list以及各自用于查找的hash表；
 - 每个分片有自己的互斥锁用于线程同步；
 - 对每个分片的查找和插入都需要加锁，因为读操作也会修改LRU的元数据
 - 可通过NewLRUCache()创建一个LRU cache

##### 缓存Index, Filter, 和 Compression Dictionary block
 - 默认情况下，index, filter, and compression dictionary blocks都是缓存在block cache之外；
 - 除了设置max_open_files之外，用户无法控制他们的缓存大小；
 - 通过设置cache_index_and_filter_blocks=true, 可以把他们缓存到block cache中，以便更好的控制总的内存占用


### bloom filter
 - 本质上是一个bit数组
 - 每个key对应n个bit位(n可配置)，通过多个hash确认每个bit的位置，只要有1个bit为0，则能够确定该key绝对不存在于一个sst
 - 一旦配置了，每一层级的sst文件内都会构建一个bloom filter，除了最底层的sst可以配置为不构建
 - 当打开一个sst时，相应的过滤器也会被使用其原始配置加载到内存中，当关闭sst时从内存中删除；
#### 完整过滤器
 - 相对于旧的格式(对sst中的key分片分别创建过滤器，每个分片大约2kb)，新的格式为一个sst文件创建一个过滤器；代价是需要占用更多的内存
 - 对每个key的所有探测bit完全放置到一个cpu cache line（缓存对齐），减少cpu cache miss;
#### 前缀过滤器 vs 完整key过滤器
 - 当配置了过滤器，完整key过滤器值默认构建，也可手动禁用
 - 当Options.prefix_extractor设置，会自动构建前缀过滤器，相比Key过滤器占用更少的空间，但是会提高误报率；
 - 前缀过滤器能用于(可选)seek和seekForPrev，但是key过滤器只能用于点查询

 ### prefix seek
 - prefix seek本质上是prefix extractor + prefix bloom filter
 - 一旦配置了prefix extractor + prefix bloom filter，则Iterator默认启用prefix bloom filter
 - 接上一条，默认的readOptions, iterator会自动启用prefix bloom filter，即Manual prefix iterating，此时用户必须确保迭代器不要超出前缀范围，如果超过前缀范围，会出现未定义结果：包括key缺失、返回已删除key、key顺序错乱、慢查询。。。
 - 接上一条，如果希望迭代器超出前缀范围，则需手动指定read_options.total_order_seek = true， 禁用prefix bloom filter
 - 接上一条，如果希望迭代器超出前缀范围，同时又想使用prefix bloom filter，则可采用自适应前缀模式，通过设置read_options.auto_prefix_mode = true;
  1. 返回的结果与read_options.total_order_seek = true相同，但仍可使用prefix bloom filter
  2. 目前仅支持seek(), SeekForPrev()将不会启用prefix bloom filter;
  3. 需要额外的cpu资源判断每一个sst是否可以启用prefix bloom filter;
 - 通用prefix seek api
  - 默认的Manual prefix iterating模式下，通过设置ReadOptions.prefix_same_as_start=true可以保证前缀范围内没有更多的key时，返回Valid()=false；
  - 4.11版本之后，prev()支持前缀模式，但不保证超出前缀范围之后的正确性
 - 限制
  - SeekToLast()不支持前缀迭代， SeekToFirst()只有部分配置下支持，如果使用它们的话应该使用total order mode
  - 反序迭代时不应使用prefix seek

### 校验和checksum
 - 校验和与文件系统中所有数据关联(sst文件)，校验数据的完整性
 - ReadOptions::verify_checksums 强制对读取的所有数据进行校验，默认开启
 - Options::paranoid_checks偏执检查：使rocksdb在数校验检测到错误时，抛出错误，可能是打开数据库是抛错，也可能是后续操作；
 - DB::VerifyChecksum()：通过调用该方法手动校验所有sst的所有数据，目前只支持BlockBasedTable格式的sst
 - 当数据库检测到错误时，可以使用rocksdb::RepairDB进行修复
 

### Direct IO: todo