//! AST tipado para SIL con información de spans.
//!
//! Corresponde a la gramática EBNF del Whitepaper §2.

use std::ops::Range;

/// Span de código fuente (byte offsets).
pub type Span = Range<usize>;

/// Identificador con span.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
    pub nombre: String,
    pub span: Span,
}

/// Programa completo: lista de declaraciones.
#[derive(Debug, Clone)]
pub struct Program {
    pub defs: Vec<Decl>,
}

/// Declaración de nivel superior.
#[derive(Debug, Clone)]
pub enum Decl {
    Tarea(TareaDecl),
    Estructura(EstructuraDecl),
    Variante(VarianteDecl),
    // TODO(M2): Servicio, Modelo, MemoriaPersistente, Capacidad, Modulo, Migracion
}

/// Declaración de tarea (función).
#[derive(Debug, Clone)]
pub struct TareaDecl {
    pub nombre: Ident,
    pub params: Vec<Param>,
    pub retorno: Option<TipoDato>,
    pub cuerpo: Cuerpo,
    pub span: Span,
}

/// Parámetro: nombre + tipo.
#[derive(Debug, Clone)]
pub struct Param {
    pub nombre: Ident,
    pub tipo: TipoDato,
}

/// Cuerpo: sentencias + restricciones opcionales.
#[derive(Debug, Clone, Default)]
pub struct Cuerpo {
    pub stmts: Vec<Stmt>,
    pub restricciones: Vec<Restriccion>,
}

/// Sentencia CNL.
#[derive(Debug, Clone)]
pub enum Stmt {
    Verificar {
        cond: Expr,
        span: Span,
    },
    Asumir {
        cond: Expr,
        span: Span,
    },
    Demostrar {
        cond: Expr,
        span: Span,
    },
    Asignar {
        nombre: Ident,
        tipo: Option<TipoDato>,
        expr: Expr,
        mutable: bool,
        span: Span,
    },
    Retornar {
        expr: Option<Expr>,
        span: Span,
    },
    /// solicitar capacidad <permiso> para "<recurso>" [con ttl <ns>] [como <nombre>]
    SolicitarCapacidad {
        permiso: PermisoCap,
        recurso: String,
        ttl_ns: Option<u64>,
        guardar_como: Option<Ident>,
        span: Span,
    },
    /// validar <nombre>
    ValidarCapacidad {
        token: Ident,
        span: Span,
    },
    /// enviar <expr> por <canal>
    EnviarCanal {
        valor: Expr,
        canal: Ident,
        span: Span,
    },
    /// recibir de <canal> como <nombre>
    RecibirCanal {
        canal: Ident,
        guardar_como: Ident,
        span: Span,
    },
    /// generar <tarea> (la tarea debe existir y no tener parámetros)
    GenerarFibra {
        tarea: Ident,
        span: Span,
    },
    /// crear canal [con capacidad <n>] como <nombre>
    CrearCanal {
        capacidad: Option<u64>,
        guardar_como: Ident,
        span: Span,
    },
    /// leer archivo "<ruta>" con <cap> como <nombre>
    /// escribir <expr> en archivo "<ruta>" con <cap>
    OperarIO {
        operacion: OpIOStmt,
        ruta: String,
        datos: Option<Expr>,
        capacidad: Ident,
        guardar_como: Option<Ident>,
        span: Span,
    },
}

/// Permiso de capacidad en sintaxis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermisoCap {
    LecturaArchivo,
    EscrituraArchivo,
    Red,
    Ejecucion,
}

/// Operacion de E/S en sintaxis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpIOStmt {
    LeerArchivo,
    EscribirArchivo,
    EnviarRed,
    RecibirRed,
}

/// Expresión.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Var(Ident),
    Lit(Literal),
    AccesoProp {
        base: Box<Expr>,
        prop: Ident,
        span: Span,
    },
    BinOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
}

/// Literal.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Entero(i64),
    Flotante(f64),
    Texto(String),
    Booleano(bool),
}

/// Operador binario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
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
}

/// Tipo de dato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TipoDato {
    Entero64,
    Flotante64,
    Booleano,
    Texto,
    CapacidadHardware,
    Void,
    USD,
    EUR,
    Lista(Box<TipoDato>),
    Mapa(Box<TipoDato>, Box<TipoDato>),
    Conjunto(Box<TipoDato>),
    Tupla(Vec<TipoDato>),
    Nominal(String),
}

/// Declaración de estructura (product type).
#[derive(Debug, Clone)]
pub struct EstructuraDecl {
    pub nombre: Ident,
    pub campos: Vec<Campo>,
    pub span: Span,
}

/// Campo de estructura.
#[derive(Debug, Clone)]
pub struct Campo {
    pub nombre: Ident,
    pub tipo: TipoDato,
}

/// Declaración de variante (sum type / ADT).
#[derive(Debug, Clone)]
pub struct VarianteDecl {
    pub nombre: Ident,
    pub casos: Vec<CasoVariante>,
    pub span: Span,
}

/// Caso de variante.
#[derive(Debug, Clone)]
pub struct CasoVariante {
    pub nombre: Ident,
    pub datos: Vec<Campo>,
}

/// Restricción de hardware.
#[derive(Debug, Clone)]
pub struct Restriccion {
    pub clave: String,
    pub valor: String,
    pub span: Span,
}
