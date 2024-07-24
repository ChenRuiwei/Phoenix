//! # UserPtr
//!
//! Used for automatically check user ptr when reading or writing.

use alloc::{string::String, sync::Arc, vec::Vec};
use core::{
    fmt::{self, Debug},
    intrinsics::{atomic_load_acquire, size_of},
    marker::PhantomData,
    mem,
    net::Ipv4Addr,
    ops::{self, ControlFlow},
};

use memory::VirtAddr;
use net::{IpAddress, IpEndpoint};
use riscv::register::scause;
use systype::{SysError, SysResult};

use super::memory_space::vm_area::MapPerm;
use crate::{
    net::{
        addr::{SockAddrIn, SockAddrIn6},
        SaFamily,
    },
    processor::{env::SumGuard, hart::current_task_ref},
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

/// Checks user ptr automatically when reading or writing.
///
/// It will be consumed once being used.
pub struct UserPtr<T: Clone + Copy + 'static, P: Policy> {
    ptr: *mut T,
    _mark: PhantomData<P>,
    _guard: SumGuard,
}

pub type UserReadPtr<T> = UserPtr<T, In>;
pub type UserWritePtr<T> = UserPtr<T, Out>;
pub type UserRdWrPtr<T> = UserPtr<T, InOut>;

unsafe impl<T: Clone + Copy + 'static, P: Policy> Send for UserPtr<T, P> {}
unsafe impl<T: Clone + Copy + 'static, P: Policy> Sync for UserPtr<T, P> {}

macro_rules! impl_fmt {
    ($name:ident) => {
        impl<T: Clone + Copy + 'static> fmt::Display for $name<T> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}({:#x})", stringify!($name), self.as_usize())
            }
        }

        impl<T: Clone + Copy + 'static> fmt::Debug for $name<T> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("ptr", &self.ptr)
                    .finish()
            }
        }
    };
}

impl_fmt!(UserReadPtr);
impl_fmt!(UserWritePtr);
impl_fmt!(UserRdWrPtr);

pub struct UserRef<'a, T> {
    i_ref: &'a mut T,
    _guard: SumGuard,
}

impl<'a, T> UserRef<'a, T> {
    pub fn new(i_ref: &'a mut T) -> Self {
        Self {
            i_ref,
            _guard: SumGuard::new(),
        }
    }

    pub unsafe fn new_unchecked(ptr: *mut T) -> Self {
        let i_ref = unsafe { &mut *ptr };
        Self::new(i_ref)
    }

    pub fn ptr(&self) -> *const T {
        self.i_ref as _
    }

    pub fn ptr_mut(&mut self) -> *mut T {
        self.i_ref as _
    }
}

impl<'a, T> ops::Deref for UserRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.i_ref
    }
}

impl<T: Clone + Copy + 'static + Debug> Debug for UserRef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("UserRef").field("ref", &self.i_ref).finish()
    }
}

pub struct UserMut<'a, T> {
    i_ref: &'a mut T,
    _guard: SumGuard,
}

impl<'a, T> UserMut<'a, T> {
    pub fn new(i_ref: &'a mut T) -> Self {
        Self {
            i_ref,
            _guard: SumGuard::new(),
        }
    }

    pub unsafe fn new_unchecked(ptr: *mut T) -> Self {
        let i_ref = unsafe { &mut *ptr };
        Self::new(i_ref)
    }

    pub fn as_ptr(&self) -> *const T {
        self.i_ref as _
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.i_ref as _
    }
}

impl<'a, T> core::ops::Deref for UserMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.i_ref
    }
}

impl<'a, T> ops::DerefMut for UserMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.i_ref
    }
}

impl<T: Clone + Copy + 'static + Debug> Debug for UserMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("UserMut").field("mut", &self.i_ref).finish()
    }
}

/// User slice. Hold slice from `UserPtr` and a `SumGuard` to provide user
/// space access.
pub struct UserSlice<'a, T> {
    slice: &'a mut [T],
    _guard: SumGuard,
}

impl<'a, T> UserSlice<'a, T> {
    pub fn new(slice: &'a mut [T]) -> Self {
        Self {
            slice,
            _guard: SumGuard::new(),
        }
    }

    pub unsafe fn new_unchecked(va: VirtAddr, len: usize) -> Self {
        let slice = core::slice::from_raw_parts_mut(va.bits() as *mut T, len);
        Self::new(slice)
    }
}

impl<'a, T> core::ops::Deref for UserSlice<'a, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl<'a, T> core::ops::DerefMut for UserSlice<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.slice
    }
}

impl<T: Clone + Copy + 'static + Debug> Debug for UserSlice<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("UserSlice")
            .field("slice", &self.slice.iter())
            .finish()
    }
}

