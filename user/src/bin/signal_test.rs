#![no_std]
#![no_main]

use signal::sigset::Sig;
use user_lib::{exit, getpid, println, sigaction, sigreturn, types::SigAction};

extern crate user_lib;

fn func() {
    println!("user_sig_test passed");
    sigreturn();
}

#[no_mangle]
pub fn main() -> i32 {
    // let mut new = SigAction::default();
    // let mut old = SigAction::default();
    // new.sa_handler = func as usize;

    // println!("signal_simple: sigaction");
    // if sigaction(Sig::SIGUSR1, &new, &mut old) < 0 {
    //     panic!("Sigaction failed!");
    // }
    // println!("signal_simple: kill");
    // if kill(getpid() as usize, Sig::SIGUSR1.index()) < 0 {
    //     println!("Kill failed!");
    //     exit(1);
    // }
    println!("signal_simple: Done");
    0
}
