use core::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};

use super::{current_time_duration, Timer, TIMER_QUEUE};

pub enum TimeoutTaskOutput<T> {
    Timeout,
    Ok(T),
}

pub struct TimeoutTaskFuture<F: Future + Send + 'static> {
    expired_time: Duration,
    task_future: F,
    has_added_to_timer: bool,
}

impl<F: Future + Send + 'static> TimeoutTaskFuture<F> {
    pub fn new(duration: Duration, task_future: F) -> Self {
        Self {
            expired_time: current_time_duration() + duration,
            task_future,
            has_added_to_timer: false,
        }
    }
}

impl<F: Future + Send + 'static> Future for TimeoutTaskFuture<F> {
    type Output = TimeoutTaskOutput<F::Output>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::trace!("[TimeoutTaskFuture::poll] enter");
        let this = unsafe { self.get_unchecked_mut() };
        let ret = unsafe { Pin::new_unchecked(&mut this.task_future).poll(cx) };
        if ret.is_ready() {
            return Poll::Ready(TimeoutTaskOutput::Ok(ready!(ret)));
        }
        if current_time_duration() >= this.expired_time {
            return Poll::Ready(TimeoutTaskOutput::Timeout);
        }
        if !this.has_added_to_timer {
            let timer = Timer {
                expired_time: this.expired_time,
                waker: Some(cx.waker().clone()),
            };
            TIMER_QUEUE.add_timer(timer);
            this.has_added_to_timer = true;
            log::trace!("[TimeoutTaskFuture::poll] add timer");
        }

        log::trace!("[TimeoutTaskFuture::poll] still not ready");

        // If single core
        #[cfg(not(feature = "kernel_interrupt"))]
        cx.waker().wake_by_ref();

        Poll::Pending
    }
}

struct IdleFuture;

impl Future for IdleFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

#[allow(unused)]
pub async fn ksleep(duration: Duration) {
    TimeoutTaskFuture::new(duration, IdleFuture {}).await;
}
