use memory::VirtAddr;

#[repr(C)]
#[derive(Clone, Copy)]
struct SignalStack {
    /// Base address of stack
    ss_sp: VirtAddr,
    /// Flags
    ss_flags: i32,
    /// Number of bytes in stack
    ss_size: usize,
}

impl Default for SignalStack {
    fn default() -> Self {
        SignalStack {
            ss_sp: 0usize.into(),
            ss_flags: 0,
            ss_size: 0,
        }
    }
}
