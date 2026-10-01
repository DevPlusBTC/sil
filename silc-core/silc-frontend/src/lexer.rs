//! Lexer determinista para SIL (CNL) usando `logos`.
//!
//! M1: implementacion completa — keywords CNL, tipos, literales,
//! simbolos multi-caracter, comentarios, INDENT/DEDENT.

use logos::Logos;

#[derive(Debug, Clone, Default)]
pub struct IndentState {
    pub stack: Vec<usize>,
    pub pendientes: std::collections::VecDeque<TokenEstructural>,
}

impl IndentState {
    fn new() -> Self {
        Self {
            stack: vec![0],
            pendientes: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenEstructural {
    Indent,
    Dedent,
    Newline,
}

#[derive(Debug, Clone, PartialEq, Logos)]
#[logos(error = LexError, extras = IndentState)]
pub enum Token {
    #[token("definir")]
    Definir,
    #[token("tarea")]
    Tarea,
    #[token("servicio")]
    Servicio,
    #[token("modelo")]
    Modelo,
    #[token("evento")]
    Evento,
    #[token("estructura")]
    Estructura,
    #[token("variante")]
    Variante,
    #[token("opcion")]
    Opcion,
    #[token("memoria_persistente")]
    MemoriaPersistente,
    #[token("migrar")]
    Migrar,
    #[token("capacidad")]
    Capacidad,
    #[token("modulo")]
    Modulo,
    #[token("requerir")]
    Requerir,
    #[token("importar")]
    Importar,
    #[token("exportar")]
    Exportar,
    #[token("vincular")]
    Vincular,
    #[token("biblioteca_nativa")]
    BibliotecaNativa,
    #[token("funcion_externa")]
    FuncionExterna,
    #[token("cuando")]
    Cuando,
    #[token("llegue")]
    Llegue,
    #[token("escuchar")]
    Escuchar,
    #[token("en")]
    En,
    #[token("puerto")]
    Puerto,
    #[token("con")]
    Con,
    #[token("protocolo")]
    Protocolo,
    #[token("para")]
    Para,
    #[token("cada")]
    Cada,
    #[token("mientras")]
    Mientras,
    #[token("si")]
    Si,
    #[token("entonces")]
    Entonces,
    #[token("retornar")]
    Retornar,
    #[token("ejecutar")]
    Ejecutar,
    #[token("cualquier")]
    Cualquier,
    #[token("otro")]
    Otro,
    #[token("caso")]
    Caso,
    #[token("coincidir")]
    Coincidir,
    #[token("desde")]
    Desde,
    #[token("hasta")]
    Hasta,
    #[token("como")]
    Como,
    #[token("datos")]
    Datos,
    #[token("de")]
    De,
    #[token("forma")]
    Forma,
    #[token("verificar")]
    Verificar,
    #[token("que")]
    Que,
    #[token("sea")]
    Sea,
    #[token("igual")]
    Igual,
    #[token("mayor")]
    Mayor,
    #[token("menor")]
    Menor,
    #[token("diferente")]
    Diferente,
    #[token("esta_activo")]
    EstaActivo,
    #[token("asumir")]
    Asumir,
    #[token("demostrar")]
    Demostrar,
    #[token("probar")]
    Probar,
    #[token("propiedad")]
    Propiedad,
    #[token("convertir")]
    Convertir,
    #[token("a", priority = 3)]
    A,
    #[token("usando")]
    Usando,
    #[token("guardar")]
    Guardar,
    #[token("enviar")]
    Enviar,
    #[token("permitir")]
    Permitir,
    #[token("conectar")]
    Conectar,
    #[token("promover")]
    Promover,
    #[token("al")]
    Al,
    #[token("acceder_campo")]
    AccederCampo,
    #[token("valor_predeterminado")]
    ValorPredeterminado,
    #[token("bajo")]
    Bajo,
    #[token("restricciones")]
    Restricciones,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("verdadero")]
    Verdadero,
    #[token("falso")]
    Falso,
    #[token("nulo")]
    Nulo,
    #[token("atomica")]
    Atomica,
    #[token("asincrona")]
    Asincrona,
    #[token("en_memoria")]
    EnMemoria,
    #[token("arena")]
    Arena,
    #[token("cero_pausas")]
    CeroPausas,
    #[token("Entero64")]
    TEntero64,
    #[token("Flotante64")]
    TFlotante64,
    #[token("Booleano")]
    TBooleano,
    #[token("Texto")]
    TTexto,
    #[token("CapacidadHardware")]
    TCapacidadHardware,
    #[token("Void")]
    TVoid,
    #[token("Tensor")]
    TTensor,
    #[token("Lista")]
    TLista,
    #[token("Mapa")]
    TMapa,
    #[token("Conjunto")]
    TConjunto,
    #[token("Tupla")]
    TTupla,
    #[token("USD")]
    TUSD,
    #[token("EUR")]
    TEUR,
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Flotante(f64),
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Entero(i64),
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    Texto(String),
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_\-]*", |lex| lex.slice().to_string())]
    Ident(String),
    #[token("->")]
    Arrow,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token(">=")]
    Ge,
    #[token("<=")]
    Le,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("|")]
    Pipe,
    #[token("=")]
    Eq,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[regex(r"\n[ \t]*", callback_indent)]
    Newline,
    #[regex(r"[ \t]+", logos::skip)]
    Espacio,
    #[regex(r"//[^\n]*", logos::skip)]
    ComentarioLinea,
    #[regex(r"/\*([^*]|\*[^/])*\*/", logos::skip)]
    ComentarioBloque,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LexError;

