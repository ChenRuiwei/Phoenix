//! Implementation of [`TrapContext`]

use core::arch::asm;

use arch::sstatus::{self, Sstatus};
use riscv::register::sstatus::{FS, SPP};

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TrapContext {
    // User to kernel should save:
    /// general regs[0..31]
    pub user_x: [usize; 32],
    /// CSR sstatus
    pub sstatus: Sstatus, // 32
    /// CSR sepc
    pub sepc: usize, // 33

    // Kernel to user should save:
    pub kernel_sp: usize, // 34
    ///
    pub kernel_ra: usize, // 35
    ///
    pub kernel_s: [usize; 12], // 36 - 47
    ///
    pub kernel_fp: usize, // 48
    /// kernel hart address
    pub kernel_tp: usize, // 49
    /// Float regs
    /// TODO: add dirty flag to know whether we should save
    pub user_fx: UserFloatContext,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct UserFloatContext {
    pub user_fx: [f64; 32], // 50 - 81
    pub fcsr: u32,          // 32bit
    pub need_save: u8,
    pub need_restore: u8,
    pub signal_dirty: u8,
}

impl UserFloatContext {
    pub fn new() -> Self {
        unsafe { core::mem::zeroed() }
    }

    pub fn mark_save_if_needed(&mut self, sstatus: Sstatus) {
        self.need_save |= (sstatus.fs() == FS::Dirty) as u8;
        self.signal_dirty |= (sstatus.fs() == FS::Dirty) as u8;
    }

    pub fn yield_task(&mut self) {
        self.save();
        self.need_restore = 1;
    }

    pub fn encounter_signal(&mut self) {
        self.save();
    }

    /// Save reg -> mem
    pub fn save(&mut self) {
        if self.need_save == 0 {
            return;
        }
        self.need_save = 0;
        unsafe {
            let mut _t: usize = 1; // alloc a register but not zero.
            asm!("
            fsd  f0,  0*8({0})
            fsd  f1,  1*8({0})
            fsd  f2,  2*8({0})
            fsd  f3,  3*8({0})
            fsd  f4,  4*8({0})
            fsd  f5,  5*8({0})
            fsd  f6,  6*8({0})
            fsd  f7,  7*8({0})
            fsd  f8,  8*8({0})
            fsd  f9,  9*8({0})
            fsd f10, 10*8({0})
            fsd f11, 11*8({0})
            fsd f12, 12*8({0})
            fsd f13, 13*8({0})
            fsd f14, 14*8({0})
            fsd f15, 15*8({0})
            fsd f16, 16*8({0})
            fsd f17, 17*8({0})
            fsd f18, 18*8({0})
            fsd f19, 19*8({0})
            fsd f20, 20*8({0})
            fsd f21, 21*8({0})
            fsd f22, 22*8({0})
            fsd f23, 23*8({0})
            fsd f24, 24*8({0})
            fsd f25, 25*8({0})
            fsd f26, 26*8({0})
            fsd f27, 27*8({0})
            fsd f28, 28*8({0})
            fsd f29, 29*8({0})
            fsd f30, 30*8({0})
            fsd f31, 31*8({0})
            csrr {1}, fcsr
            sw  {1}, 32*8({0})
        ", in(reg) self,
                inout(reg) _t
            );
        };
    }

    /// Restore mem -> reg
    pub fn restore(&mut self) {
        if self.need_restore == 0 {
            return;
        }
        self.need_restore = 0;
        unsafe {
            asm!("
            fld  f0,  0*8({0})
            fld  f1,  1*8({0})
            fld  f2,  2*8({0})
            fld  f3,  3*8({0})
            fld  f4,  4*8({0})
            fld  f5,  5*8({0})
            fld  f6,  6*8({0})
            fld  f7,  7*8({0})
            fld  f8,  8*8({0})
            fld  f9,  9*8({0})
            fld f10, 10*8({0})
            fld f11, 11*8({0})
            fld f12, 12*8({0})
            fld f13, 13*8({0})
            fld f14, 14*8({0})
            fld f15, 15*8({0})
            fld f16, 16*8({0})
            fld f17, 17*8({0})
            fld f18, 18*8({0})
            fld f19, 19*8({0})
            fld f20, 20*8({0})
            fld f21, 21*8({0})
            fld f22, 22*8({0})
            fld f23, 23*8({0})
            fld f24, 24*8({0})
            fld f25, 25*8({0})
            fld f26, 26*8({0})
            fld f27, 27*8({0})
            fld f28, 28*8({0})
            fld f29, 29*8({0})
            fld f30, 30*8({0})
            fld f31, 31*8({0})
            lw  {0}, 32*8({0})
            csrw fcsr, {0}
        ", in(reg) self
            );
        }
    }
}

impl TrapContext {
    /// Init user context
    pub fn new(entry: usize, sp: usize) -> Self {
        let mut sstatus = sstatus::read();
        // set CPU privilege to User after trap return
        sstatus.set_spp(SPP::User);
        sstatus.set_sie(false);
        sstatus.set_spie(false);
        let mut cx = Self {
            user_x: [0; 32],
            sstatus,
            sepc: entry,
            // The following regs will be stored in asm funciton __restore
            // So we don't need to save them here
            kernel_sp: 0,
            kernel_ra: 0,
            kernel_s: [0; 12],
            kernel_fp: 0,
            // We will give the right kernel tp in `__return_to_user`
            kernel_tp: 0,
            user_fx: UserFloatContext::new(),
        };
        cx.set_user_sp(sp);
        cx
    }

    pub fn init_user(
        &mut self,
        user_sp: usize,
        sepc: usize,
        argc: usize,
        argv: usize,
        envp: usize,
    ) {
        self.user_x[2] = user_sp;
        self.user_x[10] = argc;
        self.user_x[11] = argv;
        self.user_x[12] = envp;
        self.sepc = sepc;
        self.sstatus = sstatus::read();
        self.user_fx = UserFloatContext::new()
    }

    /// Set stack pointer to x_2 reg (sp)
    pub fn set_user_sp(&mut self, sp: usize) {
        // sp == x2
        self.user_x[2] = sp;
    }

    pub fn set_user_a0(&mut self, val: usize) {
        // a0 == x10
        self.user_x[10] = val;
    }

    /// Syscall number
    pub fn syscall_no(&self) -> usize {
        // a7 == x17
        self.user_x[17]
    }

    /// Set entry point
    pub fn set_entry_point(&mut self, entry: usize) {
        self.sepc = entry;
    }

    pub fn set_user_pc_to_next(&mut self) {
        self.sepc += 4;
    }
}

#[derive(Clone, Copy)]
pub struct UserContext {
    /// general regs[0..31]
    pub user_x: [usize; 32],
    /// general float regs
    pub user_fx: UserFloatContext,
    /// CSR sstatus      
    pub sstatus: Sstatus, // 32
    /// CSR sepc
    pub sepc: usize, // 33
}

impl UserContext {
    pub fn from_trap_context(trap_context: &TrapContext) -> Self {
        Self {
            user_x: trap_context.user_x,
            user_fx: trap_context.user_fx,
            sstatus: trap_context.sstatus,
            sepc: trap_context.sepc,
        }
    }
}

