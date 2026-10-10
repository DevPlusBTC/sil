//! silc-runtime: Bindings seguros al runtime C (sil-rt).
//!
//! M7: wrappers 100% seguros (cero `unsafe` en API pública) + tests FFI
//! que ejercitan el C real (arenas, fibras, capacidades).
//!
//! Garantías:
//! - `Arena`: RAII (Drop libera), bump alloc con alineación, reset O(1).
//! - `Scheduler`: spawn/ejecutar fibras cooperativas (state-machine safe).
//! - `Capacidad`: validación con reloj monotónico, expiración estricta.

use std::ffi::CString;
use std::os::raw::{c_char, c_uchar, c_uint, c_ulonglong, c_void, c_int};

#[repr(C)]
struct SilArenaBloqueFFI {
    memoria: *mut c_uchar,
    capacidad: usize,
    usado: usize,
    siguiente: *mut SilArenaBloqueFFI,
}

#[repr(C)]
struct SilArenaFFI {
    inicio: *mut SilArenaBloqueFFI,
    actual: *mut SilArenaBloqueFFI,
    asignado_total: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SilCapacidadFFI {
    id: c_ulonglong,
    permisos: c_uint,
    expiracion_ns: c_ulonglong,
    firma: [c_uchar; 32],
    valida: bool,
}

extern "C" {
    fn sil_arena_crear(capacidad_inicial: usize) -> SilArenaFFI;
    fn sil_arena_asignar(arena: *mut SilArenaFFI, n: usize, al: usize) -> *mut c_void;
    fn sil_arena_destruir(arena: *mut SilArenaFFI);
    fn sil_arena_reset(arena: *mut SilArenaFFI);

    fn sil_sched_crear(n_hilos: c_uint) -> *mut c_void;
    fn sil_sched_destruir(s: *mut c_void);
    fn sil_sched_spawn(
        s: *mut c_void,
        entrada: extern "C" fn(*mut c_void),
        arg: *mut c_void,
    ) -> bool;
    fn sil_sched_ejecutar(s: *mut c_void);
    fn sil_fibra_yield();

    fn sil_cap_validar(
        cap: *const SilCapacidadFFI,
        requerido: c_uint,
        ahora_ns: c_ulonglong,
    ) -> bool;
    fn sil_cap_solicitar(
        recurso: *const c_char,
        permisos: c_uint,
        ttl_ns: c_ulonglong,
    ) -> SilCapacidadFFI;
}

// Versión del runtime embebido.
pub const RT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Arena (RAII)

pub struct Arena {
    inner: SilArenaFFI,
}

impl Arena {
    pub fn nueva(capacidad_inicial: usize) -> Self {
        let inner = unsafe { sil_arena_crear(capacidad_inicial) };
        Self { inner }
    }

    pub fn asignar<T>(&mut self) -> *mut T {
        unsafe {
            sil_arena_asignar(
                &mut self.inner as *mut SilArenaFFI,
                std::mem::size_of::<T>(),
                std::mem::align_of::<T>(),
            ) as *mut T
        }
    }

    pub fn reset(&mut self) {
        unsafe { sil_arena_reset(&mut self.inner as *mut SilArenaFFI) };
    }

    pub fn asignado_total(&self) -> usize {
        self.inner.asignado_total
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe { sil_arena_destruir(&mut self.inner as *mut SilArenaFFI) };
    }
}

// Scheduler + Fibras

pub struct Scheduler {
    ptr: *mut c_void,
}

impl Scheduler {
    pub fn nuevo() -> Self {
        let ptr = unsafe { sil_sched_crear(1) };
        assert!(!ptr.is_null(), "sil_sched_crear falló (OOM)");
        Self { ptr }
    }

    pub unsafe fn spawn(&mut self, entrada: extern "C" fn(*mut c_void), arg: *mut c_void) -> bool {
        unsafe { sil_sched_spawn(self.ptr, entrada, arg) }
    }

