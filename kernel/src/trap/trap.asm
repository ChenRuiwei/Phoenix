.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text
    .globl __trap_from_user
    .globl __return_to_user
    .globl __trap_from_kernel
    .globl __user_rw_trap_vector
    .globl __user_rw_exception_entry
    .globl __try_read_user
    .globl __try_write_user
    .align 2


# __trap_from_user: This label marks the entry point for traps originating from user mode
__trap_from_user:
    # Swap the user stack pointer (sscratch) with the kernel stack pointer (sp)
    csrrw sp, sscratch, sp
    # Now, sp points to *TrapContext in kernel space, sscratch holds the user stack pointer
    
    # Save other general-purpose registers
    sd x1, 1*8(sp)
    # Skip sp (x2), it will be saved later
    
    # Save x3~x31 (x4 is tp, thread pointer, hence it's skipped)
    .set n, 3
    .rept 29
        SAVE_GP %n
        .set n, n+1
    .endr

    # t0, t1, t2 registers are temporary and can be used freely here since they've been saved in TrapContext
    csrr t0, sstatus    # Read the supervisor status register into t0
    csrr t1, sepc       # Read the exception program counter into t1
    sd t0, 32*8(sp)     # Save sstatus into the TrapContext
    sd t1, 33*8(sp)     # Save sepc into the TrapContext

    # Read user stack pointer from sscratch and save it into the TrapContext
    csrr t2, sscratch   # Read the user stack pointer into t2
    sd t2, 2*8(sp)      # Save user stack pointer into the TrapContext

    # Move to kernel stack pointer (kernel_sp)
    # Load the kernel return address
    ld ra, 35*8(sp)     # Load the return address from the TrapContext
    # Load callee-saved registers (s0-s11)
    ld s0, 36*8(sp)
    ld s1, 37*8(sp)
    ld s2, 38*8(sp)
    ld s3, 39*8(sp)
    ld s4, 40*8(sp)
    ld s5, 41*8(sp)
    ld s6, 42*8(sp)
    ld s7, 43*8(sp)
    ld s8, 44*8(sp)
    ld s9, 45*8(sp)
    ld s10, 46*8(sp)
    ld s11, 47*8(sp)

    # Load kernel frame pointer (fp) and thread pointer (tp)
    ld fp, 48*8(sp)
    ld tp, 49*8(sp)

    # Finally, load the kernel stack pointer from the TrapContext
    ld sp, 34*8(sp)
    
    # Return to the kernel return address (ra)
    ret

# __return_to_user: This label marks the entry point for returning from kernel mode to user mode.
# a0: Pointer to TrapContext in user space (constant)
__return_to_user:
    # Set sscratch to store the TrapContext's address
    csrw sscratch, a0

    # Save kernel callee-saved registers into TrapContext
    sd sp, 34*8(a0)    # Save stack pointer
    sd ra, 35*8(a0)    # Save return address
    sd s0, 36*8(a0)    # Save callee-saved register s0
    sd s1, 37*8(a0)    # Save callee-saved register s1
    sd s2, 38*8(a0)    # Save callee-saved register s2
    sd s3, 39*8(a0)    # Save callee-saved register s3
    sd s4, 40*8(a0)    # Save callee-saved register s4
    sd s5, 41*8(a0)    # Save callee-saved register s5
    sd s6, 42*8(a0)    # Save callee-saved register s6
    sd s7, 43*8(a0)    # Save callee-saved register s7
    sd s8, 44*8(a0)    # Save callee-saved register s8
    sd s9, 45*8(a0)    # Save callee-saved register s9
    sd s10, 46*8(a0)   # Save callee-saved register s10
    sd s11, 47*8(a0)   # Save callee-saved register s11
    sd fp, 48*8(a0)    # Save frame pointer
    sd tp, 49*8(a0)    # Save thread pointer

    # Move sp to point to TrapContext in kernel space
    mv sp, a0
    # now sp points to TrapContext in kernel space, start restoring based on it
    # restore sstatus/sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # restore general purpose registers except x0/sp/
    ld x1, 1*8(sp)
    .set n, 3
    .rept 29
        LOAD_GP %n
        .set n, n+1
    .endr
    # back to user stack
    ld sp, 2*8(sp)
    sret

# kernel -> kernel
__trap_from_kernel:
    # only need to save caller-saved regs
    # note that we don't save sepc & stvec here
    addi sp, sp, -17*8
    sd  ra,  1*8(sp)
    sd  t0,  2*8(sp)
    sd  t1,  3*8(sp)
    sd  t2,  4*8(sp)
    sd  t3,  5*8(sp)
    sd  t4,  6*8(sp)
    sd  t5,  7*8(sp)
    sd  t6,  8*8(sp)
    sd  a0,  9*8(sp)
    sd  a1, 10*8(sp)
    sd  a2, 11*8(sp)
    sd  a3, 12*8(sp)
    sd  a4, 13*8(sp)
    sd  a5, 14*8(sp)
    sd  a6, 15*8(sp)
    sd  a7, 16*8(sp)
    call kernel_trap_handler
    ld  ra,  1*8(sp)
    ld  t0,  2*8(sp)
    ld  t1,  3*8(sp)
    ld  t2,  4*8(sp)
    ld  t3,  5*8(sp)
    ld  t4,  6*8(sp)
    ld  t5,  7*8(sp)
    ld  t6,  8*8(sp)
    ld  a0,  9*8(sp)
    ld  a1, 10*8(sp)
    ld  a2, 11*8(sp)
    ld  a3, 12*8(sp)
    ld  a4, 13*8(sp)
    ld  a5, 14*8(sp)
    ld  a6, 15*8(sp)
    ld  a7, 16*8(sp)
    addi sp, sp, 17*8
    sret

# arg: (user_ptr)
# return: (usize, usize)
# if a0 == 0, which means no exception happens
# if a0 == 1, which means exception happens, then we will treat a1 as scause
#
# Safety: need to set stvec to __user_rw_trap_vector and vector mode first
__try_read_user:
    mv a1, a0
    mv a0, zero
    # will trap into __user_rw_trap_vector if exception happens
    # we don't care what value this lb will read
    lb a1, 0(a1)
    ret

__try_write_user:
    mv a2, a0
    mv a0, zero
    lb a1, 0(a2)
    sb a1, 0(a2)
    ret

__user_rw_exception_entry:
    csrr a0, sepc
    addi a0, a0, 4
    csrw sepc, a0
    li   a0, 1
    csrr a1, scause
    sret

    .align 8
__user_rw_trap_vector:
    j __user_rw_exception_entry
    .rept 16
    .align 2
    j __trap_from_kernel
    .endr
    unimp

