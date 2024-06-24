use alloc::{collections::BTreeMap, vec::Vec};
use core::{
    fmt::Error,
    future::{self, Future},
    mem::size_of,
    pin::Pin,
    task::{Context, Poll},
};

use async_utils::{dyn_future, Async};
use memory::VirtAddr;
use systype::{SysError, SysResult, SyscallResult};
use time::timespec::TimeSpec;
use timer::timelimited_task::{TimeLimitedTaskFuture, TimeLimitedTaskOutput};
use vfs_core::PollEvents;

use super::Syscall;
use crate::mm::{UserMut, UserRdWrPtr, UserReadPtr, UserSlice, UserWritePtr};

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct PollFd {
    fd: i32,      // file descriptor
    events: i16,  // requested events
    revents: i16, // returned events
}

const FD_SETSIZE: usize = 1024;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FdSet {
    fds_bits: [u64; FD_SETSIZE / size_of::<u64>()],
}

impl FdSet {
    pub fn zero() -> Self {
        Self {
            fds_bits: [0; 1024 / size_of::<u64>()],
        }
    }

    pub fn clear(&mut self) {
        for i in 0..self.fds_bits.len() {
            self.fds_bits[i] = 0;
        }
    }

    pub fn set(&mut self, fd: usize) {
        let idx = fd / 64;
        let bit = fd % 64;
        let mask = 1 << bit;
        self.fds_bits[idx] |= mask;
    }

    pub fn is_set(&self, fd: usize) -> bool {
        let idx = fd / 64;
        let bit = fd % 64;
        let mask = 1 << bit;
        self.fds_bits[idx] & mask != 0
    }
}

pub struct PollFuture<'a> {
    futures: Vec<Async<'a, SysResult<PollEvents>>>,
    ready_cnt: usize,
}

impl Future for PollFuture<'_> {
    type Output = Vec<(usize, SysResult<PollEvents>)>;

    /// Return vec of futures that are ready. Return `Poll::Pending` if
    /// no futures are ready.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut ret_vec = Vec::new();
        for (i, future) in this.futures.iter_mut().enumerate() {
            let result = unsafe { Pin::new_unchecked(future).poll(cx) };
            if let Poll::Ready(result) = result {
                this.ready_cnt += 1;
                ret_vec.push((i, result))
            }
        }
        if this.ready_cnt > 0 {
            Poll::Ready(ret_vec)
        } else {
            Poll::Pending
        }
    }
}