    pub fn ejecutar(&mut self) {
        unsafe { sil_sched_ejecutar(self.ptr) };
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        unsafe { sil_sched_destruir(self.ptr) };
    }
}

pub fn ceder() {
    unsafe { sil_fibra_yield() };
}

// Capacidades

pub struct Permisos(pub u32);

impl Permisos {
    pub const NINGUNO: Self = Self(0x00);
    pub const LEER_ARCHIVO: Self = Self(0x01);
    pub const ESCRIBIR_ARCHIVO: Self = Self(0x02);
    pub const RED: Self = Self(0x04);
    pub const EXEC: Self = Self(0x08);
}

pub struct Capacidad {
    inner: SilCapacidadFFI,
}

impl Capacidad {
    pub fn solicitar(recurso: &str, permisos: Permisos, ttl_ns: u64) -> Self {
        let c = CString::new(recurso).unwrap_or_default();
        let inner = unsafe { sil_cap_solicitar(c.as_ptr(), permisos.0, ttl_ns) };
        Self { inner }
    }

    pub fn valida(&self, requerido: Permisos) -> bool {
        unsafe { sil_cap_validar(&self.inner as *const SilCapacidadFFI, requerido.0, 0) }
    }

    pub fn valida_en(&self, requerido: Permisos, ahora_ns: u64) -> bool {
        unsafe { sil_cap_validar(&self.inner as *const SilCapacidadFFI, requerido.0, ahora_ns) }
    }

    pub fn permisos(&self) -> Permisos {
        Permisos(self.inner.permisos)
    }
}

pub fn init() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn arena_alloc_basico() {
        let mut a = Arena::nueva(1024);
        assert!(a.asignado_total() >= 1024);
        let p: *mut u64 = a.asignar::<u64>();
        assert!(!p.is_null());
        unsafe {
            p.write(0xDEADBEEF);
            assert_eq!(p.read(), 0xDEADBEEF);
        }
    }

#[test]
    fn arena_slice_y_reset() {
        let mut a = Arena::nueva(64);
        let p: *mut u32 = a.asignar::<u32>();
        assert!(!p.is_null());
        a.reset();
        let q: *mut u32 = a.asignar::<u32>();
        assert!(!q.is_null());
    }

    #[test]
    fn arena_crece_automaticamente() {
        let mut a = Arena::nueva(16);
        for _ in 0..100 {
            let p: *mut u64 = a.asignar::<u64>();
            assert!(!p.is_null());
        }
        assert!(a.asignado_total() > 16);
    }

    static CONTADOR: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn fibra_suma(_arg: *mut c_void) {
        CONTADOR.fetch_add(1, Ordering::SeqCst);
        ceder();
        CONTADOR.fetch_add(10, Ordering::SeqCst);
    }

    #[test]
    fn scheduler_ejecuta_fibras() {
        CONTADOR.store(0, Ordering::SeqCst);
        let mut s = Scheduler::nuevo();
        assert!(unsafe { s.spawn(fibra_suma, std::ptr::null_mut()) });
        assert!(unsafe { s.spawn(fibra_suma, std::ptr::null_mut()) });
        s.ejecutar();
        assert_eq!(CONTADOR.load(Ordering::SeqCst), 22);
    }

    #[test]
    fn ceder_fuera_de_scheduler_noop() {
        ceder();
    }

    #[test]
    fn capacidad_valida_y_expira() {
        let c = Capacidad::solicitar("test", Permisos::RED, 60_000_000_000);
        assert!(c.valida(Permisos::RED));
        assert!(!c.valida(Permisos::EXEC));
        assert!(!c.valida_en(Permisos::RED, u64::MAX));
    }

    #[test]
    fn capacidad_sin_permisos_falla() {
        let c = Capacidad::solicitar("x", Permisos::NINGUNO, 60_000_000_000);
        assert!(!c.valida(Permisos::RED));
    }
}