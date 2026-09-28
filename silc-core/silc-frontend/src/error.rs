//! Errores del frontend con diagnósticos humanos (miette).
//!
//! Corresponde a Whitepaper §10.1: errores en lenguaje natural controlado,
//! no códigos opacos.

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Error del frontend (lexer + parser).
#[derive(Debug, Error, Diagnostic)]
pub enum ErrorFrontend {
    #[error("Carácter no reconocido '{car}'")]
    #[diagnostic(
        code(silc::lexer::caracter_desconocido),
        help("Revisa la sintaxis CNL. Caracteres válidos: letras, dígitos, _, -, \"...\". Palabras clave en minúsculas.")
    )]
    CaracterDesconocido {
        car: char,
        #[label("aquí")]
        span: SourceSpan,
    },

    #[error("Se esperaba {esperado} pero se encontró '{encontrado}'")]
    #[diagnostic(
        code(silc::parser::inesperado),
        help("Revisa la gramática CNL. Ejemplo: `definir tarea nombre(param: Tipo) -> Tipo:`")
    )]
    TokenInesperado {
        esperado: String,
        encontrado: String,
        #[label("aquí")]
        span: SourceSpan,
    },

    #[error("Fin de archivo inesperado")]
    #[diagnostic(
        code(silc::parser::eof_inesperado),
        help("Falta cerrar un bloque con la indentación correcta o falta el cuerpo de la tarea.")
    )]
    FinInesperado,
}
