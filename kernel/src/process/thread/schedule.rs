use alloc::{boxed::Box, sync::Arc};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use log::trace;

use super::{thread_loop::user_loop, Thread};
use crate::processor::{
    self,
    ctx::{LocalContext, UserTaskContext},
};

struct YieldFuture {
    pub has_yielded: bool,
}

impl YieldFuture {
    const fn new() -> Self {
        Self { has_yielded: false }
    }
}

impl Future for YieldFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.has_yielded {
            return Poll::Ready(());
        }
        self.has_yielded = true;
        // Wake up this future, which means putting this thread into the tail of the
        // task queue
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/// The outermost future for user task, i.e. the future that wraps one thread's
/// task future (doing some env context changes e.g. pagetable switching)
pub struct UserTaskFuture<F: Future + Send + 'static> {
    task_ctx: Box<LocalContext>,
    task_future: F,
}

impl<F: Future + Send + 'static> UserTaskFuture<F> {
    #[inline]
    pub fn new(thread: Arc<Thread>, future: F) -> Self {
        let task_ctx = UserTaskContext {
            thread: thread.clone(),
            page_table: thread.process.inner.lock().memory_space.page_table.clone(),
        };
        let local_ctx = Box::new(LocalContext::new(Some(task_ctx)));
        Self {
            task_ctx: local_ctx,
            task_future: future,
        }
    }
}

impl<F: Future + Send + 'static> Future for UserTaskFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // There are 2 cases that are safe:
        // 1. the outermost future itself is unpin
        // 2. the outermost future isn't unpin but we make sure that it won't be moved
        // SAFETY: although getting the mut ref of a pin type is unsafe,
        // we only need to change the task_ctx, which is ok
        let this = unsafe { self.get_unchecked_mut() };
        let hart = processor::local_hart();
        hart.enter_user_task_switch(&mut this.task_ctx);

        // run the `threadloop`
        // SAFETY:
        // the task future(i.e. threadloop) won't be moved.
        // One way to avoid unsafe is to wrap the task_future in
        // a Mutex<Pin<Box<>>>>, which requires locking for every polling
        let ret = unsafe { Pin::new_unchecked(&mut this.task_future).poll(cx) };
        hart.leave_user_task_switch(&mut this.task_ctx);

        ret
    }
}

pub struct KernelTaskFuture<F: Future<Output = ()> + Send + 'static> {
    /// Used to construct the new kernel task's context and hold idle context
    /// temporarily (the context of the hart running in `rust_main`
    /// function as an executor) when `kernel_task_switch`
    task_ctx: Box<LocalContext>,
    task: F,
}

impl<F: Future<Output = ()> + Send + 'static> KernelTaskFuture<F> {
    pub fn new(task: F) -> Self {
        Self {
            task_ctx: Box::new(LocalContext::new(None)),
            task,
        }
    }
}

impl<F: Future<Output = ()> + Send + 'static> Future for KernelTaskFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        trace!("switch to kernel task");
        let this = unsafe { self.get_unchecked_mut() };
        let hart = processor::local_hart();
        hart.kernel_task_switch(&mut this.task_ctx);
        let ret = unsafe { Pin::new_unchecked(&mut this.task).poll(cx) };
        hart.kernel_task_switch(&mut this.task_ctx);
        ret
    }
}

/// Yield the current thread (and the scheduler will switch to next thread)
pub async fn yield_now() {
    YieldFuture::new().await;
}

/// Spawn a new async user task
pub fn spawn_user_thread(thread: Arc<Thread>) {
    let future = UserTaskFuture::new(thread.clone(), user_loop(thread));
    let (runnable, task) = executor::spawn(future);
    runnable.schedule();
    task.detach();
}

/// Spawn a new async kernel task (used for doing some kernel init work or timed
/// tasks)
pub fn spawn_kernel_thread<F: Future<Output = ()> + Send + 'static>(kernel_thread: F) {
    let future = KernelTaskFuture::new(kernel_thread);
    let (runnable, task) = executor::spawn(future);
    runnable.schedule();
    task.detach();
}
