//! Errores del frontend con diagnósticos humanos (miette).
//!
//! Corresponde a Whitepaper §10.1: errores en lenguaje natural controlado,
//! no códigos opacos.

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Error del frontend (reader + AST builder).
#[derive(Debug, Error, Diagnostic)]
pub enum ErrorFrontend {
    #[error("Carácter no reconocido '{car}'")]
    #[diagnostic(
        code(silc::reader::caracter_desconocido),
        help("Revisa la sintaxis S-exp. Caracteres válidos: letras, dígitos, _, -, \"...\". Palabras clave en minúsculas.")
    )]
    CaracterDesconocido {
        car: char,
        #[label("aquí")]
        span: SourceSpan,
    },

    #[error("Se esperaba {esperado} pero se encontró '{encontrado}'")]
    #[diagnostic(
        code(silc::a_ast::inesperado),
        help("Revisa la gramática S-exp. Ejemplo: `(task nombre (params (x i64)) (returns i64) (body (return x)))`")
    )]
    TokenInesperado {
        esperado: String,
        encontrado: String,
        #[label("aquí")]
        span: SourceSpan,
    },

    #[error("Fin de archivo inesperado")]
    #[diagnostic(
        code(silc::reader::eof_inesperado),
        help("Falta cerrar un paréntesis o el cuerpo de la tarea.")
    )]
    FinInesperado,

    #[error("Expresión anidada demasiado profunda (máximo {max})")]
    #[diagnostic(
        code(silc::reader::expresion_profunda),
        help("Simplifica la expresión. SIL limita el anidamiento para evitar desbordamiento de pila en el reader.")
    )]
    ExpresionDemasiadoProfunda {
        max: u32,
        #[label("aquí")]
        span: SourceSpan,
    },

    #[error("Forma desconocida '{nombre}'")]
    #[diagnostic(
        code(silc::sexp::forma_desconocida),
        help("Consulta docs/SPEC-v2.md para la lista de formas válidas.")
    )]
    FormaDesconocida {
        nombre: String,
        #[label("esta forma")]
        span: SourceSpan,
    },

    #[error("'{forma}' esperaba {esperado} y recibió {obtenido}")]
    #[diagnostic(
        code(silc::sexp::aridad),
        help("Revisa el número de elementos de la forma en docs/SPEC-v2.md.")
    )]
    AridadIncorrecta {
        forma: String,
        esperado: String,
        obtenido: usize,
        #[label("esta forma")]
        span: SourceSpan,
    },

    #[error("Identificador inválido '{nombre}'")]
    #[diagnostic(
        code(silc::sexp::identificador),
        help("Un identificador empieza por letra o '_' y sigue con letras, dígitos o '_'. 'verdadero' y 'falso' están reservados como literales.")
    )]
    IdentificadorInvalido {
        nombre: String,
        #[label("aquí")]
        span: SourceSpan,
    },
}
