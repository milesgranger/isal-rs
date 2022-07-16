#![allow(dead_code)] // TODO
use std::env;
use std::fs;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let _include = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("isa-l/include");
    let lib_name = "isal"; //compile(&include)?;

    let out_dir = env::var("OUT_DIR")?;

    //println!("cargo:rustc-link-search={}", out_dir);
    println!("cargo:rustc-link-lib={}", lib_name);
    println!("cargo:rerun-if-changed=wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        //.clang_arg(format!("-I{}", include.display()))
        .clang_arg("-fPIC")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()?;

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(out_dir).join("bindings.rs");
    bindings.write_to_file(out_path)?;

    Ok(())
}

// Compile into static lib
fn compile(include: &PathBuf) -> Result<&'static str> {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("isa-l/igzip");

    // deps
    let deps = fs::read_dir(&src)
        .unwrap()
        .into_iter()
        .map(|f| f.unwrap())
        .filter(|f| {
            let file_name = f.file_name();
            let name = file_name.to_str().unwrap();
            name.ends_with(".c")
                && !name.contains("perf")
                && !name.contains("test")
                && !name.contains("example")
                && !name.contains("generate")
        })
        .map(|f| f.path());

    cc::Build::new()
        .compiler(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("zcc"))
        .files(deps)
        .include(&include)
        .cpp(false)
        .shared_flag(false)
        .static_flag(true)
        .flag("-g")
        .flag("-s")
        .compile("igzip");

    Ok("igzip")
}
