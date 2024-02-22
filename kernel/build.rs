use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    let link_script = fs::read_to_string(PathBuf::from(manifest_dir).join("linker.ld")).unwrap();

    let ram_size = config::mm::RAM_SIZE - config::mm::KERNEL_OFFSET;

    let new = link_script
        .replace("%RAM_START%", &config::mm::KERNEL_START_PHYS.to_string())
        .replace("%VIRT_START%", &config::mm::KERNEL_START.to_string())
        .replace("%RAM_SIZE%", &ram_size.to_string());

    let dest = PathBuf::from(out_dir).join("linker.ld");
    fs::write(&dest, new).unwrap();
    println!("cargo:rustc-link-arg=-T{}", dest.display());
}
