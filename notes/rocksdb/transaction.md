## 事务的作用
 - 实现多线程并发修改数据的同时能够处理写冲突
 - 事务 vs WriteBatch
    1. 两者都能保证原子性：要么全部执行，要么全部不执行, 所有写操作提交时一次性批量写入
    2. 两者都是执行(提交)之前，写操作无法被其他线程读取
    3. WriteBatch无法实现read-your-own-writes, transaction通过WriteBatchWithIndex实现read-your-own-writes
    4. 最终提交的时候，transaction也会通过WriteBatchWithIndex生成一个WriteBatch来提交，所以transaction内部也是通过WriteBatch来实现原子性提交
## 两种事务数据库实例(处理写冲突方式不同)
 - TransactionDB(悲观锁)
    1. 事务内每一个写操作都需要先对该key加锁，如果加锁失败则整个事务失败
    2. 事务提交时，由于所有key都加锁成功，所以提交一般不会失败
    2. 适用于写冲突比较频繁的场景
 - OptimisticTransactionDB(乐观锁)
    1. 冲突检测发生在最后提交的时候
    2. 冲突检测仅对比内存中memtables中的记录，如果记录在内存中不存在，则判定为事务提交失败(不会从sstfile中读取，可能是为了写性能？)
    3. 比较逻辑是 比较当前db(仅内存中)中该key的最新seq是否小于该事务中的写seq
    4. 省去了锁的资源消耗，适合写冲突不频繁的场景，或者大量非事务+少量事务的场景，但对于写冲突频繁的场景则极易容易出现事务提交失败
## Read-Write场景
 - 在事务内使用GetForUpdate获取key的值，类似于mysql的select for update，这是目前唯一可以实现Read-Write的方案
 - TransactionDB中使用GetForUpdate会给key加锁
 - OptimisticTransactionDB中使用GetForUpdate,在事务提交时会对该key进行冲突检测

## 关于sequence number
 - seq number是递增的，类似于mysql的事务id，每一个事务会被分配一个seq number(非事务写也是一个只有单个写操作的事务)
 - 事务内所有写操作的seq number是相同的，在事务开始时就分配好了
 - writebatch中所有的写操作的seq number不相同，他们是递增的
 - 每一个record(memtable以及sstfile中)都会有一个seq number，代表着该record写入时的seq number
 - seq number可用于mvcc，snapshot，iterator数据一致性(只会返回<=某一个seqnumber的record>)，事务中的写冲突检测等
 - getSnapshot返回一个快照，与当前数据库最大seqnumber关联

## 在事务内SetSnapshot
 - 与普通事务的区别：（冲突检测升级）
    1. 普通事务只能保证当前事务先修改的key其他事务不能修改，通过SetSnapshot可以实现在事务开始的那一刻，其他事务不能修改任何key
    4. 调用txn->SetSnapshot会间接调用db->GetSnapshot返回当前的seq，并与当前事务关联，可用于写冲突检测
    2. TransactionDB事务中，其他事务修改会直接失败
    3. OptimisticTransactionDB事务中，其他事务修改可以成功，但当前事务提交会失败
    
## 在事务内实现可重复读Repeatable Read
 - 类似于普通读，在ReadOptions中设置Snapshot即可
 - 如果当前事务已经调用SetSnapshot，则可以通过txn->GetSnapshot()返回该事务关联的snapshot
 - ReadOptions中的snapshot参数仅仅用于可重复读，和事务的写冲突检测没有任何关系