fn callback_indent(lex: &mut logos::Lexer<Token>) -> Option<()> {
    let slice = lex.slice();
    let columna = slice.len().saturating_sub(1);
    let state: &mut IndentState = &mut lex.extras;
    if state.stack.is_empty() {
        state.stack.push(0);
    }
    let actual = *state.stack.last().unwrap();
    if columna > actual {
        state.stack.push(columna);
        state.pendientes.push_back(TokenEstructural::Indent);
    } else {
        while state.stack.len() > 1 && *state.stack.last().unwrap() > columna {
            state.stack.pop();
            state.pendientes.push_back(TokenEstructural::Dedent);
        }
    }
    Some(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub tok: Token,
    pub span: std::ops::Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenFull {
    Tok(SpannedToken),
    Indent { span: std::ops::Range<usize> },
    Dedent { span: std::ops::Range<usize> },
    Newline { span: std::ops::Range<usize> },
    Eof,
}

pub fn lexear(fuente: &str) -> Vec<TokenFull> {
    let mut lex = Token::lexer(fuente);
    lex.extras = IndentState::new();
    let mut out: Vec<TokenFull> = Vec::new();
    while let Some(res) = lex.next() {
        let span = lex.span();
        match res {
            Ok(Token::Newline) => {
                out.push(TokenFull::Newline { span: span.clone() });
                while let Some(est) = lex.extras.pendientes.pop_front() {
                    match est {
                        TokenEstructural::Indent => {
                            out.push(TokenFull::Indent { span: span.clone() })
                        }
                        TokenEstructural::Dedent => {
                            out.push(TokenFull::Dedent { span: span.clone() })
                        }
                        TokenEstructural::Newline => {}
                    }
                }
            }
            Ok(tok) => out.push(TokenFull::Tok(SpannedToken { tok, span })),
            Err(_) => {
                out.push(TokenFull::Tok(SpannedToken {
                    tok: Token::Ident(String::from("\u{0}ERR")),
                    span,
                }));
            }
        }
    }
    let eof_span = fuente.len()..fuente.len();
    while lex.extras.stack.len() > 1 {
        lex.extras.stack.pop();
        out.push(TokenFull::Dedent {
            span: eof_span.clone(),
        });
    }
    out.push(TokenFull::Eof);
    out
}

pub fn lexear_simple(fuente: &str) -> Vec<SpannedToken> {
    lexear(fuente)
        .into_iter()
        .filter_map(|t| match t {
            TokenFull::Tok(s) => Some(s),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn toks(f: &str) -> Vec<TokenFull> {
        lexear(f)
    }
    fn es_tok(t: &TokenFull, pred: impl Fn(&Token) -> bool) -> bool {
        matches!(t, TokenFull::Tok(s) if pred(&s.tok))
    }

    #[test]
    fn lexea_tarea_minima() {
        let t = toks("definir tarea f():");
        assert!(t.iter().any(|x| es_tok(x, |k| *k == Token::Definir)));
        assert!(t.iter().any(|x| es_tok(x, |k| *k == Token::Tarea)));
    }

    #[test]
    fn keywords_cnl_completos() {
        let t = toks("servicio modelo evento estructura variante opcion memoria_persistente migrar capacidad");
        let kws: Vec<_> = t
            .iter()
            .filter_map(|x| match x {
                TokenFull::Tok(s) => Some(format!("{:?}", s.tok)),
                _ => None,
            })
            .collect();
        for esperado in [
            "Servicio",
            "Modelo",
            "Evento",
            "Estructura",
            "Variante",
            "Opcion",
            "MemoriaPersistente",
            "Migrar",
            "Capacidad",
        ] {
            assert!(kws.iter().any(|k| k == esperado), "falta {esperado}");
        }
    }

    #[test]
    fn tipos_primitivos() {
        let t = toks("Entero64 Flotante64 Booleano Texto CapacidadHardware Void Tensor Lista Mapa Conjunto Tupla USD EUR");
        for tp in [
            Token::TEntero64,
            Token::TFlotante64,
            Token::TBooleano,
            Token::TTexto,
            Token::TCapacidadHardware,
            Token::TVoid,
            Token::TTensor,
            Token::TLista,
            Token::TMapa,
            Token::TConjunto,
            Token::TTupla,
            Token::TUSD,
            Token::TEUR,
        ] {
            assert!(t.iter().any(|x| es_tok(x, |k| *k == tp)), "falta {tp:?}");
        }
    }

    #[test]
    fn literales_numericos() {
        let t = toks("42 1.5");
        let enteros: Vec<_> = t
            .iter()
            .filter_map(|x| match x {
                TokenFull::Tok(s) => match &s.tok {
                    Token::Entero(n) => Some(*n),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let flotantes: Vec<_> = t
            .iter()
            .filter_map(|x| match x {
                TokenFull::Tok(s) => match &s.tok {
                    Token::Flotante(f) => Some(*f),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert!(enteros.contains(&42));
        assert!(flotantes.iter().any(|f| (*f - 1.5).abs() < 1e-9));
    }

    #[test]
    fn strings_con_interpolacion() {
        let t = toks(r#"let msg = "Hola {nombre}""#);
        let strs: Vec<_> = t
            .iter()
            .filter_map(|x| match x {
                TokenFull::Tok(s) => match &s.tok {
                    Token::Texto(st) => Some(st.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(strs.len(), 1);
        assert!(strs[0].contains("{nombre}"));
    }

    #[test]
    fn simbolos_multicaracter() {
        let t = toks("-> == != >= <=");
        let syms: Vec<String> = t
            .iter()
            .filter_map(|x| match x {
                TokenFull::Tok(s) => Some(format!("{:?}", s.tok)),
                _ => None,
            })
            .collect();
        for s in ["Arrow", "EqEq", "NotEq", "Ge", "Le"] {
            assert!(syms.iter().any(|x| x == s), "falta {s}");
        }
    }

    #[test]
    fn indent_dedent_basico() {
        let t = toks("definir tarea f():\n    retornar 1\n");
        let kinds: Vec<&str> = t
            .iter()
            .map(|x| match x {
                TokenFull::Tok(_) => "tok",
                TokenFull::Indent { .. } => "INDENT",
                TokenFull::Dedent { .. } => "DEDENT",
                TokenFull::Newline { .. } => "NL",
                TokenFull::Eof => "EOF",
            })
            .collect();
        assert!(kinds.contains(&"INDENT"), "falta INDENT: {kinds:?}");
        assert!(kinds.contains(&"DEDENT"), "falta DEDENT: {kinds:?}");
        assert_eq!(kinds.last(), Some(&"EOF"));
    }

    #[test]
    fn comentarios_ignorados() {
        let t = toks("// linea\n/* bloque */\ndefinir");
        let toks_reales: Vec<_> = t
            .iter()
            .filter_map(|x| match x {
                TokenFull::Tok(s) => Some(format!("{:?}", s.tok)),
                _ => None,
            })
            .collect();
        assert_eq!(toks_reales, vec!["Definir"]);
    }

    #[test]
    fn ejemplo_servicio_completo() {
        let src = "definir servicio S:\n    escuchar en puerto 8080\n    cuando llegue evento E(m):\n        verificar que m > 0\n        retornar m\n";
        let t = toks(src);
        assert!(t.iter().any(|x| es_tok(x, |k| *k == Token::Servicio)));
        assert!(t.iter().any(|x| es_tok(x, |k| *k == Token::Escuchar)));
        assert!(t.iter().any(|x| es_tok(x, |k| *k == Token::Cuando)));
        assert!(t.iter().any(|x| es_tok(x, |k| *k == Token::Verificar)));
        let indents = t
            .iter()
            .filter(|x| matches!(x, TokenFull::Indent { .. }))
            .count();
        assert!(indents >= 2, "esperaba indent anidado, got {indents}");
    }
}
