use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {

    // Build isa-l
    build_isal()?;

    // This will not break if not built from source into expected 'isal-sys/install',
    // deferring to the system installed libisal instead.
    let search_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("install/lib");
    println!("cargo:rustc-link-search=native={}", search_dir.display());
    println!("cargo:rustc-link-lib=static=isal");

    println!("cargo:rerun-if-changed=wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .derive_default(true)
        .dynamic_link_require_all(true)
        .clang_arg("-fPIC")
        .generate_inline_functions(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()?;

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_dir = env::var("OUT_DIR")?;
    let out_path = PathBuf::from(out_dir).join("bindings.rs");
    bindings.write_to_file(out_path)?;

    Ok(())
}


fn build_isal() -> Result<()> {
    std::env::set_current_dir("./isa-l")?;
    clean()?;
    compile()?;
    std::env::set_current_dir("./..")?;
    Ok(())
}

fn compile() -> Result<()> {
    let output = Command::new("make")
        .args(["install", "prefix=./../install", "-f", "Makefile.unx"]) //, r#"CC="zig cc""#, r#"AR="zig ar""#])
        .output()?;
    if !output.status.success() {
        panic!("Failed to install isa-l: {}", String::from_utf8(output.stderr)?);
    }
    Ok(())
}

fn clean() -> Result<()> {
    let output = Command::new("make")
        .args(["clean", "-f", "Makefile.unx"])
        .output()?;
    if !output.status.success() {
        panic!("Failed to clean isa-l: {}", String::from_utf8(output.stderr)?);
    }
    Ok(())
}