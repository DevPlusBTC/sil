//! Nodos Causal-IR: cuadruplo ortogonal ⟨ID_SSA, Operacion, Capacidad, Invariante⟩.
//!
//! M3: definicion completa — tipos, operaciones, bloques, arenas.
//! Corresponde a Whitepaper §7.2.

use slotmap::{new_key_type, SlotMap, Key};
use std::collections::HashMap;

new_key_type! {
    /// ID estable para valores SSA.
    pub struct ValueId;
    /// ID estable para bloques basicos.
    pub struct BlockId;
    /// ID estable para arenas.
    pub struct ArenaId;
}

// =============================================================================
// Tipos
// =============================================================================

/// Tipo SIL completo (mapeo 1:1 desde AST TipoDato).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SilType {
    Entero64,
    Flotante64,
    Booleano,
    Texto,
    CapacidadHardware,
    Void,
    USD,
    EUR,
    Lista(Box<SilType>),
    Mapa(Box<SilType>, Box<SilType>),
    Conjunto(Box<SilType>),
    Tupla(Vec<SilType>),
    Nominal(String),
}

// =============================================================================
// Capacidades (Zero-Trust)
// =============================================================================

/// Permiso de capacidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permiso {
    LecturaArchivo,
    EscrituraArchivo,
    Red,
    Ejecucion,
}

/// Requerimiento de capacidad para operacion de E/S.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapacidadReq {
    pub permiso: Permiso,
    pub ttl_ns: u64,
    pub recurso: Option<String>,
}

impl CapacidadReq {
    /// TTL por defecto: 500 microsegundos (500_000 ns).
    pub const TTL_DEFECTO_NS: u64 = 500_000;

    pub fn red(recurso: Option<String>) -> Self {
        Self {
            permiso: Permiso::Red,
            ttl_ns: Self::TTL_DEFECTO_NS,
            recurso,
        }
    }
}

// =============================================================================
// Invariantes SMT
// =============================================================================

/// Clase de invariante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseInvariante {
    Asuncion,
    Demostracion,
}

/// Formula logica (para traduccion SMT).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FormulaLogica {
    Var(String),
    VarId(ValueId),
    ConstInt(i64),
    ConstBool(bool),
    BinOp {
        op: OpLogico,
        lhs: Box<FormulaLogica>,
        rhs: Box<FormulaLogica>,
    },
    No(Box<FormulaLogica>),
}

/// Operador logico/aritmetico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpLogico {
    Gt,
    Lt,
    Eq,
    Ne,
    Ge,
    Le,
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
}

/// Invariante SMT adjunta a instruccion.
#[derive(Debug, Clone)]
pub struct InvarianteSMT {
    pub formula: FormulaLogica,
    pub hash: [u8; 64],
    pub clase: ClaseInvariante,
}

impl InvarianteSMT {
    /// Calcula hash SHA3-512 canonico de la formula.
    pub fn con_hash(formula: FormulaLogica, clase: ClaseInvariante) -> Self {
        use sha3::{Digest, Sha3_512};
        let canon = canonizar(&formula);
        let mut h = Sha3_512::new();
        h.update(canon.as_bytes());
        let digest = h.finalize();
        let mut hash = [0u8; 64];
        hash.copy_from_slice(&digest);
        Self {
            formula,
            hash,
            clase,
        }
    }
}

/// Serializacion canonica para hashing estable.
/// Usa ValueId directamente para VarId (unico y estable).
pub fn canonizar(f: &FormulaLogica) -> String {
    match f {
        FormulaLogica::Var(v) => format!("V:{v}"),
        FormulaLogica::VarId(v) => format!("V#{}", v.data().as_ffi()), // ValueId interno
        FormulaLogica::ConstInt(n) => format!("I:{n}"),
        FormulaLogica::ConstBool(b) => format!("B:{b}"),
        FormulaLogica::BinOp { op, lhs, rhs } => {
            format!("({:?} {} {})", op, canonizar(lhs), canonizar(rhs))
        }
        FormulaLogica::No(x) => format!("(No {})", canonizar(x)),
    }
}

/// Convierte formula a SMT-LIB2 con nombres fuente (para debug/auditoria).
/// Requiere mapa ValueId -> nombre fuente.
pub fn a_smtlib2_con_nombres(f: &FormulaLogica, nombres: &std::collections::HashMap<ValueId, String>) -> String {
    match f {
        FormulaLogica::Var(v) => v.clone(),
        FormulaLogica::VarId(v) => nombres.get(v).cloned().unwrap_or_else(|| format!("v{}", v.data().as_ffi())),
        FormulaLogica::ConstInt(n) => {
            if *n < 0 {
                format!("(- {})", n.abs())
            } else {
                format!("{n}")
            }
        }
        FormulaLogica::ConstBool(b) => format!("{b}"),
        FormulaLogica::BinOp { op, lhs, rhs } => {
            let o = match op {
                OpLogico::Gt => ">",
                OpLogico::Lt => "<",
                OpLogico::Eq => "=",
                OpLogico::Ne => "distinct",
                OpLogico::Ge => ">=",
                OpLogico::Le => "<=",
                OpLogico::Add => "+",
                OpLogico::Sub => "-",
                OpLogico::Mul => "*",
                OpLogico::Div => "div",
                OpLogico::And => "and",
                OpLogico::Or => "or",
            };
            format!("({o} {} {})", a_smtlib2_con_nombres(lhs, nombres), a_smtlib2_con_nombres(rhs, nombres))
        }
        FormulaLogica::No(x) => format!("(not {})", a_smtlib2_con_nombres(x, nombres)),
    }
}

