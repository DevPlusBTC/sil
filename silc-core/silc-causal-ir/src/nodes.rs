//! Nodos Causal-IR: cuádruplo ortogonal ⟨ID_SSA, Operación, Capacidad, Invariante⟩.
//!
//! Corresponde a Whitepaper §7.2.

use slotmap::new_key_type;

new_key_type! {
    /// ID estable para valores SSA.
    pub struct ValueId;
    /// ID estable para bloques básicos.
    pub struct BlockId;
    /// ID estable para arenas.
    pub struct ArenaId;
}

/// Tipo SIL (subset M0, completo en M3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SilType {
    Entero64,
    Booleano,
    Texto,
    Void,
}

/// Permiso de capacidad (Zero-Trust).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permiso {
    LecturaArchivo,
    EscrituraArchivo,
    Red,
    Ejecucion,
}

/// Requerimiento de capacidad para una operación de E/S.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapacidadReq {
    pub permiso: Permiso,
    pub ttl_ns: u64,
    pub recurso: Option<String>,
}

/// Clase de invariante SMT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseInvariante {
    Asuncion,
    Demostracion,
}

/// Fórmula lógica (para traducción SMT).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FormulaLogica {
    Var(String),
    ConstInt(i64),
    ConstBool(bool),
    BinOp {
        op: OpLogico,
        lhs: Box<FormulaLogica>,
        rhs: Box<FormulaLogica>,
    },
}

/// Operador lógico/aritmético.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpLogico {
    Gt, Lt, Eq, Add, Sub, Mul,
}

/// Invariante SMT adjunta a una instrucción.
#[derive(Debug, Clone)]
pub struct InvarianteSMT {
    pub formula: FormulaLogica,
    pub hash: [u8; 64],
    pub clase: ClaseInvariante,
}

/// Instrucción causal.
#[derive(Debug, Clone)]
pub struct InstrCausal {
    pub op: Operacion,
    pub capacidad: Option<CapacidadReq>,
    pub invariante: Option<InvarianteSMT>,
}

/// Operación IR.
#[derive(Debug, Clone)]
pub enum Operacion {
    Param { nombre: String, tipo: SilType },
    AsignarArena { arena: ArenaId, nombre: String, valor: ValueId },
    Retornar { valor: Option<ValueId> },
}

/// Tarea en IR.
#[derive(Debug, Clone, Default)]
pub struct TareaIR {
    pub nombre: String,
}
