use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::{
    fmt::Error,
    future::{self, Future},
    mem::{self, size_of},
    ops::Deref,
    pin::Pin,
    task::{Context, Poll},
};

use async_utils::{dyn_future, yield_now, Async, Select2Futures, SelectOutput};
use memory::VirtAddr;
use signal::SigSet;
use systype::{SysError, SysResult, SyscallResult};
use time::timespec::TimeSpec;
use timer::timelimited_task::{TimeLimitedTaskFuture, TimeLimitedTaskOutput};
use vfs::fd_table::Fd;
use vfs_core::{File, PollEvents};

use super::Syscall;
use crate::{
    mm::{UserMut, UserRdWrPtr, UserReadPtr, UserSlice, UserWritePtr},
    task::signal::IntrBySignalFuture,
    trap::context,
};

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct PollFd {
    /// file descriptor
    fd: i32,
    /// requested events    
    events: PollEvents,
    /// returned events
    revents: PollEvents,
}

const FD_SETSIZE: usize = 1024;
const FD_SETLEN: usize = FD_SETSIZE / (8 * size_of::<u64>());

#[derive(Debug, Copy, Clone)]
#[repr(C)]
/// A fixed length array, where each element is a 64 bit unsigned integer. It is
/// used to store a bitmap of a set of file descriptors
pub struct FdSet {
    fds_bits: [u64; FD_SETLEN],
}

impl FdSet {
    pub fn zero() -> Self {
        Self {
            fds_bits: [0; FD_SETLEN],
        }
    }

    pub fn clear(&mut self) {
        for i in 0..self.fds_bits.len() {
            self.fds_bits[i] = 0;
        }
    }

    /// Add the given file descriptor to the collection. Calculate the index and
    /// corresponding bit of the file descriptor in the array, and set the bit
    /// to 1
    pub fn set(&mut self, fd: usize) {
        let idx = fd / 64;
        let bit = fd % 64;
        let mask = 1 << bit;
        self.fds_bits[idx] |= mask;
    }

    /// Check if the given file descriptor is in the collection. Calculate the
    /// index and corresponding bit of the file descriptor in the array, and
    /// check if the bit is 1
    pub fn is_set(&self, fd: usize) -> bool {
        let idx = fd / 64;
        let bit = fd % 64;
        let mask = 1 << bit;
        self.fds_bits[idx] & mask != 0
    }
}

pub struct PPollFuture {
    polls: Vec<(PollEvents, Arc<dyn File>)>,
}

impl Future for PPollFuture {
    type Output = Vec<(usize, PollEvents)>;

    /// Return vec of futures that are ready. Return `Poll::Pending` if
    /// no futures are ready.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut ret_vec = Vec::new();
        for (i, (events, file)) in this.polls.iter().enumerate() {
            let result = unsafe { Pin::new_unchecked(&mut file.poll(*events)).poll(cx) };
            match result {
                Poll::Pending => unreachable!(),
                Poll::Ready(result) => {
                    if !result.is_empty() {
                        ret_vec.push((i, result))
                    }
                }
            }
        }
        if ret_vec.len() > 0 {
            Poll::Ready(ret_vec)
        } else {
            Poll::Pending
        }
    }
}

pub struct PSelectFuture {
    polls: Vec<(Fd, PollEvents, Arc<dyn File>)>,
}

impl Future for PSelectFuture {
    type Output = Vec<(Fd, PollEvents)>;

    /// Return vec of futures that are ready. Return `Poll::Pending` if
    /// no futures are ready.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut ret_vec = Vec::with_capacity(this.polls.len());
        for (fd, events, file) in this.polls.iter() {
            let result = unsafe { Pin::new_unchecked(&mut file.poll(*events)).poll(cx) };
            match result {
                Poll::Pending => unreachable!(),
                Poll::Ready(result) => {
                    if !result.is_empty() {
                        ret_vec.push((*fd, result))
                    }
                }
            }
        }
        if ret_vec.len() > 0 {
            Poll::Ready(ret_vec)
        } else {
            Poll::Pending
        }
    }
}

