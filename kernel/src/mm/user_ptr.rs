//! UserPtr
//! 这个模块用来绕过裸指针的异步 Send 检查
//!
//! Adapted from FTL OS

use alloc::{string::String, sync::Arc, vec::Vec};
use core::{
    fmt::{Display, Formatter},
    intrinsics::size_of,
    marker::PhantomData,
    ops::ControlFlow,
};

use memory::{StepByOne, VirtAddr};
use riscv::register::scause;
use systype::{SysError, SysResult};

use crate::{
    task::Task,
    trap::{
        kernel_trap::{set_kernel_user_rw_trap, will_read_fail, will_write_fail},
        set_kernel_trap,
    },
};

pub trait Policy: Clone + Copy + 'static {}

pub trait Read: Policy {}
pub trait Write: Policy {}

#[derive(Clone, Copy)]
pub struct In;
#[derive(Clone, Copy)]
pub struct Out;
#[derive(Clone, Copy)]
pub struct InOut;

impl Policy for In {}
impl Policy for Out {}
impl Policy for InOut {}
impl Read for In {}
impl Write for Out {}
impl Read for InOut {}
impl Write for InOut {}

// PERF: UserPtr will check multiple times when read or something are called
// multiple times.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct UserPtr<T: Clone + Copy + 'static, P: Policy> {
    ptr: *mut T,
    _mark: PhantomData<P>,
}

pub type UserReadPtr<T> = UserPtr<T, In>;
pub type UserWritePtr<T> = UserPtr<T, Out>;
pub type UserInOutPtr<T> = UserPtr<T, InOut>;

unsafe impl<T: Clone + Copy + 'static, P: Policy> Send for UserPtr<T, P> {}
unsafe impl<T: Clone + Copy + 'static, P: Policy> Sync for UserPtr<T, P> {}

impl<T: Clone + Copy + 'static, P: Policy> UserPtr<T, P> {
    fn new(ptr: *mut T) -> Self {
        Self {
            ptr,
            _mark: PhantomData,
        }
    }

    pub fn null() -> Self {
        Self::new(core::ptr::null_mut())
    }
    pub fn from_usize(a: usize) -> Self {
        Self::new(a as *mut T)
    }
    pub fn is_null(self) -> bool {
        self.ptr.is_null()
    }
    pub fn not_null(self) -> bool {
        !self.ptr.is_null()
    }
    pub fn as_usize(self) -> usize {
        self.ptr as usize
    }
    pub fn raw_ptr(self) -> *const T {
        self.ptr
    }
    /// return None if UserAddr == nullptr
    pub fn as_ptr(self) -> Option<*const T> {
        if self.ptr.is_null() {
            return None;
        }
        Some(self.ptr)
    }
    pub fn offset(self, count: isize) -> Self {
        Self::new(unsafe { self.ptr.offset(count) })
    }
    pub fn transmute<V: Clone + Copy + 'static>(self) -> UserPtr<V, P> {
        UserPtr::new(self.ptr as *mut V)
    }
    pub fn add(self, count: usize) -> Self {
        Self::new(unsafe { self.ptr.add(count) })
    }
}
impl<T: Clone + Copy + 'static, P: Read> UserPtr<T, P> {
    pub fn nonnull(self) -> Option<Self> {
        self.not_null().then_some(self)
    }

    pub fn as_ref(self, task: &Arc<Task>) -> SysResult<&T> {
        debug_assert!(self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>(),
            PageFaultAccessType::RO,
        )?;
        let res = unsafe { &*self.ptr };
        Ok(res)
    }

    pub fn as_slice(self, n: usize, task: &Arc<Task>) -> SysResult<&[T]> {
        debug_assert!(n == 0 || self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>() * n,
            PageFaultAccessType::RO,
        )?;
        let res = unsafe { core::slice::from_raw_parts(self.ptr, n) };
        Ok(res)
    }

    pub fn read(self, task: &Arc<Task>) -> SysResult<T> {
        debug_assert!(self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>(),
            PageFaultAccessType::RO,
        )?;
        let res = unsafe { core::ptr::read(self.ptr) };
        Ok(res)
    }

    pub fn read_array(self, n: usize, task: &Arc<Task>) -> SysResult<Vec<T>> {
        debug_assert!(n == 0 || self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>() * n,
            PageFaultAccessType::RO,
        )?;

        let mut res = Vec::with_capacity(n);
        unsafe {
            let ptr = self.ptr;
            for i in 0..n {
                res.push(ptr.add(i).read());
            }
        }

        Ok(res)
    }
}

impl<P: Read> UserPtr<u8, P> {
    pub fn read_cstr(self, task: &Arc<Task>) -> SysResult<String> {
        debug_assert!(self.not_null());
        let mut str = String::with_capacity(32);
        let mut has_ended = false;

        task.ensure_user_area(
            VirtAddr::from(self.as_usize()),
            usize::MAX,
            PageFaultAccessType::RO,
            |beg, len| unsafe {
                let mut ptr = beg.as_mut_ptr();
                for _ in 0..len {
                    let c = ptr.read();
                    if c == 0 {
                        has_ended = true;
                        return ControlFlow::Break(None);
                    }
                    str.push(c as char);
                    ptr = ptr.offset(1);
                }
                ControlFlow::Continue(())
            },
        )?;

        if has_ended {
            Ok(str)
        } else {
            Err(SysError::EINVAL)
        }
    }
}