impl<T: Clone + Copy + 'static, P: Policy> UserPtr<T, P> {
    fn new(ptr: *mut T) -> Self {
        Self {
            ptr,
            _mark: PhantomData,
            _guard: SumGuard::new(),
        }
    }

    pub fn null() -> Self {
        Self::new(core::ptr::null_mut())
    }

    fn from_usize(vaddr: usize) -> Self {
        Self::new(vaddr as *mut T)
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn not_null(&self) -> bool {
        !self.ptr.is_null()
    }

    pub fn as_usize(&self) -> usize {
        self.ptr as usize
    }

    pub fn as_ptr(&self) -> *const T {
        self.ptr
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        self.ptr
    }
}

// TODO: consider return EFAULT when self is null.
impl<T: Clone + Copy + 'static, P: Read> UserPtr<T, P> {
    pub fn into_ref(self, task: &Arc<Task>) -> SysResult<UserRef<T>> {
        if self.is_null() {
            return Err(SysError::EFAULT);
        }
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>(),
            PageFaultAccessType::RO,
        )?;
        let res = unsafe { &mut *self.ptr };
        Ok(UserRef::new(res))
    }

    pub fn into_slice(self, task: &Arc<Task>, n: usize) -> SysResult<UserSlice<T>> {
        debug_assert!(self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>() * n,
            PageFaultAccessType::RO,
        )?;
        let slice = unsafe { core::slice::from_raw_parts_mut(self.ptr, n) };
        Ok(UserSlice::new(slice))
    }

    pub fn read(self, task: &Arc<Task>) -> SysResult<T> {
        if self.is_null() {
            log::warn!("[UserReadPtr] null ptr");
            return Err(SysError::EFAULT);
        }
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>(),
            PageFaultAccessType::RO,
        )?;
        let res = unsafe { core::ptr::read(self.ptr) };
        Ok(res)
    }

    pub fn read_array(self, task: &Arc<Task>, n: usize) -> SysResult<Vec<T>> {
        debug_assert!(self.not_null());
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

    /// Read a pointer vector (a.k.a 2d array) that ends with null, e.g. argv,
    /// envp.
    pub fn read_cvec(self, task: &Arc<Task>) -> SysResult<Vec<usize>> {
        debug_assert!(self.not_null());
        let mut vec = Vec::with_capacity(32);
        task.ensure_user_area(
            VirtAddr::from(self.as_usize()),
            usize::MAX,
            PageFaultAccessType::RO,
            |beg, len| unsafe {
                let mut ptr = beg.0 as *const usize;
                for _ in 0..len {
                    let c = ptr.read();
                    if c == 0 {
                        return ControlFlow::Break(None);
                    }
                    vec.push(c);
                    ptr = ptr.offset(1);
                }
                ControlFlow::Continue(())
            },
        )?;
        Ok(vec)
    }
}

impl<P: Read> UserPtr<u8, P> {
    // TODO: set length limit to cstr
    pub fn read_cstr(self, task: &Arc<Task>) -> SysResult<String> {
        debug_assert!(self.not_null());
        let mut str = String::with_capacity(32);

        task.ensure_user_area(
            VirtAddr::from(self.as_usize()),
            usize::MAX,
            PageFaultAccessType::RO,
            |beg, len| unsafe {
                let mut ptr = beg.as_mut_ptr();
                for _ in 0..len {
                    let c = ptr.read();
                    if c == 0 {
                        return ControlFlow::Break(None);
                    }
                    str.push(c as char);
                    ptr = ptr.offset(1);
                }
                ControlFlow::Continue(())
            },
        )?;
        Ok(str)
    }
}

// TODO: should ref hold SumGuard?
impl<T: Clone + Copy + 'static, P: Write> UserPtr<T, P> {
    pub fn into_mut(self, task: &Arc<Task>) -> SysResult<UserMut<T>> {
        debug_assert!(self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>(),
            PageFaultAccessType::RW,
        )?;
        let res = unsafe { &mut *self.ptr };
        Ok(UserMut::new(res))
    }

    pub fn into_mut_slice(self, task: &Arc<Task>, n: usize) -> SysResult<UserSlice<T>> {
        debug_assert!(self.not_null());
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>() * n,
            PageFaultAccessType::RW,
        )?;
        // WARN: `core::slice::from_raw_parts_mut` does not accept null pointer even for
        // zero length slice, hidden bug may be caused when doing so.
        let slice = unsafe { core::slice::from_raw_parts_mut(self.ptr, n) };
        Ok(UserSlice::new(slice))
    }

    pub fn write(self, task: &Arc<Task>, val: T) -> SysResult<()> {
        debug_assert!(self.not_null());
        if !Arc::ptr_eq(task, current_task_ref()) {
            unsafe { task.switch_page_table() };
        }
        task.just_ensure_user_area(
            VirtAddr::from(self.as_usize()),
            size_of::<T>(),
            PageFaultAccessType::RW,
        )?;
        unsafe { core::ptr::write(self.ptr, val) };
        if !Arc::ptr_eq(task, current_task_ref()) {
            unsafe { current_task_ref().switch_page_table() };
        }
        Ok(())
    }

    pub fn write_unchecked(self, _task: &Arc<Task>, val: T) -> SysResult<()> {
        debug_assert!(self.not_null());
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

    pub fn write_cstr_unchecked(self, _task: &Arc<Task>, val: &str) -> SysResult<()> {
        debug_assert!(self.not_null());
        let bytes = val.as_bytes();
        let mut ptr = self.as_mut_ptr();
        for byte in bytes {
            unsafe {
                ptr.write(*byte);
                ptr = ptr.offset(1)
            };
        }
        unsafe { ptr.write(0) };
        Ok(())
    }
}

