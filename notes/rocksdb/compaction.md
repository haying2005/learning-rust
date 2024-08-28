### leveled compaction
- rocksdb默认的压缩方式
- 压缩的最小单位为sst文件
- 除L0外，每一级都是一个单独的排序集合sorted run, 由多个sst文件按顺序组成，每个sst文件内的key不重叠
- level越大，更新时间越老
- 查找时在每一级使用全二分查找，包括选出候选sst文件以及sst文件内的查找过程
- L0内为多个sst文件，每个sst都对应着一个memtable
- L1级开始，每一级都有目标大小限制，通常是成倍增加
- 压缩过程:
    1. 当L0的sst文件达到上限(level0_file_num_compaction_trigger)，触发压缩，通常会将L0级的所有sst文件全部压缩到L1(因为有重复key)
    2. 如果L1级大小超过目标大小后，则会选择至少一个sst文件开始往L2级合并，合并的过程是将sst文件内的所有数据合并到L2内对应的sst文件中
    3. 继续往下检查，如果达到合并要求，则按照第二步的方式重复进行
- 多线程压缩: 压缩过程中，如果多个sst文件需要往下级合并，并且在下层没有重叠的sst文件，则可以多线程同步进行
- 默认情况下L0到L1的压缩是单线程，但是可以通过max_subcompactions启用subcompaction，提高性能


### Universal Compaction
- Universal Compaction属于tired Compaction的变种，当leveled compaction无法满足高频的写操作时，可以尝试
- 压缩的最小单位为相邻的sorted runs
- 最多允许存在N个sorted run,这些sorted run之间key会重叠，但时间不可能重叠
- 合并只可能发生在时间相邻的两个或多个sorted run，合并输出一个新的sorted run，合并后依然满足所有sorted run时间不重叠
- 合并时sorted run的选择：
    1. 从最小的开始
    2. 如果合并后的大小如果大于相邻的sorted run，那把它也合并进来（尽可能保证时间上新的sorted run在体积上小于老的）
- 两种压缩范围
    1. major compaction：合并所有sorted runs，输出一个sorted runs，会造成暂时性的磁盘空间占用加倍(注意预留足够的磁盘空间)
    2. minor compaction：合并部分sorted runs, 输出一个sorted runs
- 4种触发条件，按照顺序先后进行判断(所有触发条件必须满足前提：sorted runs数量 >= options.level0_file_num_compaction_trigger)
    1. 根据数据年龄触发
        - 当发现有文件年龄大于options.periodic_compaction_seconds时，从老到新选择sorted runs进行合并，直到遇到一个正在被其他合并的sorted run
        - 合并后的sorted runs放到最底层level，除非最底层被用于ingestion behind（此时放到倒数第二级）
    2. 根据空间放大触发
        - 当size amplification ratio大于options.compaction_options_universal.max_size_amplification_percent / 100时触发
        - 所有文件全部合并
    3. todo: Compaction Triggered by number of sorted runs while respecting size_ratio
    4. todo: Compaction Triggered by number of sorted runs without respecting size_ratio
