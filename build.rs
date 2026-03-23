fn main() {
    // Link the TSS SVM library and libm (aarch64 only — precompiled for NXP i.MX93 / cortex-a55).
    // When cross-compiling with --target aarch64-unknown-linux-gnu, libtss_svm.a must be in lib/.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!("cargo:rustc-link-search=native={}/lib", manifest_dir);
        println!("cargo:rustc-link-lib=static=tss_svm");
        println!("cargo:rustc-link-lib=m");
    }
    println!("cargo:rerun-if-changed=lib/libtss_svm.a");
}
