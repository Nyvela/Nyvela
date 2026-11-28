use std::{env, path::PathBuf};

fn main() {
    let source = env::var_os("LUMEN_DISK_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Lumfat/src/disk.rs")
        });
    let source = source
        .canonicalize()
        .expect("locate Lumfat's NVMe transport");
    println!("cargo:rerun-if-env-changed=LUMEN_DISK_SOURCE");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rustc-env=LUMEN_DISK_SOURCE={}", source.display());
}
