#![no_std]
#![no_main]

extern crate user_lib;

use siginfo::SigInfo;
use user_lib::*;

fn func(signal: usize, info: SigInfo, context: UContext) {
    println!("1 Signal number: {}", signal);
    println!("1 Signal info pointer: {:?}", info);
    println!("1 Signal context pointer: {:?}", context);
    println!("1 yes! user_sig_test passed");
    sigreturn();
}

#[no_mangle]
pub fn main() -> i32 {
    let mut new = SigAction::default();
    let mut old = SigAction::default();
    new.sa_handler = func as usize;
    new.sa_flags = SigActionFlag::SA_SIGINFO;
    println!("sa handler address:{}", new.sa_handler);
    println!("signal_simple: sigaction");
    if sigaction(Sig::SIGUSR1, &new, &mut old) < 0 {
        panic!("Sigaction failed!");
    }
    println!("signal_simple: first kill");
    println!("kill pid: {}", getpid());
    if kill(getpid(), Sig::SIGUSR1) < 0 {
        println!("Kill failed!");
        exit(1);
    }

    fn func2(signal: usize, info: SigInfo, context: UContext) {
        println!("2 this is func2");
        println!("2 Signal number: {}", signal);
        println!("2 Signal info pointer: {:?}", info);
        println!("2 Signal context pointer: {:?}", context);
        println!("2 yes! user_sig_test passed");
        sigreturn();
    }
    println!("signal_simple: second kill");
    new.sa_handler = func2 as usize;

    if sigaction(Sig::SIGUSR2, &new, &mut old) < 0 {
        panic!("Sigaction failed!");
    }
    println!("kill pid: {}", getpid());
    if kill(getpid(), Sig::SIGUSR2) < 0 {
        println!("Kill failed!");
        exit(1);
    }
    println!("signal_simple: Done");
    0
}
