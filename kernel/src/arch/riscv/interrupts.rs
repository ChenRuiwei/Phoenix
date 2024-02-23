use riscv::register::sstatus;

#[inline(always)]
pub unsafe fn disable() {
    sstatus::clear_sie();
}

#[inline(always)]
pub unsafe fn enable() {
    sstatus::set_sie();
}

#[inline(always)]
pub fn is_enabled() -> bool {
    sstatus::read().sie()
}
