# Debug 记录

<!--toc:start-->

- [Debug 记录](#debug-记录)
  - [COW 中的 `Arc::strong_cnt`](#cow-中的-arcstrongcnt)
  - [`do_execve` 中 `MemorySpace` 的赋值顺序](#doexecve-中-memoryspace-的赋值顺序)
  <!--toc:end-->

## COW 中的 `Arc::strong_cnt`

在 COW 机制的实现时，我们使用 `Arc::strong_cnt` 来获取每页的引用计数。
如果引用计数大于 1，就复制一份新的页；如果引用计数等于 1，就直接修改 PTE 即可。
但是，在 log 时，我们经常发现两个核心同时获取到引用计数为 3 的情况，
这是因为 `Arc::strong_cnt` 与 `Arc` 的 drop 之间的过程并不能保证原子性。
当然，这不会影响到 COW 机制的正确性，毕竟同时获取到引用计数为 2 时，也就多一次拷贝页的操作。

## `do_execve` 中 `MemorySpace` 的赋值顺序

在 `do_execve` 函数中，我们需要将原有的 `MemorySpace` 赋值成新的 `MemorySpace`，
但是**在赋值之前需要先切换页表**。

如果切换页表在赋值操作之后，会导致原来的 `MemorySpace` 被 `drop` 掉，
从而让其持有的 `PageTable` 也被 `drop` 掉，
也就导致 `PageTable` 持有的保存内部页表的 `FrameTracker` 被 `drop` 掉，
而在切换页表之前，我们的 satp 寄存器还是原来的页表。
因此在多核环境下，其他核心可能会获取原来页表的 `Frame`，并往里面写入数据覆盖掉里面的内容，
就导致在赋值操作和切换页表之间存在页表被修改的危险区域，会导致偶发的 panic。
