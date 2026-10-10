//! Build script: compila sil-rt/*.c con flags deterministas.

fn main() {
    let mut build = cc::Build::new();
    build
        .files([
            "../sil-rt/src/arena.c",
            "../sil-rt/src/cap.c",
        ])
        .include("../sil-rt/include")
        .warnings(false);

    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("msvc") {
        build.flag("/O2").flag("/W0");
    } else {
        build.flag("-std=c99").flag("-O2").flag("-Wall");
    }

    build.compile("sil_rt");

    println!("cargo:rerun-if-changed=../sil-rt/src/");
    println!("cargo:rerun-if-changed=../sil-rt/include/");
}