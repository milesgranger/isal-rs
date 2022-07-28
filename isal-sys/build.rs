use std::env;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    // This will not break if not built from source into expected 'isal-sys/install',
    // deferring to the system installed libisal instead.
    let search_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("install/lib");
    println!("cargo:rustc-link-search=native={}", search_dir.display());
    println!("cargo:rustc-link-lib=isal");

    println!("cargo:rerun-if-changed=wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .derive_default(true)
        .dynamic_link_require_all(true)
        .clang_arg("-fPIC")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()?;

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_dir = env::var("OUT_DIR")?;
    let out_path = PathBuf::from(out_dir).join("bindings.rs");
    bindings.write_to_file(out_path)?;

    Ok(())
}
