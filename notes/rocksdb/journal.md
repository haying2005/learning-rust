### rocksdb日志有以下两种
 1. Write Ahead Log (WAL): 记录内存状态更新
 2. MANIFEST: 记录磁盘状态更新

### WAL
#### 概述
 - 所有数据更新操作会被记录在两个地方: memtable(to be flushed to SST files later) 和 wal日志文件(on disk);
 - wal日志文件(on disk, 可用于崩溃后的数据恢复), 单个日志文件可记录多个列族的记录
 - wal可用于数据恢复，以及分布式系统中不同实例之间复制数据

#### WAL生命周期
 - 当某一个列族从memtable中flush到sstfile之后，会创建一个新的wal文件，所有列族的更新记录会记录到新的wal，之前的旧wal不再记录任何数据
 - 当wal中所有数据(包括所有列族)都被flush之后，该wal才会被删除或归档(归档之后不会立即删除, 可能有其他用途，例如replicate the data between RocksDB instances)

#### WAL配置
 - DBOptions::wal_dir：可以配置到单独的目录，例如可以将wal配置到一些性能较慢但廉价的存储介质
 - DBOptions::WAL_ttl_seconds, DBOptions::WAL_size_limit_MB：指定被归档的wal文件触发删除的时间阈值/磁盘空间占用阈值
 - DBOptions::max_total_wal_size：触发自动flush的wal大小阈值
 - BOptions::avoid_flush_during_recovery
 - DBOptions::manual_wal_flush 手动flush
 - DBOptions::wal_filter
 - DBOptions::wal_compression：wal压缩算法
 - WriteOptions::disableWAL 禁用wal，不在乎数据丢失的情况下可用

 ### MANIFEST todo: