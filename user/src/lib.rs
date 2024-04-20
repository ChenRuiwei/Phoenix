#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod error;
mod lang_items;
mod syscall;
pub mod types;

#[macro_use]
extern crate bitflags;
extern crate alloc;

use alloc::vec::Vec;
use core::ptr::null;

use buddy_system_allocator::LockedHeap;
pub use error::SyscallErr;
use syscall::*;
use types::*;

// const USER_HEAP_SIZE: usize = 16384;
const USER_HEAP_SIZE: usize = 0x32000;

// Note that heap space is allocated in .data segment
// TODO: can we change to dynamically allocate by invoking sys_sbrk?
// static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    unsafe {
        // HEAP.lock()
        //     .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);

        const HEAP_START: usize = 0x0000_0002_0000_0000;
        sys_brk(HEAP_START + USER_HEAP_SIZE);
        HEAP.lock().init(HEAP_START, USER_HEAP_SIZE);
    }
    let mut v: Vec<&'static str> = Vec::new();
    for i in 0..argc {
        let str_start =
            unsafe { ((argv + i * core::mem::size_of::<usize>()) as *const usize).read_volatile() };
        let len = (0usize..)
            .find(|i| unsafe { ((str_start + *i) as *const u8).read_volatile() == 0 })
            .unwrap();
        v.push(
            core::str::from_utf8(unsafe {
                core::slice::from_raw_parts(str_start as *const u8, len)
            })
            .unwrap(),
        );
    }
    let exit_code = main(argc, v.as_slice());
    // println!("program {} will exit", v[0]);
    exit(exit_code);
}

#[linkage = "weak"]
#[no_mangle]
fn main(_: usize, _: &[&str]) -> i32 {
    panic!("Cannot find main!");
}

#[macro_export]
macro_rules! wexitstatus {
    ($a:expr) => {
        ($a & 0xffffff00) >> 8
    };
}

// pub fn getcwd(path: usize, len: usize) -> isize {
//     sys_getcwd(path, len)
// }

// pub fn mount(dev_name: usize, target_path: usize, ftype: usize, flags: u32,
// data: usize) -> isize {     sys_mount(dev_name, target_path, ftype, flags,
// data) }

// pub fn uname(buf: usize) -> isize {
//     sys_uname(buf)
// }

//************file system***************/
pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
}
pub fn dup3(oldfd: usize, newfd: usize, flags: OpenFlags) -> isize {
    sys_dup3(oldfd, newfd, flags.bits() as usize)
}
pub fn openat(path: &str, flags: OpenFlags) -> isize {
    // TODO: change to the version that has `mode` arg
    sys_openat(AT_FDCWD as usize, path.as_ptr(), flags.bits() as usize, 0)
}
pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf.as_mut_ptr(), buf.len())
}
pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf.as_ptr(), buf.len())
}
pub fn mmap(
    addr: *const u8,
    length: usize,
    prot: i32,
    flags: i32,
    fd: usize,
    offset: usize,
) -> isize {
    sys_mmap(
        addr as usize,
        length,
        prot as usize,
        flags as usize,
        fd,
        offset,
    )
}

//************ task ***************/
pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code);
    loop {}
}
pub fn exit_group(exit_code: i32) -> ! {
    sys_exit_group(exit_code);
    loop {}
}
pub fn yield_() -> isize {
    sys_yield()
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn fork() -> isize {
    sys_fork()
}
pub fn kill(pid: isize, sig: Sig) -> isize {
    sys_kill(pid as usize, sig.raw() as i32)
}
pub fn execve(cmd: &str, args: &[*const u8], env: &[*const u8]) -> isize {
    sys_execve(
        cmd.as_ptr(),
        args.as_ptr() as *const usize,
        env.as_ptr() as *const usize,
    )
}

pub fn wait(exit_code: &mut i32) -> isize {
    sys_waitpid(-1, exit_code as *mut _)
}

pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    sys_waitpid(pid as isize, exit_code as *mut _)
}

pub fn pipe(pipe_fd: &mut [i32]) -> isize {
    sys_pipe(pipe_fd.as_mut_ptr())
}

pub fn close(fd: usize) -> isize {
    sys_close(fd)
}

//************ time ***************/
pub fn get_timeofday(time_val: &mut TimeVal) -> isize {
    sys_gettimeofday(time_val as *mut TimeVal as *mut usize, 0 as *mut usize)
}

//************ signal ***************/
pub fn sigaction(sig_no: Sig, act: &SigAction, old_act: &mut SigAction) -> isize {
    sys_sigaction(
        sig_no.index(),
        act as *const SigAction as *const usize,
        old_act as *mut SigAction as *mut usize,
    )
}

pub fn sigreturn() -> isize {
    sys_sigreturn()
}
