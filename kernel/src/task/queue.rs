use alloc::sync::Arc;
use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicU32, AtomicU64, Ordering::*},
};
const LOCAL_QUEUE_CAPACITY: usize = 64;
const MASK: usize = LOCAL_QUEUE_CAPACITY - 1;

pub struct Local<T> {
    inner: Arc<Inner<T>>,
}

unsafe impl<T: Send> Send for Local<T> {}

pub struct Inner<T> {
    head: AtomicU64,
    tail: AtomicU32,
    buffer: [UnsafeCell<MaybeUninit<T>>; LOCAL_QUEUE_CAPACITY],
}

impl<T> Inner<T> {
    fn remaining_slots(&self) -> usize {
        let (steal, _) = unpack(self.head.load(Acquire));
        let tail = self.tail.load(Acquire);

        LOCAL_QUEUE_CAPACITY - (tail.wrapping_sub(steal) as usize)
    }

    fn len(&self) -> u32 {
        let (_, head) = unpack(self.head.load(Acquire));
        let tail = self.tail.load(Acquire);

        tail.wrapping_sub(head)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Local<T> {
    pub fn new() -> Self {
        let inner = Arc::new(Inner {
            head: Default::default(),
            tail: Default::default(),
            buffer: core::array::from_fn(|_| UnsafeCell::new(MaybeUninit::uninit())),
        });
        Self { inner }
    }

    /// Pops a task from the local queue.
    /// 基本copy自tokio源码
    /// 如果开始操作时还没有人来偷取任务，那么读取（使用 Atomic Load
    /// 方法读取，并 unpack）到的 Steal 和 Head 一定相等，那么只需要将 Steal 和
    /// Head 都加一，再利用 compare and swap (cmp-swp)
    /// 操作存入结果，成功的话则说明 task
    /// 拿取成功。这里存在两个原子操作，第一个为 load，第二个为
    /// cmp-swp，第二个操作有可能失败，失败的原因是此时恰巧其他线程来偷取任务，
    /// 修改了该原子变量。如果cmp-swp
    /// 操作失败，则头开始再尝试一次即可，直到成功。
    /// 如果开始操作时已经有人正在偷取任务，那么读取到的 Steal 和 Head
    /// 一定不相等，那么只需要将 Head 加一，Steal 保持不变，再利用 cmp-swp
    /// 操作存入结果即可。同上，cmp-swp 如果成功，说明拿取本地 task
    /// 成功，否则失败，重复上述操作直到成功。
    pub fn pop(&mut self) -> Option<T> {
        let mut head = self.inner.head.load(Acquire);
        let idx = loop {
            let (steal, real) = unpack(head);
            // safety: this is the **only** thread that updates this cell.
            let tail = self.inner.tail.load(Relaxed);
            if real == tail {
                // queue is empty
                return None;
            }
            let next_real = real.wrapping_add(1);
            // If `steal == real` there are no concurrent stealers. Both `steal`
            // and `real` are updated.
            let next = if steal == real {
                pack(next_real, next_real)
            } else {
                assert_ne!(steal, next_real);
                pack(steal, next_real)
            };
            // Attempt to claim a task.
            let res = self
                .inner
                .head
                .compare_exchange(head, next, AcqRel, Acquire);
            match res {
                // & MASK为了保证索引在达到队列最大长度时自动回绕到0
                Ok(_) => break real as usize & MASK,
                Err(actual) => head = actual,
            }
        };
        Some(unsafe { self.inner.buffer[idx].get().read().assume_init() })
    }

    pub fn push(&mut self, mut task: T, mut inject: impl FnMut(T)) {
        let tail = loop {
            let head = self.inner.head.load(Acquire);
            let (steal, real) = unpack(head);
            let tail = self.inner.tail.load(Relaxed);
            // 确定是否有足够的空间在队列的尾部插入一个新元素
            if tail.wrapping_sub(steal) < LOCAL_QUEUE_CAPACITY as u32 {
                break tail;
            }
            // 当steal不等于real时，表示有窃取操作正在进行。在这种情况下，
            // 如果本地线程尝试添加任务，
            // 它应该采取特殊措施（如通过inject函数将任务放入备用存储），
            // 以避免与窃取操作冲突
            if steal != real {
                inject(task);
                return;
            }
            match self.push_overflow(task, real, tail, &mut inject) {
                Ok(()) => return,
                Err(t) => task = t,
            }
        };
        let idx = tail as usize & MASK;
        unsafe { self.inner.buffer[idx].get().write(MaybeUninit::new(task)) };
        self.inner.tail.store(tail.wrapping_add(1), Release);
    }

    pub fn push_overflow(
        &mut self,
        task: T,
        head: u32,
        tail: u32,
        mut inject: impl FnMut(T),
    ) -> Result<(), T> {
        assert_eq!(
            tail.wrapping_sub(head) as usize,
            LOCAL_QUEUE_CAPACITY,
            "queue is not full; tail = {}; head = {}",
            tail,
            head
        );
        /// How many elements are we taking from the local queue.
        ///
        /// This is one less than the number of tasks pushed to the inject
        /// queue as we are also inserting the `task` argument.
        const NUM_TASKS_TAKEN: u32 = (LOCAL_QUEUE_CAPACITY / 2) as u32;
        let prev = pack(head, head);
        let next_head = head.wrapping_add(NUM_TASKS_TAKEN);
        let next = pack(next_head, next_head);
        // Claim a bunch of tasks
        //
        // We are claiming the tasks **before** reading them out of the buffer.
        // This is safe because only the **current** thread is able to push new
        // tasks.
        //
        // There isn't really any need for memory ordering... Relaxed would
        // work. This is because all tasks are pushed into the queue from the
        // current thread (or memory has been acquired if the local queue handle
        // moved).
        if self
            .inner
            .head
            .compare_exchange(prev, next, Release, Relaxed)
            .is_err()
        {
            // We failed to claim the tasks, losing the race. Return out of
            // this function and try the full `push` routine again. The queue
            // may not be full anymore.
            return Err(task);
        }
        inject(task);
        let batch = (head..next_head).map(|head| {
            let index = head as usize & MASK;
            // SAFETY: Successful CAS assumed ownership of these values.
            unsafe { self.inner.buffer[index].get().read().assume_init() }
        });
        batch.for_each(inject);

        Ok(())
    }
    pub fn stealer(&self) -> Steal<T> {
        Steal(self.inner.clone())
    }
}

pub struct Steal<T>(Arc<Inner<T>>);

unsafe impl<T: Send> Send for Steal<T> {}
unsafe impl<T: Send> Sync for Steal<T> {}

impl<T> Steal<T> {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Steals half the tasks from self and place them into `dst`.
    /// 注意是从自己的任务队列中窃取一半给dst，再从窃取的任务中保留一个任务返回给调用者
    pub fn steal_into_and_pop(&self, dst: &mut Local<T>) -> Option<T>{
        // Safety: the caller is the only thread that mutates `dst.tail` and
        // holds a mutable reference.
        let dst_tail = dst.inner.tail.load(Relaxed);
        let (steal, _) = unpack(dst.inner.head.load(Acquire));
        // 检查dst队列是否有足够空间接受窃取的任务。如果dst中的可用空间小于self中一半的任务数量，则不执行窃取
        if dst_tail.wrapping_sub(steal) > LOCAL_QUEUE_CAPACITY as u32 / 2 {
            return None;
        }
        // n变量包含了窃取的任务数量
        let mut n = self.steal_n(dst, dst_tail);
        if n == 0 {
            return None;
        }
        // 返回的任务是从窃取的这些任务中选取的最后一个任务。因此，需要从dst队列的尾部索引dst_tail开始，加上n - 1来定位到这个任务在队列中的位置
        n -= 1;
        let ret_pos = dst_tail.wrapping_add(n);
        let ret_idx = ret_pos as usize & MASK;
        // 返回的任务在dst队列中的位置是ret_pos
        let ret = unsafe { dst.inner.buffer[ret_idx].get().read().assume_init() };

        if n == 0 {
            // The `dst` queue is empty, but a single task was stolen
            return Some(ret);
        }
        // Make the stolen items available to consumers
        dst.inner.tail.store(dst_tail.wrapping_add(n), Release);

        Some(ret)
    }

    // Steal tasks from `self`, placing them into `dst`. Returns the number of
    // tasks that were stolen.
    pub fn steal_n(&self, dst: &mut Local<T>, dst_tail: u32) -> u32 {
        let mut prev_packed = self.0.head.load(Acquire);
        let mut next_packed;
        
        let n = loop {
            let (src_head_steal, src_head_real) = unpack(prev_packed);
            let src_tail = self.0.tail.load(Acquire);

            // If these two do not match, another thread is concurrently
            // stealing from the queue.
            if src_head_steal != src_head_real {
                return 0;
            }

            // Number of available tasks to steal
            let n = src_tail.wrapping_sub(src_head_real);
            let n = n - n / 2;

            if n == 0 {
                // No tasks available to steal
                return 0;
            }

            // Update the real head index to acquire the tasks.
            let steal_to = src_head_real.wrapping_add(n);
            assert_ne!(src_head_steal, steal_to);
            next_packed = pack(src_head_steal, steal_to);

            // Claim all those tasks. This is done by incrementing the "real"
            // head but not the steal. By doing this, no other thread is able to
            // steal from this queue until the current thread completes.
            let res = self
                .0
                .head
                .compare_exchange(prev_packed, next_packed, AcqRel, Acquire);

            match res {
                Ok(_) => break n,
                Err(actual) => prev_packed = actual,
            }
        };
        
        assert!(
            n <= LOCAL_QUEUE_CAPACITY as u32 / 2,
            "actual = {}",
            n
        );
        
        let (first, _) = unpack(next_packed);

        // Take all the tasks
        for i in 0..n {
            // Compute the positions
            let src_pos = first.wrapping_add(i);
            let dst_pos = dst_tail.wrapping_add(i);

            // Map to slots
            let src_idx = src_pos as usize & MASK;
            let dst_idx = dst_pos as usize & MASK;

            // Read the task
            //
            // safety: We acquired the task with the atomic exchange above.
            let task = unsafe { self.0.buffer[src_idx].get().read() };

            // Write the task to the new slot
            //
            // safety: `dst` queue is empty and we are the only producer to
            // this queue.
            unsafe { dst.inner.buffer[dst_idx].get().write(task) };
        }

        let mut prev_packed = next_packed;

        // Update `src_head_steal` to match `src_head_real` signalling that the
        // stealing routine is complete.
        loop {
            let head = unpack(prev_packed).1;
            next_packed = pack(head, head);

            let res = self
                .0
                .head
                .compare_exchange(prev_packed, next_packed, AcqRel, Acquire);

            match res {
                Ok(_) => return n,
                Err(actual) => {
                    let (actual_steal, actual_real) = unpack(actual);

                    assert_ne!(actual_steal, actual_real);

                    prev_packed = actual;
                }
            }
        }
    }
}



/// Split the head value into the real head and the index a stealer is working
/// on.
fn unpack(n: u64) -> (u32, u32) {
    let steal = n & u32::MAX as u64;
    let real = n >> 32;
    (steal as u32, real as u32)
}

/// Join the two head values
fn pack(steal: u32, real: u32) -> u64 {
    (steal as u64) | ((real as u64) << 32)
}
