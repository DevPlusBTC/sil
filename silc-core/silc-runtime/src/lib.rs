//! silc-runtime: Bindings seguros al runtime C (sil-rt).
//!
//! M0: esqueleto. FFI completo en M7.

/// Versión del runtime embebido.
pub const RT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Inicializa el runtime (arenas globales, scheduler). M0: no-op.
pub fn init() {
    // TODO(M7): llamar sil_rt_init vía FFI.
}
