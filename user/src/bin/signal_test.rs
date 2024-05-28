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
    let pid = fork();
    if pid == 0 {
        let mut new = SigAction::default();
        let mut old = SigAction::default();
        new.sa_handler = func as usize;

        println!("signal_simple2: child sigaction");
        if sigaction(Sig::SIGUSR1, &new, &mut old) < 0 {
            panic!("Sigaction failed!");
        }
        println!("signal_simple2: child done");
        sleep(1000);
        // yield_();
        exit(0);
    } else if pid > 0 {
        // yield_();
        sleep(500);
        println!("signal_simple2: parent send SIGUSR1 to child");
        if kill(pid as isize, Sig::SIGUSR1) < 0 {
            println!("Kill failed!");
            exit(1);
        }
        println!("signal_simple2: parent wait child");
        let mut exit_code = 0;
        waitpid(pid as usize, &mut exit_code);
        println!("signal_simple2: parent Done");
        exit(0);
    }

    0
}
