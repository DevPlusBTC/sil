//! Build script: compila sil-rt/*.c con flags deterministas.

fn main() {
    let mut build = cc::Build::new();
    build
        .files([
            "../sil-rt/src/arena.c",
            "../sil-rt/src/cap.c",
            "../sil-rt/src/channel.c",
            "../sil-rt/src/fiber.c",
            "../sil-rt/src/aio.c",
            "../sil-rt/src/ebpf.c",
            "../sil-rt/src/sil_hmac.c",
        ])
        .include("../sil-rt/include")
        .warnings(false);

    let target = std::env::var("TARGET").unwrap_or_default();
    // io_uring opt-in: `SIL_IO_URING=1` al compilar activa la ruta io_uring
    // (requiere liburing-dev). Sin la variable se usa pread/pwrite.
    if std::env::var("SIL_IO_URING").is_ok() {
        build.define("SIL_IO_URING", None);
        println!("cargo:rerun-if-env-changed=SIL_IO_URING");
    }
    if target.contains("msvc") {
        build.flag("/O2").flag("/W0");
    } else {
        // `_GNU_SOURCE` antes de cualquier header: con -std=c99 (__STRICT_ANSI__)
        // glibc no declara sigset_t / AT_FDCWD / struct addrinfo, y liburing.h
        // (y getaddrinfo/clock_gettime en el resto de sil-rt) fallan al compilar.
        build
            .flag("-D_GNU_SOURCE")
            .flag("-std=c99")
            .flag("-O2")
            .flag("-Wall");
    }

    build.compile("sil_rt");

    println!("cargo:rerun-if-changed=../sil-rt/src/");
    println!("cargo:rerun-if-changed=../sil-rt/include/");
}
