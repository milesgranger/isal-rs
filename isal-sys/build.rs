use std::env;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    // TODO: make lib -f Makefile.unx host_cpu=x86_64-linux-musl CC="$CC -target x86_64-linux-musl" AR="zig ar" LDFLAGS=-static
    // then copy 'bin/isa-l.a' -> 'bin/libisa-l.a'

    let include = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("isa-l/include");
    let out_dir = env::var("OUT_DIR")?;
    let search_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("isa-l/bin");

    println!("cargo:rustc-link-search=native={}", search_dir.display());
    println!("cargo:rustc-link-lib=isal");
    println!("cargo:rustc-link-lib=static=isa-l");

    println!("cargo:rerun-if-changed=wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include.display()))
        .clang_arg(format!("-L{}", search_dir.display()))
        .clang_arg("-lisal")
        .clang_arg("-lisa-l")
        .derive_default(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()?;

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(out_dir).join("bindings.rs");
    bindings.write_to_file(out_path)?;

    Ok(())
}
