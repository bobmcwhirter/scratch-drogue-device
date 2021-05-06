//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

fn copy_config(out: &Path, file: &str) {
    if Path::new(file).exists() {
        fs::copy(file, out.join(file)).expect("error copying file");
        println!("cargo:rerun-if-changed={}", file);
    } else {
        println!(
            "Unable to locate config file {}. Creating empty file (your application may not work)",
            file
        );
        // Ok if this doesnt work, it will fail during build
        let _ = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join(file));
    }
}

fn main() {
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // Copy wifi credentials
    fs::create_dir_all(out.join("config")).expect("error creating output directory for config");
    copy_config(&out, "config/wifi.ssid.txt");
    copy_config(&out, "config/wifi.password.txt");

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=memory.x");
}
