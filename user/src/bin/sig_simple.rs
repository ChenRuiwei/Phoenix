#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::*;

fn func() {
    println!("user_sig_test passed");
    sigreturn();
}

#[no_mangle]
pub fn main() -> i32 {
    let mut new = SigAction::default();
    let mut old = SigAction::default();
    new.sa_handler = func as usize;
    println!("sa handler address:{}", new.sa_handler);
    println!("signal_simple: sigaction");
    if sigaction(Sig::SIGUSR1, &new, &mut old) < 0 {
        panic!("Sigaction failed!");
    }
    println!("signal_simple: kill");
    println!("kill pid: {}", getpid());
    if kill(getpid(), Sig::SIGUSR1) < 0 {
        println!("Kill failed!");
        exit(1);
    }
    println!("signal_simple: Done");
    0
}