impl Syscall<'_> {
    /// `ppoll` is used to monitor a set of file descriptors to see if they have
    /// readable, writable, or abnormal events
    pub async fn sys_ppoll(
        &self,
        fds: UserRdWrPtr<PollFd>,
        nfds: usize,
        timeout: UserReadPtr<TimeSpec>,
        sigmask: UserReadPtr<SigSet>,
    ) -> SyscallResult {
        let task = self.task;
        let fds_va: VirtAddr = fds.as_usize().into();
        let mut poll_fds = fds.read_array(&task, nfds)?;
        let timeout = if timeout.is_null() {
            None
        } else {
            Some(timeout.read(&task)?.into())
        };

        let new_mask = if sigmask.is_null() {
            None
        } else {
            Some(sigmask.read(task)?)
        };
        log::info!(
            "[sys_ppoll] fds:{poll_fds:?}, nfds:{nfds}, timeout:{timeout:?}, sigmask:{new_mask:?}"
        );
        let mut polls = Vec::<(PollEvents, Arc<dyn File>)>::with_capacity(nfds as usize);
        for poll_fd in poll_fds.iter() {
            let fd = poll_fd.fd as usize;
            let events = poll_fd.events;
            let file = task.with_fd_table(|table| table.get_file(fd))?;
            log::debug!("fd:{fd}, file path:{}", file.dentry().path());
            polls.push((events, file));
        }

        let old_mask = if let Some(mask) = new_mask {
            Some(mem::replace(task.sig_mask(), mask))
        } else {
            None
        };

        let poll_future = PPollFuture { polls };

        let mut poll_fds_slice = unsafe { UserSlice::<PollFd>::new_unchecked(fds_va, nfds) };
        task.set_interruptable();
        task.set_wake_up_signal(!*task.sig_mask_ref());
        let ret_vec = if let Some(timeout) = timeout {
            match TimeLimitedTaskFuture::new(timeout, poll_future).await {
                TimeLimitedTaskOutput::Ok(ret_vec) => ret_vec,
                TimeLimitedTaskOutput::TimeOut => {
                    log::debug!("[sys_ppoll]: timeout");
                    return Ok(0);
                }
            }
        } else {
            let intr_future = IntrBySignalFuture {
                task: task.clone(),
                mask: *task.sig_mask_ref(),
            };
            match Select2Futures::new(poll_future, intr_future).await {
                SelectOutput::Output1(ret_vec) => ret_vec,
                SelectOutput::Output2(_) => return Err(SysError::EINTR),
            }
        };
        task.set_running();

        let ret = ret_vec.len();
        for (i, result) in ret_vec {
            poll_fds[i].revents |= result
        }
        poll_fds_slice.copy_from_slice(&poll_fds);

        if let Some(old_mask) = old_mask {
            *task.sig_mask() = old_mask;
        }
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
        sigmask: UserReadPtr<SigSet>,
    ) -> SyscallResult {
        let task = self.task;
        if nfds < 0 {
            return Err(SysError::EINVAL);
        }
        let nfds = nfds as usize;
        let timeout = if timeout.is_null() {
            None
        } else {
            Some(timeout.read(task)?.into())
        };
        let new_mask = if sigmask.is_null() {
            None
        } else {
            Some(sigmask.read(task)?)
        };

        log::info!("[sys_pselect6] nfds:{nfds}, readfds:{readfds}, writefds:{writefds}, exceptfds:{exceptfds}, timeout:{timeout:?}, sigmask:{new_mask:?}");

        let mut readfds = if readfds.is_null() {
            None
        } else {
            let readfds = readfds.into_mut(task)?;
            log::info!("readfds: {:?}", &readfds.fds_bits);
            Some(readfds)
        };
        let mut writefds = if writefds.is_null() {
            None
        } else {
            let writefds = writefds.into_mut(task)?;
            log::info!("writefds: {:?}", &writefds.fds_bits);
            Some(writefds)
        };
        let mut exceptfds = if exceptfds.is_null() {
            None
        } else {
            let exceptfds = exceptfds.into_mut(task)?;
            log::info!("exceptfds: {:?}", &exceptfds.fds_bits);
            Some(exceptfds)
        };

        let mut polls = Vec::<(Fd, PollEvents, Arc<dyn File>)>::with_capacity(nfds as usize);
        for fd in 0..nfds as usize {
            let mut events = PollEvents::empty();
            readfds.as_ref().map(|fds| {
                if fds.is_set(fd) {
                    events.insert(PollEvents::IN)
                }
            });
            writefds.as_ref().map(|fds| {
                if fds.is_set(fd) {
                    events.insert(PollEvents::OUT)
                }
            });
            if !events.is_empty() {
                let file = task.with_fd_table(|f| f.get_file(fd))?;
                log::debug!("fd:{fd}, file path:{}", file.dentry().path());
                polls.push((fd, events, file));
            }
        }

        let old_mask = if let Some(mask) = new_mask {
            Some(mem::replace(task.sig_mask(), mask))
        } else {
            None
        };
        task.set_interruptable();
        task.set_wake_up_signal(!*task.sig_mask_ref());
        let pselect_future = PSelectFuture { polls };
        let ret_vec = if let Some(timeout) = timeout {
            match TimeLimitedTaskFuture::new(timeout, pselect_future).await {
                TimeLimitedTaskOutput::Ok(ret_vec) => ret_vec,
                TimeLimitedTaskOutput::TimeOut => {
                    log::debug!("[sys_pselect6]: timeout");
                    return Ok(0);
                }
            }
        } else {
            let intr_future = IntrBySignalFuture {
                task: task.clone(),
                mask: *task.sig_mask_ref(),
            };
            match Select2Futures::new(pselect_future, intr_future).await {
                SelectOutput::Output1(ret_vec) => ret_vec,
                SelectOutput::Output2(_) => return Err(SysError::EINTR),
            }
        };

        readfds.as_mut().map(|fds| fds.clear());
        writefds.as_mut().map(|fds| fds.clear());
        exceptfds.as_mut().map(|fds| fds.clear());

        task.set_running();

        // restore old signal mask
        if let Some(mask) = old_mask {
            *task.sig_mask() = mask;
        }

        let mut ret = 0;
        for (fd, events) in ret_vec {
            if events.contains(PollEvents::IN) | events.contains(PollEvents::HUP) {
                readfds.as_mut().map(|fds| fds.set(fd));
                ret += 1;
            }
            if events.contains(PollEvents::OUT) {
                writefds.as_mut().map(|fds| fds.set(fd));
                ret += 1;
            }
        }
        Ok(ret)
    }
}
