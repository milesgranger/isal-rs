#[cfg(feature = "regenerate-bindings")]
use std::path::PathBuf;
use std::{
    io::{self, Write},
    path::Path,
    process::{Command, Stdio},
};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    {
        let out_dir_str = std::env::var("OUT_DIR").unwrap();
        let out_dir = Path::new(&out_dir_str);

        let src_dir = out_dir.join("isa-l");
        if !src_dir.exists() {
            copy_dir::copy_dir("isa-l", &src_dir).unwrap();
        }

        let install_path_str =
            std::env::var("ISAL_INSTALL_PREFIX").unwrap_or(out_dir_str.to_owned());
        let install_path = Path::new(&install_path_str).join("isa-l");

        let current_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&install_path).unwrap();

        // TODO: support 'nmake' for windows and aarch64 target
        let cmd = Command::new("make")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args([
                "install",
                &format!("prefix={}", install_path.display()),
                "-f",
                "Makefile.unx",
                &format!("CFLAGS_=-fPIC -O3"),
            ])
            .spawn();
        std::env::set_current_dir(&current_dir).unwrap();

        let output = cmd.unwrap().wait_with_output().unwrap();
        io::stdout().write_all(&output.stdout).unwrap();
        io::stderr().write_all(&output.stderr).unwrap();
        if !output.status.success() {
            panic!("Building isa-l failed");
        }

        // Solves undefined reference to __cpu_model when using __builtin_cpu_supports() in shuffle.c
        if let Ok(true) = std::env::var("CARGO_CFG_TARGET_ENV").map(|v| v == "musl") {
            println!("cargo:rustc-link-lib=gcc");
        }

        for subdir in ["bin", "lib", "lib64"] {
            let search_path = install_path.join(subdir);
            println!("cargo:rustc-link-search=native={}", search_path.display());
        }
    }

    #[allow(unused_variables)]
    let libname = if cfg!(target_os = "windows") {
        "libisal"
    } else {
        "isal"
    };

    #[cfg(feature = "static")]
    println!("cargo:rustc-link-lib=static={}", libname);

    #[cfg(feature = "shared")]
    println!("cargo:rustc-link-lib=isal");

    #[cfg(feature = "regenerate-bindings")]
    {
        let out = PathBuf::from(&(format!("{}/igzip_lib.rs", std::env::var("OUT_DIR").unwrap())));
        bindgen::Builder::default()
            // The input header we would like to generate
            // bindings for.
            .header("isa-l/include/igzip_lib.h")
            // Tell cargo to invalidate the built crate whenever any of the
            // included header files changed.
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .blocklist_type("__uint64_t_")
            .blocklist_type("__size_t")
            // .blocklist_item("BLOSC2_[C|D]PARAMS_DEFAULTS")
            // .allowlist_type(".*ISAL.*")
            // .allowlist_type(".*isal.*")
            // .allowlist_function(".*isal.*")
            // .allowlist_var(".*ISAL.*")
            // Replaced by libc::FILE
            .blocklist_type("FILE")
            .blocklist_type("_IO_FILE")
            .blocklist_type("_IO_codecvt")
            .blocklist_type("_IO_wide_data")
            .blocklist_type("_IO_marker")
            .blocklist_type("_IO_lock_t")
            // Replaced by i64
            .blocklist_type("LARGE_INTEGER")
            // Replaced by libc::timespec
            .blocklist_type("timespec")
            // etc
            .blocklist_type("__time_t")
            .blocklist_type("__syscall_slong_t")
            // .blocklist_type("__off64_t")
            .blocklist_type("__off_t")
            .size_t_is_usize(true)
            // .no_default("_[c|d]params")
            .generate()
            .expect("Unable to generate bindings")
            .write_to_file(out)
            .unwrap();
    }
}