impl<T: Clone + Copy + 'static, P: Write> UserPtr<T, P> {
    pub fn raw_ptr_mut(self) -> *mut T {
        self.ptr
    }
    pub fn nonnull_mut(self) -> Option<Self> {
        self.not_null().then_some(self)
    }

    pub fn as_mut(self, task: &Arc<Task>) -> SysResult<&mut T> {
        debug_assert!(self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>(),
            PageFaultAccessType::RW,
        )?;
        let res = unsafe { &mut *self.ptr };
        Ok(res)
    }

    pub fn as_mut_slice(self, n: usize, task: &Arc<Task>) -> SysResult<&mut [T]> {
        debug_assert!(n == 0 || self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>() * n,
            PageFaultAccessType::RW,
        )?;
        let res = unsafe { core::slice::from_raw_parts_mut(self.ptr, n) };
        Ok(res)
    }

    pub fn write(self, task: &Arc<Task>, val: T) -> SysResult<()> {
        debug_assert!(self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>(),
            PageFaultAccessType::RW,
        )?;
        unsafe { core::ptr::write(self.ptr, val) };
        Ok(())
    }

    pub fn write_array(self, task: &Arc<Task>, val: &[T]) -> SysResult<()> {
        debug_assert!(self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>() * val.len(),
            PageFaultAccessType::RW,
        )?;
        unsafe {
            let mut ptr = self.ptr;
            for &v in val {
                ptr.write(v);
                ptr = ptr.offset(1);
            }
        }
        Ok(())
    }
}

impl<P: Write> UserPtr<u8, P> {
    /// should only be used at syscall getdent with dynamic-len structure
    pub unsafe fn write_as_bytes<U>(self, task: &Arc<Task>, val: &U) -> SysResult<()> {
        debug_assert!(self.not_null());

        let len = size_of::<U>();
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            len,
            PageFaultAccessType::RW,
        )?;

        unsafe {
            let view = core::slice::from_raw_parts(val as *const U as *const u8, len);
            let mut ptr = self.ptr as *mut u8;
            for &c in view {
                ptr.write(c);
                ptr = ptr.offset(1);
            }
        }
        Ok(())
    }

    pub fn write_cstr(self, task: &Arc<Task>, val: &str) -> SysResult<()> {
        debug_assert!(self.not_null());

        let mut str = val.as_bytes();
        let mut has_filled_zero = false;

        task.ensure_user_area(
            VirtAddr::from(self.as_usize()),
            val.len() + 1,
            PageFaultAccessType::RW,
            |beg, len| unsafe {
                let mut ptr = beg.as_mut_ptr();
                let writable_len = len.min(str.len());
                for _ in 0..writable_len {
                    let c = str[0];
                    str = &str[1..];
                    ptr.write(c);
                    ptr = ptr.offset(1);
                }
                if str.is_empty() && writable_len < len {
                    ptr.write(0);
                    has_filled_zero = true;
                }
                ControlFlow::Continue(())
            },
        )?;

        if has_filled_zero {
            Ok(())
        } else {
            Err(SysError::EINVAL)
        }
    }
}

impl<T: Clone + Copy + 'static, P: Policy> From<usize> for UserPtr<T, P> {
    fn from(a: usize) -> Self {
        Self::from_usize(a)
    }
}

impl<T: Clone + Copy + 'static, P: Policy> Display for UserPtr<T, P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "UserPtr({:#x})", self.as_usize())
    }
}

impl Task {
    fn just_ensure_user_area(
        &self,
        begin: VirtAddr,
        len: usize,
        access: PageFaultAccessType,
    ) -> SysResult<()> {
        self.ensure_user_area(begin, len, access, |_, _| ControlFlow::Continue(()))
    }

    /// ensure that the whole range is accessible, or return an error
    fn ensure_user_area(
        &self,
        begin: VirtAddr,
        len: usize,
        access: PageFaultAccessType,
        mut f: impl FnMut(VirtAddr, usize) -> ControlFlow<Option<SysError>>,
    ) -> SysResult<()> {
        if len == 0 {
            return Ok(());
        }

        unsafe { set_kernel_user_rw_trap() };

        let test_fn = match access {
            PageFaultAccessType::RO => will_read_fail,
            PageFaultAccessType::RW => will_write_fail,
            _ => panic!("invalid access type"),
        };

        let mut curr_vaddr = begin;
        let mut readable_len = 0;
        while readable_len < len {
            if test_fn(curr_vaddr.0) {
                // TODO: handle_pagefault
                // self.with_mut_memory(|m| m.handle_pagefault(curr_vaddr,
                // access))     .map_err(|_| SysError::EFAULT)?;
            }

            let next_page_beg: VirtAddr = VirtAddr::from(curr_vaddr.floor().next());
            let len = next_page_beg - curr_vaddr;

            match f(curr_vaddr, len) {
                ControlFlow::Continue(_) => {}
                ControlFlow::Break(None) => {
                    unsafe { set_kernel_trap() };
                    return Ok(());
                }
                ControlFlow::Break(Some(e)) => {
                    unsafe { set_kernel_trap() };
                    return Err(e);
                }
            }

            readable_len += len;
            curr_vaddr = next_page_beg;
        }

        unsafe { set_kernel_trap() };
        Ok(())
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PageFaultAccessType: u8 {
        const READ = 1 << 1;
        const WRITE = 1 << 2;
        const EXECUTE = 1 << 3;
    }
}

impl PageFaultAccessType {
    // no write & no execute == read only
    pub const RO: Self = Self::READ;
    // can't use | (bits or) here
    // see https://github.com/bitflags/bitflags/issues/180
    pub const RW: Self = Self::RO.union(Self::WRITE);
    pub const RX: Self = Self::RO.union(Self::EXECUTE);

    pub fn from_exception(e: scause::Exception) -> Self {
        match e {
            scause::Exception::InstructionPageFault => Self::RX,
            scause::Exception::LoadPageFault => Self::RO,
            scause::Exception::StorePageFault => Self::RW,
            _ => panic!("unexcepted exception type for PageFaultAccessType"),
        }
    }
}
