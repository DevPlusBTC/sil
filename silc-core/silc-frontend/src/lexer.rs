//! Lexer determinista para SIL (CNL) usando `logos`.
//!
//! M0: esqueleto con tokens básicos. Implementación completa (keywords CNL,
//! indentación INDENT/DEDENT, longest-match) en M1.

use logos::Logos;

/// Token léxico de SIL.
#[derive(Debug, Clone, PartialEq, Logos)]
#[logos(error = LexError)]
pub enum Token {
    // --- Palabras clave (subset M0, completo en M1) ---
    #[token("definir")]
    Definir,
    #[token("tarea")]
    Tarea,
    #[token("retornar")]
    Retornar,
    #[token("verificar")]
    Verificar,
    #[token("que")]
    Que,
    #[token("asumir")]
    Asumir,
    #[token("demostrar")]
    Demostrar,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("bajo")]
    Bajo,
    #[token("restricciones")]
    Restricciones,

    // --- Tipos ---
    #[token("Entero64")]
    TEntero64,
    #[token("Booleano")]
    TBooleano,
    #[token("Texto")]
    TTexto,
    #[token("Void")]
    TVoid,

    // --- Literales ---
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Entero(i64),
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

    // --- Símbolos ---
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("->")]
    Arrow,
    #[token("=")]
    Eq,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("+")]
    Plus,
    #[token("*")]
    Star,

    // --- Estructura (M1: INDENT/DEDENT real desde indent_stack) ---
    #[token("\n")]
    Newline,

    // --- Ignorados ---
    #[regex(r"[ \t]+", logos::skip)]
    #[regex(r"//[^\n]*", logos::skip)]
    Ignorado,
}

/// Error léxico.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LexError;

/// Token con span (para el parser).
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub tok: Token,
    pub span: std::ops::Range<usize>,
}

/// Lexea código fuente completo. M0: sin INDENT/DEDENT (M1).
pub fn lexear(fuente: &str) -> Vec<SpannedToken> {
    Token::lexer(fuente)
        .spanned()
        .filter_map(|(res, span)| res.ok().map(|tok| SpannedToken { tok, span }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexea_tarea_minima() {
        let toks = lexear("definir tarea f():");
        assert!(toks.iter().any(|t| t.tok == Token::Definir));
        assert!(toks.iter().any(|t| t.tok == Token::Tarea));
    }
}
