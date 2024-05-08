use std::{
    env, fs,
    fs::{read_dir, File},
    io::{Result, Write},
    path::PathBuf,
};

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

    insert_app_data().unwrap();
}

fn insert_app_data() -> Result<()> {
    let mut target_path: String = String::from("./target/riscv64gc-unknown-none-elf/");
    let mode = option_env!("MODE").unwrap_or("debug");
    target_path.push_str(mode);
    target_path.push('/');
    let mut f = File::create("src/link_app.asm").unwrap();
    let mut apps: Vec<_> = read_dir("../user/src/bin")
        .unwrap()
        .map(|dir_entry| {
            let mut name_with_ext = dir_entry.unwrap().file_name().into_string().unwrap();
            name_with_ext.drain(name_with_ext.find('.').unwrap()..name_with_ext.len());
            name_with_ext
        })
        .filter(|name| name == "preliminary_tests")
        .collect();

    apps.sort();

    // let testcases = [
    //     // "true",
    //     // "time-test",
    //     // "busybox_testcode.sh",
    //     // "busybox_cmd.txt",
    //     // "busybox",
    //     // "lmbench_all",
    //     // "lmbench_testcode.sh",
    //     // "runtest.exe",
    //     // "entry-static.exe",
    //     // "run-static.sh",
    // ];
    //
    // for testcase in testcases {
    //     apps.push(testcase.to_string());
    // }

    writeln!(
        f,
        r#"
    .align 3
    .section .data
    .global _num_app
_num_app:
    .quad {}"#,
        apps.len()
    )?;

    for i in 0..apps.len() {
        writeln!(f, r#"    .quad app_{}_start"#, i)?;
    }
    writeln!(f, r#"    .quad app_{}_end"#, apps.len() - 1)?;

    writeln!(
        f,
        r#"
    .global _app_names
_app_names:"#
    )?;
    for app in apps.iter() {
        writeln!(f, r#"    .string "{}""#, app)?;
    }

    for (idx, app) in apps.iter().enumerate() {
        println!("app_{}: {}", idx, app);
        writeln!(
            f,
            r#"
    .section .data
    .global app_{0}_start
    .global app_{0}_end
    .align 3
app_{0}_start:
    .incbin "{2}{1}"
app_{0}_end:"#,
            idx, app, target_path
        )?;
    }
    Ok(())
}