// =============================================================================
// Operaciones IR
// =============================================================================

/// Constante.
#[derive(Debug, Clone, PartialEq)]
pub enum Constante {
    Int(i64),
    Float(f64),
    Bool(bool),
    Texto(String),
}

/// Operador aritmetico (con flag de seguridad SMT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpArit {
    Add,
    Sub,
    Mul,
    Div,
}

/// Operador de comparacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpCmp {
    Gt,
    Lt,
    Eq,
    Ne,
    Ge,
    Le,
}

/// Operacion de E/S mediada por capacidad.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OpIO {
    LeerArchivo,
    EscribirArchivo,
    EnviarRed,
    RecibirRed,
}

/// Instruccion causal.
#[derive(Debug, Clone)]
pub struct InstrCausal {
    /// Resultado SSA (None para instrucciones sin valor: retornar-void, solicitar-cap, etc.)
    pub resultado: Option<ValueId>,
    pub op: Operacion,
    pub capacidad: Option<CapacidadReq>,
    pub invariante: Option<InvarianteSMT>,
}

/// Operacion IR.
#[derive(Debug, Clone)]
pub enum Operacion {
    Param {
        nombre: String,
        tipo: SilType,
    },
    Const {
        valor: Constante,
        tipo: SilType,
    },
    AsignarArena {
        arena: ArenaId,
        nombre: String,
        valor: ValueId,
    },
    Promover {
        valor: ValueId,
        destino: ArenaId,
    },
    BinOpSegura {
        op: OpArit,
        lhs: ValueId,
        rhs: ValueId,
        /// true si SMT probo no-overflow → backend emite `nsw` y omite checks.
        sin_overflow: bool,
    },
    Comparar {
        op: OpCmp,
        lhs: ValueId,
        rhs: ValueId,
    },
    Retornar {
        valor: Option<ValueId>,
    },
    Ramificar {
        cond: ValueId,
        entonces: BlockId,
        sino: BlockId,
    },
    Saltar {
        destino: BlockId,
    },
    SolicitarCapacidad {
        req: CapacidadReq,
    },
    ValidarCapacidad {
        token: ValueId,
    },
    InvocarIO {
        operacion: OpIO,
        args: Vec<ValueId>,
        token: ValueId,
    },
    CrearFibra {
        entrada: BlockId,
        arena: ArenaId,
    },
    EnviarCanal {
        canal: ValueId,
        valor: ValueId,
    },
    RecibirCanal {
        canal: ValueId,
    },
}

// =============================================================================
// Bloques y funcion
// =============================================================================

/// Terminador de bloque.
#[derive(Debug, Clone)]
pub enum Terminador {
    Retorno(Option<ValueId>),
    Salto(BlockId),
    Rama {
        cond: ValueId,
        entonces: BlockId,
        sino: BlockId,
    },
    Inalcanzable,
}

/// Bloque basico.
#[derive(Debug, Clone)]
pub struct Bloque {
    pub id: BlockId,
    pub instrs: Vec<InstrCausal>,
    pub terminador: Terminador,
}

/// Clase de arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaKind {
    /// Arena local de tarea (vive lo que la tarea).
    TareaLocal,
    /// Sub-arena ciclica por iteracion (liberacion O(1) al fin de ciclo).
    Ciclica,
    /// Arena de persistencia long-lived (hoisting).
    Persistente,
}

/// Info de arena.
#[derive(Debug, Clone)]
pub struct ArenaInfo {
    pub kind: ArenaKind,
    pub nombre: String,
}

/// Restriccion de hardware (del bloque `bajo restricciones:`).
#[derive(Debug, Clone)]
pub struct Restriccion {
    pub clave: String,
    pub valor: String,
}

/// Tarea en IR: CFG + tablas SSA + arenas + restricciones.
#[derive(Debug, Clone)]
pub struct TareaIR {
    pub nombre: String,
    pub params: Vec<(String, SilType)>,
    pub retorno: SilType,
    pub bloques: SlotMap<BlockId, Bloque>,
    pub entrada: BlockId,
    /// Tipos de cada valor SSA.
    pub tipos: HashMap<ValueId, SilType>,
    /// Nombres de variables (para debug/smt): ValueId → nombre fuente.
    pub nombres: HashMap<ValueId, String>,
    pub arenas: SlotMap<ArenaId, ArenaInfo>,
    pub arena_local: ArenaId,
    pub restricciones: Vec<Restriccion>,
}

impl TareaIR {
    /// Itera invariantes por clase (para el verificador SMT).
    pub fn invariantes(&self, clase: ClaseInvariante) -> Vec<&InvarianteSMT> {
        let mut out = Vec::new();
        for (_, b) in self.bloques.iter() {
            for i in &b.instrs {
                if let Some(inv) = &i.invariante {
                    if inv.clase == clase {
                        out.push(inv);
                    }
                }
            }
        }
        out
    }

    /// Numero total de instrucciones (para metricas).
    pub fn num_instrs(&self) -> usize {
        self.bloques.values().map(|b| b.instrs.len()).sum()
    }
}
