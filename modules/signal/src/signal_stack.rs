use memory::VirtAddr;

use crate::sigset::SigSet;

#[repr(C)]
#[derive(Clone, Copy)]
/// 信号栈是为信号处理程序执行提供的专用栈空间.它通常包含以下内容:
/// 1.信号上下文：这是信号处理程序运行时的上下文信息，包括所有寄存器的值、
/// 程序计数器（PC）、栈指针等。它使得信号处理程序可以访问到被中断的程序的状态，
/// 并且在处理完信号后能够恢复这个状态，继续执行原程序。
/// 2.信号信息（siginfo_t）：这个结构提供了关于信号的具体信息，如信号的来源、
/// 产生信号的原因等。 3.调用栈帧：如果信号处理程序调用了其他函数，
/// 那么这些函数的栈帧也会被压入信号栈。每个栈帧通常包含了函数参数、
/// 局部变量以及返回地址。 4.信号处理程序的返回地址：当信号处理程序完成执行后，
/// 系统需要知道从哪里返回继续执行，因此信号栈上会保存一个返回地址。
pub struct SignalStack {
    /// Base address of stack
    pub ss_sp: usize,
    /// Flags
    pub ss_flags: i32,
    /// Number of bytes in stack
    pub ss_size: usize,
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

impl SignalStack {
    pub fn get_stack_top(&self) -> usize {
        self.ss_sp + self.ss_size
    }
}

#[derive(Clone, Copy)]
pub struct UContext {
    /// 当前上下文返回时将恢复执行的下一个上下文的指针
    pub uc_link: usize,
    // 当前上下文活跃时被阻塞的信号集
    pub uc_sigmask: SigSet,
    // 当前上下文使用的栈信息,包含栈的基址、大小等信息
    pub uc_stack: SignalStack,
    // 保存具体机器状态的上下文信息，这是一个机器相关的表示，包含了处理器的寄存器状态等信息
    pub uc_mcontext: MContext
}

#[derive(Clone, Copy)]
pub struct MContext {
    pub sepc: usize,
    pub user_x: [usize; 32]
}
