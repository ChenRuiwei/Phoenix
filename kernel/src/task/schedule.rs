use core::{future::Future, pin::Pin, task::{Context, Poll}};

struct YieldFuture(bool);

impl Future for YieldFuture{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.0 {
            return Poll::Ready(());
        } 
        self.0 = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}