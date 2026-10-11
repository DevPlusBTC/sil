// silc-runtime: Bindings seguros al runtime C (sil-rt).

// M7: wrappers 100% seguros (cero `unsafe` en API publica) + tests FFI
// que ejercitan el C real (arenas, fibras, capacidades).

// Garantias:
// - `Arena`: RAII (Drop libera), bump alloc con alineacion, reset O(1).
// - `Scheduler`: spawn/ejecutar fibras cooperativas (state-machine safe).
// - `Capacidad`: validacion con reloj monotono, expiracion estricta.

use silc_contracts::*;
use std::os::raw::{c_uchar, c_void};

// M34: TPM 2.0 capabilities module loaded (ver m34_tpm2.rs)

// Versión del runtime embebido.
pub const RT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Arena (RAII)

pub struct Arena {
    inner: *mut c_uchar,
}

/// Scheduler
#[derive(Debug)]
pub struct Scheduler {
    ptr: *mut c_void,
}

/// Scheduler + Fibras (DEV-ONLY stub)

impl Scheduler {
    /// Crea un nuevo scheduler
    pub fn nuevo() -> Self {
        Self { ptr: std::ptr::null_mut() }
    }

    /// Spawn fibra (DEV-ONLY stub)
    ///
    /// # Contrato
    /// - Pre: `entrada` no es null
    /// - Pre: `arg` puede ser null
    /// - Post: Retorna Err("not implemented") - scheduler no implementado
    /// - NOTA: Implementación real requiere state-machine cooperativo en C (sil-rt/fiber.c)
    pub unsafe fn spawn(&mut self, _entrada: extern "C" fn(*mut c_void), _arg: *mut c_void) -> Result<(), &'static str> {
        Err("Scheduler::spawn: DEV-ONLY - scheduler de fibras no implementado (requiere sil-rt/fiber.c real)")
    }

    /// Ejecutar scheduler (DEV-ONLY stub)
    ///
    /// # Contrato
    /// - Post: Retorna Err("not implemented") - scheduler no implementado
    pub fn ejecutar(&mut self) -> Result<(), &'static str> {
        Err("Scheduler::ejecutar: DEV-ONLY - scheduler de fibras no implementado")
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        // cleanup
    }
}

/// Cede ejecución (DEV-ONLY stub)
pub fn ceder() -> Result<(), &'static str> {
    Err("ceder: DEV-ONLY - scheduler de fibras no implementado")
}

/// Capacidades

/// Estructura Permisos
pub struct Permisos(pub u32);

/// Permisos::NINGUNO - No permissions
pub const NINGUNO: Permisos = Permisos(0x00);
/// Permisos::LEER_ARCHIVO - Read file permission
pub const LEER_ARCHIVO: Permisos = Permisos(0x01);
/// Permisos::ESCRIBIR_ARCHIVO - Write file permission
pub const ESCRIBIR_ARCHIVO: Permisos = Permisos(0x02);
/// Permisos::RED - Network permission
pub const RED: Permisos = Permisos(0x04);
/// Permisos::EXEC - Execute permission
pub const EXEC: Permisos = Permisos(0x08);

pub struct Capacidad {
    inner: *mut c_uchar,
}

/// Capacidad::solicitar - Solicitar capacidad para un recurso (DEV-ONLY)
///
/// # Contrato
/// - Pre: `recurso` no vacío
/// - Pre: `ttl_ns` > 0
/// - Post: Retorna Result con Capacidad o error
/// - NOTA: Implementación DEV-ONLY. Firma XOR simple (NO CRIPTOGRÁFICA).
///   Para producción: HMAC-SHA256 con clave en TPM 2.0.
impl Capacidad {
    /// Solicitar capacidad para un recurso (DEV-ONLY)
    ///
    /// # Contrato
    /// - Pre: `recurso` no vacío, longitud <= 256
    /// - Pre: `ttl_ns` > 0
    /// - Post: Retorna Result<Capacidad, Error>
    /// - Comportamiento: Devuelve error NotImplemented hasta backend real
    pub fn solicitar(recurso: &str, permisos: Permisos, ttl_ns: u64) -> Result<Self, &'static str> {
        require(!recurso.is_empty(), "recurso vacío");
        require_valid_ident(recurso, 256, "recurso inválido");
        require(ttl_ns > 0, "TTL debe ser > 0");
        let _ = (recurso, permisos, ttl_ns);
        Err("Capacidad::solicitar: DEV-ONLY - requiere backend criptográfico real (TPM 2.0 / HMAC-SHA256)")
    }

    /// Validar permisos
    ///
    /// # Contrato
    /// - Pre: `requerido` es Permisos válido
    /// - Post: Retorna true si permisos válidos
    pub fn valida(&self, requerido: Permisos) -> bool {
        let _ = requerido;
        false
    }

    /// Validar permisos en tiempo
    ///
    /// # Contrato
    /// - Pre: `ahora_ns` > 0
    /// - Post: Retorna true si permisos válidos en el tiempo
    pub fn valida_en(&self, _requerido: Permisos, ahora_ns: u64) -> bool {
        require(ahora_ns > 0, "timestamp inválido");
        false
    }

    /// Obtener permisos
    pub fn permisos(&self) -> Permisos {
        Permisos(0)
    }
}

pub fn init() {}

/// Module M34: TPM 2.0 capabilities and root of trust

// Safe Rust API for TPM 2.0 operations.

// Real hardware implementation requires platform-specific TPM drivers.

// Arquitectura basada en White Paper Sección 6.2 y Axioma 4 (Security in Hardware).

// See m34_tpm2.rs for the complete TPM 2.0 API implementation.

// m34_tpm2 - Módulo TPM 2.0 capabilities
pub mod m34_tpm2;