impl<T: Clone + Copy + 'static, P: Policy> From<usize> for UserPtr<T, P> {
    fn from(a: usize) -> Self {
        Self::from_usize(a)
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

    /// Ensure that the whole range is accessible, or return an error.
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
                self.with_mut_memory_space(|m| m.handle_page_fault(curr_vaddr, access))?
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
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
    }
}

impl PageFaultAccessType {
    pub const RO: Self = Self::READ;
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

    pub fn can_access(self, flag: MapPerm) -> bool {
        if self.contains(Self::WRITE) && !flag.contains(MapPerm::W) {
            return false;
        }
        if self.contains(Self::EXECUTE) && !flag.contains(MapPerm::X) {
            return false;
        }
        true
    }
}

pub struct FutexAddr {
    pub addr: VirtAddr,
    _guard: SumGuard,
}

impl FutexAddr {
    pub fn raw(&self) -> usize {
        self.addr.into()
    }
    pub fn check(&self, task: &Arc<Task>) -> SysResult<()> {
        task.just_ensure_user_area(self.addr, size_of::<VirtAddr>(), PageFaultAccessType::RO)
    }
    pub fn read(&self) -> u32 {
        unsafe { atomic_load_acquire(self.addr.0 as *const u32) }
    }
}

impl From<usize> for FutexAddr {
    fn from(a: usize) -> Self {
        Self {
            addr: a.into(),
            _guard: SumGuard::new(),
        }
    }
}

impl Task {
    /// translate SockAddr to IpEndpoint
    ///
    /// The `addr` parameter is a pointer to a `SockAddr` structure passed by
    /// the system call, and `addr_len` is the length of the address pointed
    /// to by this pointer.
    ///
    /// First, the function checks whether the address pointed to by the pointer
    /// has read permissions. Then, based on the `sa_family` member of
    /// theSockAddr structure, it determines which variant of the `SockAddr`
    /// enum the user-provided parameter corresponds to.
    pub fn audit_sockaddr(&self, addr: usize, addrlen: usize) -> SysResult<IpEndpoint> {
        let _guard = SumGuard::new();
        self.just_ensure_user_area(addr.into(), addrlen, PageFaultAccessType::RO)?;
        let family = SaFamily::try_from(unsafe { *(addr as *const u16) })?;
        match family {
            SaFamily::AF_INET => {
                if addrlen < mem::size_of::<SockAddrIn>() {
                    log::error!("[audit_sockaddr] AF_INET addrlen error");
                    return Err(SysError::EINVAL);
                }
                let sock_addr_in = unsafe { *(addr as *const SockAddrIn) };
                // this will convert network byte order to host byte order
                Ok(IpEndpoint::from(sock_addr_in))
            }
            SaFamily::AF_INET6 => {
                if addrlen < mem::size_of::<SockAddrIn6>() {
                    log::error!("[audit_sockaddr] AF_INET6 addrlen error");
                    return Err(SysError::EINVAL);
                }
                let sock_addr_in6: SockAddrIn6 = unsafe { *(addr as *const _) };
                // this will convert network byte order to host byte order
                Ok(IpEndpoint::from(sock_addr_in6))
            }
            SaFamily::AF_UNIX => unimplemented!(),
        }
    }

    pub fn write_sockaddr(
        self: &Arc<Task>,
        addr: usize,
        addrlen: usize,
        endpoint: IpEndpoint,
    ) -> SysResult<()> {
        if addr == 0 {
            return Ok(());
        }
        match endpoint.addr {
            IpAddress::Ipv4(_) => {
                UserWritePtr::<SockAddrIn>::from(addr).write(self, SockAddrIn::from(endpoint))?;
                UserWritePtr::<u32>::from(addrlen)
                    .write(self, mem::size_of::<SockAddrIn>() as u32)?;
            }
            IpAddress::Ipv6(_) => {
                UserWritePtr::<SockAddrIn6>::from(addr).write(self, endpoint.into())?;
                UserWritePtr::<u32>::from(addrlen)
                    .write(self, mem::size_of::<SockAddrIn6>() as u32)?;
            }
        }
        Ok(())
    }
}