impl Syscall<'_> {
    pub async fn sys_ppoll(
        &self,
        fds: UserRdWrPtr<PollFd>,
        nfds: usize,
        timeout_ts: UserReadPtr<TimeSpec>,
        _sigmask: usize,
    ) -> SyscallResult {
        let task = self.task;
        let fds_va: VirtAddr = fds.as_usize().into();
        let mut poll_fds = fds.read_array(&task, nfds)?;
        let timeout = if timeout_ts.is_null() {
            None
        } else {
            Some(timeout_ts.read(&task)?.into())
        };

        let mut futures = Vec::<Async<SysResult<PollEvents>>>::with_capacity(nfds);
        for poll_fd in poll_fds.iter() {
            let fd = poll_fd.fd as usize;
            let events = PollEvents::from_bits(poll_fd.events).unwrap();
            let file = task.with_fd_table(|table| table.get_file(fd))?;
            let future = dyn_future(async move { file.poll(events).await });
            futures.push(future);
        }

        let poll_future = PollFuture {
            futures,
            ready_cnt: 0,
        };

        let mut poll_fds_slice = unsafe { UserSlice::<PollFd>::new_unchecked(fds_va, nfds) };

        let ret_vec = if let Some(timeout) = timeout {
            match TimeLimitedTaskFuture::new(timeout, poll_future).await {
                TimeLimitedTaskOutput::Ok(ret_vec) => ret_vec,
                TimeLimitedTaskOutput::TimeOut => {
                    log::debug!("[sys_ppoll]: timeout");
                    return Ok(0);
                }
            }
        } else {
            poll_future.await
        };

        let ret = ret_vec.len();
        for (i, result) in ret_vec {
            if let Ok(result) = result {
                poll_fds[i].revents |= result.bits() as i16;
            } else {
                poll_fds[i].revents |= PollEvents::POLLERR.bits() as i16;
            }
        }
        poll_fds_slice.copy_from_slice(&poll_fds);
        Ok(ret)
    }

    /// select() allows a program to monitor multiple file descriptors, waiting
    /// until one or more of the file descriptors become "ready" for some class
    /// of I/O operation (e.g., input possible). A file descriptor is considered
    /// ready if it is possible to perform a corresponding I/O operation (e.g.,
    /// read(2), or a sufficiently small write(2)) without blocking.
    // TODO: execptfds is not used
    pub async fn sys_pselect6(
        &self,
        nfds: i32,
        readfds: UserRdWrPtr<FdSet>,
        writefds: UserRdWrPtr<FdSet>,
        exceptfds: UserRdWrPtr<FdSet>,
        timeout: UserReadPtr<TimeSpec>,
        sigmask: usize,
    ) -> SyscallResult {
        let task = self.task;
        if nfds < 0 {
            return Err(SysError::EINVAL);
        }

        log::info!("[sys_pselect6] nfds:{nfds}, readfds:{readfds}, writefds:{writefds}, exceptfds:{exceptfds}, timeout:{timeout}, sigmask:{sigmask}");

        let mut zero_read_fdset = FdSet::zero();
        let mut zero_write_fdset = FdSet::zero();
        let mut readfds = if readfds.is_null() {
            UserMut::new(&mut zero_read_fdset)
        } else {
            readfds.into_mut(task)?
        };
        let mut writefds = if writefds.is_null() {
            UserMut::new(&mut zero_write_fdset)
        } else {
            writefds.into_mut(task)?
        };
        let timeout = if timeout.is_null() {
            None
        } else {
            Some(timeout.read(task)?.into())
        };

        // `future` idx in `futures` -> fd
        let mut mapping = BTreeMap::<usize, usize>::new();
        let mut futures = Vec::<Async<SysResult<PollEvents>>>::with_capacity(nfds as usize);
        for fd in 0..nfds as usize {
            let mut events = PollEvents::empty();
            if readfds.is_set(fd) {
                events.insert(PollEvents::POLLIN)
            }
            if writefds.is_set(fd) {
                events.insert(PollEvents::POLLOUT)
            }
            if !events.is_empty() {
                let file = task.with_fd_table(|f| f.get_file(fd))?;
                log::debug!("file path:{}", file.dentry().path());
                let future = dyn_future(async move { file.poll(events).await });
                futures.push(dyn_future(future));
                mapping.insert(futures.len() - 1, fd);
            }
        }

        readfds.clear();
        writefds.clear();

        let poll_future = PollFuture {
            futures,
            ready_cnt: 0,
        };
        let ret_vec = if let Some(timeout) = timeout {
            match TimeLimitedTaskFuture::new(timeout, poll_future).await {
                TimeLimitedTaskOutput::Ok(ret_vec) => ret_vec,
                TimeLimitedTaskOutput::TimeOut => {
                    log::debug!("[sys_ppoll]: timeout");
                    return Ok(0);
                }
            }
        } else {
            poll_future.await
        };

        let mut ret = 0;
        for (i, result) in ret_vec {
            if let Ok(events) = result {
                let fd = mapping.remove(&i).unwrap();
                if events.contains(PollEvents::POLLIN) {
                    readfds.set(fd);
                    ret += 1;
                }
                if events.contains(PollEvents::POLLOUT) {
                    writefds.set(fd);
                    ret += 1;
                }
            }
        }
        Ok(ret)
    }
}
