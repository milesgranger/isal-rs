## Rust sys and wrapper crates for [isa-l](https://github.com/intel/isa-l)

[![CI](https://github.com/milesgranger/isal-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/milesgranger/isal-rs/actions/workflows/ci.yml)

---

### [isal-sys](./isal-sys/)
  - _sys crate, raw Rust bindings to the isa-l library. [only bindings to lib_gzip]_
### [isal-rs](./isal-rs/)
  - _high level wrapper, including gzip/deflate API. [_WIP_]_

---

### Versioning: 
Versions are specified in normal SemVer format, and a trailing "`+<< commit hash >>`" to indicate
which commit in [isa-l](https://github.com/intel/isa-l) the crate is built against. ie: `0.1.0+62519d9`
