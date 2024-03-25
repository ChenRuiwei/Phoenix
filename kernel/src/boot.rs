use driver::println;

const BOOT_MSG: &str = r#"
===============================================================
  __________  ________ _______  __  _____________   ___  ______
 / ___/ __/ |/ / __/ // /  _/ |/ / / __/_  __/ _ | / _ \/_  __/
/ (_ / _//    /\ \/ _  // //    / _\ \  / / / __ |/ , _/ / /
\___/___/_/|_/___/_//_/___/_/|_/ /___/ /_/ /_/ |_/_/|_| /_/
===============================================================
"#;

pub fn print_boot_message() {
    println!("{}", BOOT_MSG);
}

/// Clear BSS segment at start up
pub fn clear_bss() {
    extern "C" {
        fn _sbss();
        fn _ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(_sbss as usize as *mut u8, _ebss as usize - _sbss as usize)
            .fill(0);
    }
}
