//! Lector S-expression para SIL v2.
//!
//! Un Sexp es un átomo, un texto entre comillas o una lista. No hay keywords,
//! precedencia ni INDENT/DEDENT: toda la gramática vive en `a_ast.rs`, y este
//! módulo solo convierte bytes en árbol.

use crate::ast::Span;
use crate::error::ErrorFrontend;
use miette::SourceSpan;

/// Profundidad máxima de anidamiento (evita desbordar la pila del lector).
pub const MAX_PROFUNDIDAD: u32 = 256;

/// Nodo S-expression con su span en bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum Sexp {
    /// Identificador, número u operador. El texto es literal, sin interpretar.
    Atomo { texto: String, span: Span },
    /// Literal de texto entre comillas, ya desescapado.
    Texto { valor: String, span: Span },
    /// Lista `( ... )`, incluye los paréntesis en el span.
    Lista { items: Vec<Sexp>, span: Span },
}

impl Sexp {
    pub fn span(&self) -> &Span {
        match self {
            Sexp::Atomo { span, .. } | Sexp::Texto { span, .. } | Sexp::Lista { span, .. } => span,
        }
    }

    /// Texto del átomo, o `None` si es texto o lista.
    pub fn atomo(&self) -> Option<&str> {
        match self {
            Sexp::Atomo { texto, .. } => Some(texto),
            _ => None,
        }
    }

    /// Elementos de la lista, o `None` si no lo es.
    pub fn items(&self) -> Option<&[Sexp]> {
        match self {
            Sexp::Lista { items, .. } => Some(items),
            _ => None,
        }
    }
}

/// Lee una fuente completa en una secuencia de formas de nivel superior.
pub fn leer(fuente: &str) -> Result<Vec<Sexp>, ErrorFrontend> {
    let mut l = Lector::nuevo(fuente);
    let mut formas = Vec::new();
    l.saltar();
    while l.peek().is_some() {
        formas.push(l.sexp(0)?);
        l.saltar();
    }
    Ok(formas)
}

struct Lector<'a> {
    fuente: &'a str,
    chars: Vec<(usize, char)>,
    pos: usize,
}

impl<'a> Lector<'a> {
    fn nuevo(fuente: &'a str) -> Self {
        Self {
            fuente,
            chars: fuente.char_indices().collect(),
            pos: 0,
        }
    }

    /// Offset en bytes del carácter `i` (o el final de la fuente).
    fn byte(&self, i: usize) -> usize {
        self.chars.get(i).map_or(self.fuente.len(), |(b, _)| *b)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).map(|(_, c)| *c)
    }

    /// Consume espacios y comentarios `;` hasta fin de línea.
    fn saltar(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => self.pos += 1,
                Some(';') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn sexp(&mut self, prof: u32) -> Result<Sexp, ErrorFrontend> {
        let inicio = self.byte(self.pos);
        match self.peek() {
            Some('(') => {
                if prof >= MAX_PROFUNDIDAD {
                    return Err(ErrorFrontend::ExpresionDemasiadoProfunda {
                        max: MAX_PROFUNDIDAD,
                        span: (inicio, 1).into(),
                    });
                }
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.saltar();
                    match self.peek() {
                        Some(')') => {
                            self.pos += 1;
                            return Ok(Sexp::Lista {
                                items,
                                span: inicio..self.byte(self.pos),
                            });
                        }
                        Some(_) => items.push(self.sexp(prof + 1)?),
                        None => {
                            return Err(ErrorFrontend::TokenInesperado {
                                esperado: "')'".to_string(),
                                encontrado: "fin de archivo".to_string(),
                                span: span(inicio, 1),
                            })
                        }
                    }
                }
            }
            Some(')') => Err(ErrorFrontend::TokenInesperado {
                esperado: "expresión".to_string(),
                encontrado: "')'".to_string(),
                span: span(inicio, 1),
            }),
            Some('"') => self.texto(inicio),
            Some(_) => self.atomo(inicio),
            None => Err(ErrorFrontend::FinInesperado),
        }
    }

    fn atomo(&mut self, inicio: usize) -> Result<Sexp, ErrorFrontend> {
        let desde = self.pos;
        while let Some(c) = self.peek() {
            if c.is_whitespace() || matches!(c, '(' | ')' | '"' | ';') {
                break;
            }
            self.pos += 1;
        }
        Ok(Sexp::Atomo {
            texto: self.chars[desde..self.pos]
                .iter()
                .map(|(_, c)| *c)
                .collect(),
            span: inicio..self.byte(self.pos),
        })
    }

    /// Mismos escapes que el lexer CNL: `\\ \" \n \t \r`; el resto se conserva.
    fn texto(&mut self, inicio: usize) -> Result<Sexp, ErrorFrontend> {
        self.pos += 1; // comilla de apertura
        let mut valor = String::new();
        loop {
            match self.peek() {
                Some('"') => {
                    self.pos += 1;
                    return Ok(Sexp::Texto {
                        valor,
                        span: inicio..self.byte(self.pos),
                    });
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some('\\') => valor.push('\\'),
                        Some('"') => valor.push('"'),
                        Some('n') => valor.push('\n'),
                        Some('t') => valor.push('\t'),
                        Some('r') => valor.push('\r'),
                        Some(otro) => {
                            valor.push('\\');
                            valor.push(otro);
                        }
                        None => return Err(ErrorFrontend::FinInesperado),
                    }
                    self.pos += 1;
                }
                Some(c) => {
                    valor.push(c);
                    self.pos += 1;
                }
                None => {
                    return Err(ErrorFrontend::TokenInesperado {
                        esperado: "'\"'".to_string(),
                        encontrado: "fin de archivo".to_string(),
                        span: span(inicio, 1),
                    })
                }
            }
        }
    }
}

