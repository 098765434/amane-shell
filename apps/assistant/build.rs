//! Links Cactus into the daemon when it's given: `CACTUS_LIB_DIR` holds
//! `libcactus.a` (Cactus's prebuilt release), `LIBCXX_DIR` the LLVM libc++
//! it was built against (`libc++.a`, `libc++abi.a`), both linked statically
//! so the daemon needs nothing more at run time. Without them the daemon
//! builds anyway and answers that it has no engine, so the workspace builds
//! on any machine. Only the binaries link it: the shell, which takes the
//! library, never does.

use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(cactus)");
    println!("cargo::rerun-if-env-changed=CACTUS_LIB_DIR");
    println!("cargo::rerun-if-env-changed=LIBCXX_DIR");

    let Some(cactus) = std::env::var_os("CACTUS_LIB_DIR").map(PathBuf::from) else {
        return;
    };
    let archive = cactus.join("libcactus.a");
    if !archive.exists() {
        println!(
            "cargo::warning=CACTUS_LIB_DIR has no libcactus.a ({}); building without Cactus",
            cactus.display()
        );
        return;
    }
    println!("cargo::rerun-if-changed={}", archive.display());
    println!("cargo::rustc-cfg=cactus");
    println!("cargo::rustc-link-arg-bins={}", archive.display());
    match std::env::var_os("LIBCXX_DIR").map(PathBuf::from) {
        Some(cxx) => {
            println!(
                "cargo::rustc-link-arg-bins={}",
                cxx.join("libc++.a").display()
            );
            println!(
                "cargo::rustc-link-arg-bins={}",
                cxx.join("libc++abi.a").display()
            );
        }
        None => println!("cargo::rustc-link-arg-bins=-lc++"),
    }
    println!("cargo::rustc-link-arg-bins=-lpthread");
    println!("cargo::rustc-link-arg-bins=-lm");
}
