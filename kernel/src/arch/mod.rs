#[cfg(target_arch = "riscv64")]
mod riscv;

#[cfg(target_arch = "riscv64")]
pub use self::riscv::*;