fn span(offset: usize, len: usize) -> SourceSpan {
    (offset, len).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atomo(s: &Sexp) -> &str {
        s.atomo().expect("átomo")
    }

    #[test]
    fn atomo_y_listas_anidadas() {
        let f = leer("(task sumar (params (x i64)))").unwrap();
        assert_eq!(f.len(), 1);
        let items = f[0].items().unwrap();
        assert_eq!(atomo(&items[0]), "task");
        assert_eq!(atomo(&items[1]), "sumar");
        assert_eq!(items[2].items().unwrap().len(), 2);
    }

    #[test]
    fn comentarios_y_varias_formas() {
        let f = leer("; cabecera\n(a)\n; otra\n(b)").unwrap();
        assert_eq!(f.len(), 2);
    }

    #[test]
    fn texto_con_escapes() {
        let f = leer(r#"(ruta "a\"b\\c\nd")"#).unwrap();
        match &f[0].items().unwrap()[1] {
            Sexp::Texto { valor, .. } => assert_eq!(valor, "a\"b\\c\nd"),
            otro => panic!("esperaba texto, got {otro:?}"),
        }
    }

    #[test]
    fn span_cubre_los_parentesis() {
        let f = leer("  (a b)").unwrap();
        assert_eq!(f[0].span(), &(2..7));
    }

    #[test]
    fn span_correcto_con_utf8() {
        let f = leer("(ñandú x)").unwrap();
        // 'ñ' y 'ú' ocupan 2 bytes cada uno: ( = 1, ñandú = 7, espacio = 1, x = 1, ) = 1
        assert_eq!(f[0].span(), &(0..11));
        assert_eq!(atomo(&f[0].items().unwrap()[0]), "ñandú");
    }

    #[test]
    fn parentesis_sin_cerrar_falla() {
        assert!(matches!(
            leer("(a b"),
            Err(ErrorFrontend::TokenInesperado { .. })
        ));
    }

    #[test]
    fn parentesis_de_mas_falla() {
        assert!(matches!(
            leer("(a))"),
            Err(ErrorFrontend::TokenInesperado { .. })
        ));
    }

    #[test]
    fn texto_sin_cerrar_falla() {
        assert!(matches!(
            leer("(a \"x)"),
            Err(ErrorFrontend::TokenInesperado { .. })
        ));
    }

    #[test]
    fn anidamiento_excesivo_no_desborda_pila() {
        let fuente = "(".repeat(MAX_PROFUNDIDAD as usize + 10);
        assert!(matches!(
            leer(&fuente),
            Err(ErrorFrontend::ExpresionDemasiadoProfunda { .. })
        ));
    }

    #[test]
    fn operadores_son_atomos() {
        let f = leer("(+ a b)").unwrap();
        assert_eq!(atomo(&f[0].items().unwrap()[0]), "+");
    }
}
