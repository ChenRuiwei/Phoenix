use config::{mm::HART_START_ADDR, processor::HART_NUM};
use driver::{println, sbi};

const BOOT_BANNER: &str = r#"
    ____  __                     _
   / __ \/ /_  ____  ___  ____  (_)  __
  / /_/ / __ \/ __ \/ _ \/ __ \/ / |/_/
 / ____/ / / / /_/ /  __/ / / / />  <
/_/   /_/ /_/\____/\___/_/ /_/_/_/|_|
"#;

pub fn print_banner() {
    println!("{}", BOOT_BANNER);
}

/// Clear BSS segment at start up.
pub fn clear_bss() {
    extern "C" {
        fn _sbss();
        fn _ebss();
    }
    unsafe {
        let start = _sbss as usize as *mut u64;
        let end = _ebss as usize as *mut u64;
        let len = end.offset_from(start) as usize;
        core::slice::from_raw_parts_mut(start, len).fill(0);

        // Handle any remaining bytes if the length is not a multiple of u64
        let start_byte = start as *mut u8;
        let len_bytes = _ebss as usize - _sbss as usize;
        if len_bytes % 8 != 0 {
            let offset = len * 8;
            core::slice::from_raw_parts_mut(start_byte.add(offset), len_bytes - offset).fill(0);
        }
    }
}

pub fn start_harts(hart_id: usize) {
    for i in 0..HART_NUM {
        if i == hart_id {
            continue;
        }
        let status: isize = sbi::hart_start(i, HART_START_ADDR) as _;
        println!("[kernel] start to wake up hart {i}... status {status}");
    }
}
