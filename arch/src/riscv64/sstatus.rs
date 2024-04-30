use core::arch::asm;

use bit_field::BitField;
use riscv::register::sstatus::{FS, SPP};

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Sstatus {
    bits: usize,
}

impl Sstatus {
    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn fs(&self) -> FS {
        match self.bits.get_bits(13..15) {
            0 => FS::Off,
            1 => FS::Initial,
            2 => FS::Clean,
            3 => FS::Dirty,
            _ => unreachable!(),
        }
    }

    pub fn set_spie(&mut self, val: bool) {
        self.bits.set_bit(5, val);
    }

    pub fn set_sie(&mut self, val: bool) {
        self.bits.set_bit(1, val);
    }

    pub fn set_spp(&mut self, spp: SPP) {
        self.bits.set_bit(8, spp == SPP::Supervisor);
    }

    pub fn set_fs(&mut self, fs: FS) {
        let v: u8 = unsafe { core::mem::transmute(fs) };
        self.bits.set_bits(13..15, v as usize);
    }

    pub fn empty() -> Self {
        Self { bits: 0 }
    }
}

pub fn read() -> Sstatus {
    let bits: usize;
    unsafe {
        asm!("csrr {}, sstatus", out(reg) bits);
    }
    Sstatus { bits }
}

pub fn write(sstatus: usize) {
    let bits = sstatus;
    unsafe {
        asm!("csrw sstatus, {}", in(reg) bits);
    }
}
