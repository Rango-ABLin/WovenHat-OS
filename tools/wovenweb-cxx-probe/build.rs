use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-env-changed=WOVENWEB_CLANG");
    let compiler = env::var_os("WOVENWEB_CLANG").expect("set WOVENWEB_CLANG to a Clang executable");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    for name in [
        "fixture",
        "dispatch",
        "tls",
        "lifecycle",
        "lifecycle_second",
    ] {
        let source = format!("src/{name}.cpp");
        println!("cargo:rerun-if-changed={source}");
        let object = out.join(format!("{name}.o"));
        let status = Command::new(&compiler)
            .args([
                "--target=x86_64-unknown-none-elf",
                "-x",
                "c++",
                "-std=c++23",
                "-O2",
                "-ffreestanding",
                "-nostdinc++",
                "-fno-exceptions",
                "-fno-rtti",
                "-fno-pic",
                "-fno-pie",
                "-fno-lto",
                "-ftls-model=local-exec",
                "-mcmodel=large",
                "-mno-red-zone",
                "-fno-stack-protector",
                "-fno-asynchronous-unwind-tables",
                "-fno-unwind-tables",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wconversion",
                "-Wsign-conversion",
                "-c",
                &source,
                "-o",
            ])
            .arg(&object)
            .status()
            .expect("execute Clang");
        assert!(status.success(), "C++ fixture compilation failed");
        println!("cargo:rustc-link-arg={}", object.display());
    }
    let c_source = "src/c_abi.c";
    println!("cargo:rerun-if-changed={c_source}");
    let c_object = out.join("c_abi.o");
    let status = Command::new(&compiler)
        .args([
            "--target=x86_64-unknown-none-elf",
            "-x",
            "c",
            "-std=c23",
            "-O2",
            "-ffreestanding",
            "-fno-pic",
            "-fno-pie",
            "-fno-lto",
            "-mcmodel=large",
            "-mno-red-zone",
            "-fno-stack-protector",
            "-fno-asynchronous-unwind-tables",
            "-fno-unwind-tables",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wconversion",
            "-Wsign-conversion",
            "-c",
            c_source,
            "-o",
        ])
        .arg(&c_object)
        .status()
        .expect("execute Clang for the C ABI fixture");
    assert!(status.success(), "C ABI fixture compilation failed");
    println!("cargo:rustc-link-arg={}", c_object.display());
    println!("cargo:rerun-if-changed=src/fixture.hpp");
}
