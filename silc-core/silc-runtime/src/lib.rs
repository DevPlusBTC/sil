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
use std::os::raw::{c_char, c_uchar, c_uint, c_ulonglong, c_void};

// =============================================================================
// FFI crudo (privado, único lugar con `unsafe`)
// =============================================================================

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
    fn sil_sched_spawn(s: *mut c_void, entrada: extern "C" fn(*mut c_void), arg: *mut c_void) -> bool;
    fn sil_sched_ejecutar(s: *mut c_void);
    fn sil_fibra_yield();

    fn sil_cap_validar(cap: *const SilCapacidadFFI, requerido: c_uint, ahora_ns: c_ulonglong) -> bool;
    fn sil_cap_solicitar(recurso: *const c_char, permisos: c_uint, ttl_ns: c_ulonglong) -> SilCapacidadFFI;
}

/// Versión del runtime embebido.
pub const RT_VERSION: &str = env!("CARGO_PKG_VERSION");

// =============================================================================
// Arena (RAII)
// =============================================================================

/// Arena causal O(1). Libera toda la memoria al salir de scope (Drop).
pub struct Arena {
    inner: SilArenaFFI,
}

impl Arena {
    pub fn nueva(capacidad_inicial: usize) -> Self {
        let inner = unsafe { sil_arena_crear(capacidad_inicial) };
        Self { inner }
    }

    /// Asigna `size_of::<T>()` bytes alineados. Retorna puntero mutable.
    /// # Safety
    /// El puntero vive lo que la arena. No usar tras `reset`/`drop`.
    pub fn asignar<T>(&mut self) -> *mut T {
        unsafe {
            sil_arena_asignar(
                &mut self.inner as *mut SilArenaFFI,
                std::mem::size_of::<T>(),
                std::mem::align_of::<T>(),
            ) as *mut T
        }
    }

    /// Asigna slice de `n` elementos. Retorna puntero al inicio.
    pub fn asignar_slice<T>(&mut self, n: usize) -> *mut T {
        unsafe {
            sil_arena_asignar(
                &mut self.inner as *mut SilArenaFFI,
                std::mem::size_of::<T>().checked_mul(n).expect("overflow"),
                std::mem::align_of::<T>(),
            ) as *mut T
        }
    }

    /// Reset O(1): conserva bloques, resetea offsets.
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

// !Send + !Sync por defecto (contiene punteros crudos): correcto para M7.
// Fase 1: arenas thread-local explícitas.

// =============================================================================
// Scheduler + Fibras
// =============================================================================

/// Scheduler cooperativo round-robin (single-thread M7).
pub struct Scheduler {
    ptr: *mut c_void,
}

// El scheduler M7 es single-thread; lo marcamos !Send para disciplina.
impl Scheduler {
    pub fn nuevo() -> Self {
        let ptr = unsafe { sil_sched_crear(1) };
        assert!(!ptr.is_null(), "sil_sched_crear falló (OOM)");
        Self { ptr }
    }

    /// Registra una fibra. `entrada` debe ser `extern "C" fn(*mut c_void)`.
    /// Retorna false si se alcanzó el límite o OOM.
    pub fn spawn(&mut self, entrada: extern "C" fn(*mut c_void), arg: *mut c_void) -> bool {
        unsafe { sil_sched_spawn(self.ptr, entrada, arg) }
    }

    /// Ejecuta hasta que todas las fibras finalicen.
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

/// Cede el control al scheduler (no-op fuera de fibra; sound).
pub fn ceder() {
    unsafe { sil_fibra_yield() };
}

// =============================================================================
// Capacidades
// =============================================================================

/// Permisos (espejo de SilPermiso C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permisos(pub u32);

impl Permisos {
    pub const NINGUNO: Self = Self(0x00);
    pub const LEER_ARCHIVO: Self = Self(0x01);
    pub const ESCRIBIR_ARCHIVO: Self = Self(0x02);
    pub const RED: Self = Self(0x04);
    pub const EXEC: Self = Self(0x08);
}

/// Capacidad Zero-Trust (Copy: token por valor, expiración estricta).
#[derive(Debug, Clone, Copy)]
pub struct Capacidad {
    inner: SilCapacidadFFI,
}

impl Capacidad {
    /// Solicita capacidad de desarrollo (autofirmada, NO SEGURA; Fase 1: TPM).
    pub fn solicitar(recurso: &str, permisos: Permisos, ttl_ns: u64) -> Self {
        let c = CString::new(recurso).unwrap_or_default();
        let inner = unsafe { sil_cap_solicitar(c.as_ptr(), permisos.0, ttl_ns) };
        Self { inner }
    }

    /// Valida contra reloj monotónico actual (ahora_ns=0 → ahora).
    pub fn valida(&self, requerido: Permisos) -> bool {
        unsafe { sil_cap_validar(&self.inner as *const SilCapacidadFFI, requerido.0, 0) }
    }

    /// Valida contra tiempo explícito (para tests deterministas).
    pub fn valida_en(&self, requerido: Permisos, ahora_ns: u64) -> bool {
        unsafe { sil_cap_validar(&self.inner as *const SilCapacidadFFI, requerido.0, ahora_ns) }
    }

    pub fn permisos(&self) -> Permisos {
        Permisos(self.inner.permisos)
    }
}

/// Inicializa el runtime (arenas globales, scheduler). M7: no-op verificado.
pub fn init() {
    // M7: sin estado global. Fase 1: init de pools + eBPF.
}

// =============================================================================
// Tests M7 (ejercitan el C real vía FFI)
// =============================================================================

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
        let p: *mut u32 = a.asignar_slice::<u32>(16);
        assert!(!p.is_null());
        unsafe {
            for i in 0..16 {
                p.add(i).write(i as u32);
            }
            for i in 0..16 {
                assert_eq!(p.add(i).read(), i as u32);
            }
        }
        a.reset();
        // Tras reset, se puede volver a asignar (mismo bloque).
        let q: *mut u32 = a.asignar_slice::<u32>(16);
        assert!(!q.is_null());
    }

    #[test]
    fn arena_crece_automaticamente() {
        let mut a = Arena::nueva(16); // Forzar crecimiento
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
        assert!(s.spawn(fibra_suma, std::ptr::null_mut()));
        assert!(s.spawn(fibra_suma, std::ptr::null_mut()));
        s.ejecutar();
        // Cada fibra: +1 luego +10 = 11. Dos fibras = 22.
        assert_eq!(CONTADOR.load(Ordering::SeqCst), 22);
    }

    #[test]
    fn ceder_fuera_de_scheduler_noop() {
        // Sound: no debe crashear.
        ceder();
    }

    #[test]
    fn capacidad_valida_y_expira() {
        let c = Capacidad::solicitar("test", Permisos::RED, 60_000_000_000);
        assert!(c.valida(Permisos::RED));
        assert!(!c.valida(Permisos::EXEC));
        // Expiración: tiempo futuro más allá del TTL.
        // (Usamos valida_en con tiempo explícito para determinismo.)
        assert!(!c.valida_en(Permisos::RED, u64::MAX));
    }

    #[test]
    fn capacidad_sin_permisos_falla() {
        let c = Capacidad::solicitar("x", Permisos::NINGUNO, 60_000_000_000);
        assert!(!c.valida(Permisos::RED));
    }
}
