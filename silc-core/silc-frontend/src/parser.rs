//! Parser CNL: descenso recursivo sobre TokenFull → AST tipado.
//!
//! M2: implementacion completa del nucleo EBNF:
//!   Programa, DefinicionTarea, Cuerpo, Sentencias
//!   (verificar/asumir/demostrar/let/retornar),
//!   Expresiones con precedencia, BloqueRestricciones,
//!   Estructura, Variante.
//!
//! Errores via ErrorFrontend (miette) con spans precisos.

use crate::{
    ast::*,
    error::ErrorFrontend,
    lexer::{Token, TokenFull},
};
use miette::SourceSpan;

// =============================================================================
// Parser state
// =============================================================================

struct P<'a> {
    toks: &'a [TokenFull],
    pos: usize,
}

impl<'a> P<'a> {
    fn new(toks: &'a [TokenFull]) -> Self {
        Self { toks, pos: 0 }
    }

    fn peek(&self) -> &TokenFull {
        self.toks.get(self.pos).unwrap_or(&TokenFull::Eof)
    }

    fn bump(&mut self) -> TokenFull {
        let t = self.peek().clone();
        if !matches!(t, TokenFull::Eof) {
            self.pos += 1;
        }
        t
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), TokenFull::Newline { .. }) {
            self.pos += 1;
        }
    }

    fn expect_newline(&mut self) -> Result<(), ErrorFrontend> {
        match self.peek() {
            TokenFull::Newline { .. } => {
                self.pos += 1;
                Ok(())
            }
            TokenFull::Eof => Err(ErrorFrontend::FinInesperado),
            other => Err(ErrorFrontend::TokenInesperado {
                esperado: "salto de línea".into(),
                encontrado: desc(other),
                span: span_of(other),
            }),
        }
    }

    fn expect_indent(&mut self) -> Result<(), ErrorFrontend> {
        match self.peek() {
            TokenFull::Indent { .. } => {
                self.pos += 1;
                Ok(())
            }
            TokenFull::Eof => Err(ErrorFrontend::FinInesperado),
            other => Err(ErrorFrontend::TokenInesperado {
                esperado: "bloque indentado".into(),
                encontrado: desc(other),
                span: span_of(other),
            }),
        }
    }

    fn expect_dedent(&mut self) -> Result<(), ErrorFrontend> {
        match self.peek() {
            TokenFull::Dedent { .. } => {
                self.pos += 1;
                Ok(())
            }
            TokenFull::Eof => Err(ErrorFrontend::FinInesperado),
            other => Err(ErrorFrontend::TokenInesperado {
                esperado: "fin de bloque (dedent)".into(),
                encontrado: desc(other),
                span: span_of(other),
            }),
        }
    }

    fn expect_kw(&mut self, kw: Token) -> Result<Span, ErrorFrontend> {
        match self.peek() {
            TokenFull::Tok(s) if s.tok == kw => {
                let sp = s.span.clone();
                self.pos += 1;
                Ok(sp)
            }
            TokenFull::Eof => Err(ErrorFrontend::FinInesperado),
            other => Err(ErrorFrontend::TokenInesperado {
                esperado: format!("{kw:?}"),
                encontrado: desc(other),
                span: span_of(other),
            }),
        }
    }

    fn expect_sym(&mut self, sym: Token) -> Result<Span, ErrorFrontend> {
        self.expect_kw(sym)
    }

    fn peek_is_kw(&self, kw: &Token) -> bool {
        matches!(self.peek(), TokenFull::Tok(s) if &s.tok == kw)
    }

    fn peek_is_sym(&self, sym: &Token) -> bool {
        self.peek_is_kw(sym)
    }
}

fn desc(t: &TokenFull) -> String {
    match t {
        TokenFull::Tok(s) => format!("{:?}", s.tok),
        TokenFull::Indent { .. } => "INDENT".into(),
        TokenFull::Dedent { .. } => "DEDENT".into(),
        TokenFull::Newline { .. } => "salto de línea".into(),
        TokenFull::Eof => "fin de archivo".into(),
    }
}

fn span_of(t: &TokenFull) -> SourceSpan {
    match t {
        TokenFull::Tok(s) => (s.span.start, s.span.len()).into(),
        TokenFull::Indent { span } | TokenFull::Dedent { span } | TokenFull::Newline { span } => {
            (span.start, span.len().max(1)).into()
        }
        TokenFull::Eof => (0, 0).into(),
    }
}

// =============================================================================
// Entry point
// =============================================================================

/// Parsea tokens → AST.
pub fn parsear(toks: &[TokenFull]) -> Result<Program, ErrorFrontend> {
    let mut p = P::new(toks);
    p.skip_newlines();
    let mut defs = Vec::new();
    while !matches!(p.peek(), TokenFull::Eof) {
        // Saltar newlines sueltos entre declaraciones
        p.skip_newlines();
        if matches!(p.peek(), TokenFull::Eof) {
            break;
        }
        // Saltar dedents huérfanos (tolerancia)
        if matches!(p.peek(), TokenFull::Dedent { .. }) {
            p.pos += 1;
            continue;
        }
        defs.push(parse_declaracion(&mut p)?);
        p.skip_newlines();
    }
    Ok(Program { defs })
}

// =============================================================================
// Declaraciones
// =============================================================================

fn parse_declaracion(p: &mut P) -> Result<Decl, ErrorFrontend> {
    // Todas empiezan con "definir"
    p.expect_kw(Token::Definir)?;
    if p.peek_is_kw(&Token::Tarea) {
        Ok(Decl::Tarea(parse_tarea(p)?))
    } else if p.peek_is_kw(&Token::Estructura) {
        Ok(Decl::Estructura(parse_estructura(p)?))
    } else if p.peek_is_kw(&Token::Variante) {
        Ok(Decl::Variante(parse_variante(p)?))
    } else {
        Err(match p.peek() {
            TokenFull::Eof => ErrorFrontend::FinInesperado,
            other => ErrorFrontend::TokenInesperado {
                esperado: "tarea | estructura | variante".into(),
                encontrado: desc(other),
                span: span_of(other),
            },
        })
    }
}

// DefinicionTarea ::= "definir" "tarea" Ident "(" [Params] ")" ["->" Tipo] ":" NL INDENT Cuerpo DEDENT
// (el "definir" ya fue consumido por parse_declaracion)
fn parse_tarea(p: &mut P) -> Result<TareaDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Tarea)?.start;
    let nombre = parse_ident(p)?;
    p.expect_sym(Token::LParen)?;
    let mut params = Vec::new();
    if !p.peek_is_sym(&Token::RParen) {
        loop {
            let n = parse_ident(p)?;
            p.expect_sym(Token::Colon)?;
            let t = parse_tipo(p)?;
            params.push(Param { nombre: n, tipo: t });
            if p.peek_is_sym(&Token::Comma) {
                p.bump();
            } else {
                break;
            }
        }
    }
    p.expect_sym(Token::RParen)?;

    let retorno = if p.peek_is_sym(&Token::Arrow) {
        p.bump();
        Some(parse_tipo(p)?)
    } else {
        None
    };

    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;
    let cuerpo = parse_cuerpo(p)?;
    p.expect_dedent()?;

    let fin = cuerpo_fin(&cuerpo);
    Ok(TareaDecl {
        nombre,
        params,
        retorno,
        cuerpo,
        span: inicio..fin,
    })
}

fn cuerpo_fin(c: &Cuerpo) -> usize {
    // Aproximación: usamos 0 si vacío (el span exacto se refina en M3 con tracking)
    let _ = c;
    0
}

// Cuerpo ::= [Sentencia { NL Sentencia }] [ NL BloqueRestricciones ]
fn parse_cuerpo(p: &mut P) -> Result<Cuerpo, ErrorFrontend> {
    let mut stmts = Vec::new();
    let mut restricciones = Vec::new();
    let mut saw_stmt = false;
    loop {
        p.skip_newlines();
        match p.peek() {
            TokenFull::Dedent { .. } | TokenFull::Eof => break,
            TokenFull::Tok(s) if s.tok == Token::Bajo => {
                restricciones = parse_restricciones(p)?;
                p.skip_newlines();
                break;
            }
            _ => {
                stmts.push(parse_sentencia(p)?);
                saw_stmt = true;
            }
        }
    }
    // Permitir cuerpo vacío (0 statements) si no hay restricciones
    if !saw_stmt && restricciones.is_empty() {
        // Cuerpo vacío válido - solo INDENT/DEDENT
    }
    Ok(Cuerpo {
        stmts,
        restricciones,
    })
}

// BloqueRestricciones ::= "bajo" "restricciones" ":" NL INDENT Restriccion { NL Restriccion } DEDENT
fn parse_restricciones(p: &mut P) -> Result<Vec<Restriccion>, ErrorFrontend> {
    p.expect_kw(Token::Bajo)?;
    p.expect_kw(Token::Restricciones)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;
    let mut out = Vec::new();
    loop {
        p.skip_newlines();
        if matches!(p.peek(), TokenFull::Dedent { .. } | TokenFull::Eof) {
            break;
        }
        // clave: valor (ambos como texto crudo del token)
        let (clave, cspan) = match p.peek() {
            TokenFull::Tok(s) => (format!("{:?}", s.tok).to_lowercase(), s.span.clone()),
            other => {
                return Err(ErrorFrontend::TokenInesperado {
                    esperado: "clave de restricción".into(),
                    encontrado: desc(other),
                    span: span_of(other),
                })
            }
        };
        // La clave real es el nombre del identificador/keyword en minúsculas.
        // Para Ident tomamos el slice original vía Debug; mejor: extraer nombre.
        let clave_nombre = match p.peek() {
            TokenFull::Tok(s) => token_nombre(&s.tok),
            _ => clave,
        };
        let _ = cspan;
        p.bump();
        p.expect_sym(Token::Colon)?;
        let valor = match p.peek() {
            TokenFull::Tok(s) => {
                let v = token_nombre(&s.tok);
                p.bump();
                v
            }
            other => {
                return Err(ErrorFrontend::TokenInesperado {
                    esperado: "valor de restricción".into(),
                    encontrado: desc(other),
                    span: span_of(other),
                })
            }
        };
        out.push(Restriccion {
            clave: clave_nombre,
            valor,
            span: 0..0,
        });
    }
    p.expect_dedent()?;
    Ok(out)
}

/// Nombre canónico de un token para claves/valores de restricciones.
fn token_nombre(t: &Token) -> String {
    match t {
        Token::Ident(n) => n.clone(),
        _ => format!("{t:?}").to_lowercase(),
    }
}

// Estructura ::= "definir" "estructura" Ident ":" NL INDENT Campo { NL Campo } DEDENT
fn parse_estructura(p: &mut P) -> Result<EstructuraDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Estructura)?.start;
    let nombre = parse_ident(p)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;
    let mut campos = Vec::new();
    loop {
        p.skip_newlines();
        if matches!(p.peek(), TokenFull::Dedent { .. } | TokenFull::Eof) {
            break;
        }
        let n = parse_ident(p)?;
        // "como" es keyword opcional en CNL (id como Tipo | id: Tipo)
        if p.peek_is_kw(&Token::Como) || p.peek_is_sym(&Token::Colon) {
            p.bump();
        } else {
            return Err(match p.peek() {
                TokenFull::Eof => ErrorFrontend::FinInesperado,
                other => ErrorFrontend::TokenInesperado {
                    esperado: "`como` o `:`".into(),
                    encontrado: desc(other),
                    span: span_of(other),
                },
            });
        }
        let t = parse_tipo(p)?;
        campos.push(Campo { nombre: n, tipo: t });
    }
    p.expect_dedent()?;
    Ok(EstructuraDecl {
        nombre,
        campos,
        span: inicio..inicio,
    })
}

// Variante ::= "definir" "variante" Ident ":" NL INDENT ("opcion" Ident ["con" "datos" "(" Campos ")"] { NL ... }) DEDENT
fn parse_variante(p: &mut P) -> Result<VarianteDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Variante)?.start;
    let nombre = parse_ident(p)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;
    let mut casos = Vec::new();
    loop {
        p.skip_newlines();
        if matches!(p.peek(), TokenFull::Dedent { .. } | TokenFull::Eof) {
            break;
        }
        p.expect_kw(Token::Opcion)?;
        let cn = parse_ident(p)?;
        let mut datos = Vec::new();
        if p.peek_is_kw(&Token::Con) {
            p.bump();
            p.expect_kw(Token::Datos)?;
            p.expect_sym(Token::LParen)?;
            if !p.peek_is_sym(&Token::RParen) {
                loop {
                    let fn_ = parse_ident(p)?;
                    p.expect_sym(Token::Colon)?;
                    let ft = parse_tipo(p)?;
                    datos.push(Campo {
                        nombre: fn_,
                        tipo: ft,
                    });
                    if p.peek_is_sym(&Token::Comma) {
                        p.bump();
                    } else {
                        break;
                    }
                }
            }
            p.expect_sym(Token::RParen)?;
        }
        casos.push(CasoVariante { nombre: cn, datos });
    }
    p.expect_dedent()?;
    Ok(VarianteDecl {
        nombre,
        casos,
        span: inicio..inicio,
    })
}

// =============================================================================
// Sentencias
// =============================================================================

fn parse_sentencia(p: &mut P) -> Result<Stmt, ErrorFrontend> {
    match p.peek() {
        TokenFull::Tok(s) => match &s.tok {
            Token::Verificar => {
                let sp = s.span.clone();
                p.bump();
                p.expect_kw(Token::Que)?;
                let c = parse_expr_logica(p)?;
                Ok(Stmt::Verificar {
                    cond: c,
                    span: sp.start..sp.start,
                })
            }
            Token::Asumir => {
                let sp = s.span.clone();
                p.bump();
                let c = parse_expr_logica(p)?;
                Ok(Stmt::Asumir {
                    cond: c,
                    span: sp.start..sp.start,
                })
            }
            Token::Demostrar => {
                let sp = s.span.clone();
                p.bump();
                let c = parse_expr_logica(p)?;
                Ok(Stmt::Demostrar {
                    cond: c,
                    span: sp.start..sp.start,
                })
            }
            Token::Let => {
                let sp = s.span.clone();
                p.bump();
                let mutable = if p.peek_is_kw(&Token::Mut) {
                    p.bump();
                    true
                } else {
                    false
                };
                let n = parse_ident(p)?;
                let tipo = if p.peek_is_sym(&Token::Colon) {
                    p.bump();
                    Some(parse_tipo(p)?)
                } else {
                    None
                };
                p.expect_sym(Token::Eq)?;
                let e = parse_expr(p)?;
                Ok(Stmt::Asignar {
                    nombre: n,
                    tipo,
                    expr: e,
                    mutable,
                    span: sp.start..sp.start,
                })
            }
            Token::Retornar => {
                let sp = s.span.clone();
                p.bump();
                // `retornar` puede ir sin expresión (retorno Void)
                let e = match p.peek() {
                    TokenFull::Newline { .. } | TokenFull::Dedent { .. } | TokenFull::Eof => None,
                    _ => Some(parse_expr(p)?),
                };
                Ok(Stmt::Retornar {
                    expr: e,
                    span: sp.start..sp.start,
                })
            }
            _ => Err(ErrorFrontend::TokenInesperado {
                esperado: "verificar | asumir | demostrar | let | retornar".into(),
                encontrado: desc(p.peek()),
                span: span_of(p.peek()),
            }),
        },
        other => Err(match other {
            TokenFull::Eof => ErrorFrontend::FinInesperado,
            _ => ErrorFrontend::TokenInesperado {
                esperado: "sentencia".into(),
                encontrado: desc(other),
                span: span_of(other),
            },
        }),
    }
}

// =============================================================================
// Expresiones (precedencia: or > and > cmp > add > mul > unary > primary)
// =============================================================================

fn parse_expr(p: &mut P) -> Result<Expr, ErrorFrontend> {
    parse_add(p)
}

fn parse_expr_logica(p: &mut P) -> Result<Expr, ErrorFrontend> {
    parse_cmp(p)
}

fn parse_cmp(p: &mut P) -> Result<Expr, ErrorFrontend> {
    let lhs = parse_add(p)?;
    // Operadores relacionales CNL y simbólicos
    let op = if p.peek_is_kw(&Token::Sea) {
        p.bump();
        if p.peek_is_kw(&Token::Mayor) {
            p.bump();
            p.expect_kw(Token::A)?;
            Some(BinOp::Gt)
        } else if p.peek_is_kw(&Token::Menor) {
            p.bump();
            p.expect_kw(Token::A)?;
            Some(BinOp::Lt)
        } else if p.peek_is_kw(&Token::Igual) {
            p.bump();
            p.expect_kw(Token::A)?;
            Some(BinOp::Eq)
        } else {
            None
        }
    } else if p.peek_is_sym(&Token::Gt) {
        p.bump();
        Some(BinOp::Gt)
    } else if p.peek_is_sym(&Token::Lt) {
        p.bump();
        Some(BinOp::Lt)
    } else if p.peek_is_sym(&Token::EqEq) {
        p.bump();
        Some(BinOp::Eq)
    } else if p.peek_is_sym(&Token::NotEq) {
        p.bump();
        Some(BinOp::Ne)
    } else if p.peek_is_sym(&Token::Ge) {
        p.bump();
        Some(BinOp::Ge)
    } else if p.peek_is_sym(&Token::Le) {
        p.bump();
        Some(BinOp::Le)
    } else {
        None
    };
    if let Some(op) = op {
        let rhs = parse_add(p)?;
        let span = expr_span(&lhs);
        Ok(Expr::BinOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span,
        })
    } else {
        Ok(lhs)
    }
}

fn parse_add(p: &mut P) -> Result<Expr, ErrorFrontend> {
    let mut lhs = parse_mul(p)?;
    loop {
        let op = if p.peek_is_sym(&Token::Plus) {
            BinOp::Add
        } else if p.peek_is_sym(&Token::Minus) {
            BinOp::Sub
        } else {
            break;
        };
        p.bump();
        let rhs = parse_mul(p)?;
        let span = expr_span(&lhs);
        lhs = Expr::BinOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span,
        };
    }
    Ok(lhs)
}

fn parse_mul(p: &mut P) -> Result<Expr, ErrorFrontend> {
    let mut lhs = parse_primary(p)?;
    loop {
        let op = if p.peek_is_sym(&Token::Star) {
            BinOp::Mul
        } else if p.peek_is_sym(&Token::Slash) {
            BinOp::Div
        } else {
            break;
        };
        p.bump();
        let rhs = parse_primary(p)?;
        let span = expr_span(&lhs);
        lhs = Expr::BinOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span,
        };
    }
    Ok(lhs)
}

fn parse_primary(p: &mut P) -> Result<Expr, ErrorFrontend> {
    match p.peek().clone() {
        TokenFull::Tok(s) => match s.tok {
            Token::Minus => {
                // Unary minus: -expr
                p.bump();
                let rhs = parse_primary(p)?;
                let span = expr_span(&rhs);
                Ok(Expr::BinOp {
                    op: BinOp::Sub,
                    lhs: Box::new(Expr::Lit(Literal::Entero(0))),
                    rhs: Box::new(rhs),
                    span,
                })
            }
            Token::Entero(n) => {
                p.bump();
                Ok(Expr::Lit(Literal::Entero(n)))
            }
            Token::Flotante(f) => {
                p.bump();
                Ok(Expr::Lit(Literal::Flotante(f)))
            }
            Token::Texto(st) => {
                p.bump();
                Ok(Expr::Lit(Literal::Texto(st)))
            }
            Token::Verdadero => {
                p.bump();
                Ok(Expr::Lit(Literal::Booleano(true)))
            }
            Token::Falso => {
                p.bump();
                Ok(Expr::Lit(Literal::Booleano(false)))
            }
            Token::Ident(nombre) => {
                let sp = s.span.clone();
                let n = nombre.clone();
                p.bump();
                let base = Ident {
                    nombre: n,
                    span: sp.clone(),
                };
                if p.peek_is_sym(&Token::Dot) {
                    p.bump();
                    let prop = parse_ident(p)?;
                    Ok(Expr::AccesoProp {
                        base: Box::new(Expr::Var(base)),
                        prop,
                        span: sp,
                    })
                } else {
                    Ok(Expr::Var(base))
                }
            }
            Token::LParen => {
                p.bump();
                let e = parse_expr(p)?;
                p.expect_sym(Token::RParen)?;
                Ok(e)
            }
            _ => Err(ErrorFrontend::TokenInesperado {
                esperado: "expresión (literal, identificador, `(`)".into(),
                encontrado: desc(p.peek()),
                span: span_of(p.peek()),
            }),
        },
        _ => Err(match p.peek() {
            TokenFull::Eof => ErrorFrontend::FinInesperado,
            other => ErrorFrontend::TokenInesperado {
                esperado: "expresión".into(),
                encontrado: desc(other),
                span: span_of(other),
            },
        }),
    }
}

fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Var(i) => i.span.clone(),
        Expr::Lit(_) => 0..0,
        Expr::AccesoProp { span, .. } => span.clone(),
        Expr::BinOp { span, .. } => span.clone(),
    }
}

// =============================================================================
// Identificadores y tipos
// =============================================================================

fn parse_ident(p: &mut P) -> Result<Ident, ErrorFrontend> {
    match p.peek() {
        TokenFull::Tok(s) => match &s.tok {
            Token::Ident(nombre) => {
                let sp = s.span.clone();
                let n = nombre.clone();
                p.bump();
                Ok(Ident {
                    nombre: n,
                    span: sp,
                })
            }
            _ => Err(ErrorFrontend::TokenInesperado {
                esperado: "identificador".into(),
                encontrado: desc(p.peek()),
                span: span_of(p.peek()),
            }),
        },
        TokenFull::Eof => Err(ErrorFrontend::FinInesperado),
        other => Err(ErrorFrontend::TokenInesperado {
            esperado: "identificador".into(),
            encontrado: desc(other),
            span: span_of(other),
        }),
    }
}

fn parse_tipo(p: &mut P) -> Result<TipoDato, ErrorFrontend> {
    match p.peek().clone() {
        TokenFull::Tok(s) => match s.tok {
            Token::TEntero64 => {
                p.bump();
                Ok(TipoDato::Entero64)
            }
            Token::TFlotante64 => {
                p.bump();
                Ok(TipoDato::Flotante64)
            }
            Token::TBooleano => {
                p.bump();
                Ok(TipoDato::Booleano)
            }
            Token::TTexto => {
                p.bump();
                Ok(TipoDato::Texto)
            }
            Token::TCapacidadHardware => {
                p.bump();
                Ok(TipoDato::CapacidadHardware)
            }
            Token::TVoid => {
                p.bump();
                Ok(TipoDato::Void)
            }
            Token::TUSD => {
                p.bump();
                Ok(TipoDato::USD)
            }
            Token::TEUR => {
                p.bump();
                Ok(TipoDato::EUR)
            }
            Token::TLista => {
                p.bump();
                p.expect_kw(Token::De)?;
                Ok(TipoDato::Lista(Box::new(parse_tipo(p)?)))
            }
            Token::TMapa => {
                p.bump();
                p.expect_kw(Token::De)?;
                let k = parse_tipo(p)?;
                p.expect_kw(Token::A)?;
                Ok(TipoDato::Mapa(Box::new(k), Box::new(parse_tipo(p)?)))
            }
            Token::TConjunto => {
                p.bump();
                p.expect_kw(Token::De)?;
                Ok(TipoDato::Conjunto(Box::new(parse_tipo(p)?)))
            }
            Token::TTupla => {
                p.bump();
                // "de" es opcional antes de Tupla (CNL: "Tupla(...)" o "de Tupla(...)")
                if p.peek_is_kw(&Token::De) {
                    p.bump();
                }
                p.expect_sym(Token::LParen)?;
                let mut xs = Vec::new();
                if !p.peek_is_sym(&Token::RParen) {
                    loop {
                        xs.push(parse_tipo(p)?);
                        if p.peek_is_sym(&Token::Comma) {
                            p.bump();
                        } else {
                            break;
                        }
                    }
                }
                p.expect_sym(Token::RParen)?;
                Ok(TipoDato::Tupla(xs))
            }
            Token::Ident(_) => {
                let id = parse_ident(p)?;
                Ok(TipoDato::Nominal(id.nombre.clone()))
            }
            _ => Err(ErrorFrontend::TokenInesperado {
                esperado: "tipo (Entero64, Texto, ...)".into(),
                encontrado: desc(p.peek()),
                span: span_of(p.peek()),
            }),
        },
        _ => Err(match p.peek() {
            TokenFull::Eof => ErrorFrontend::FinInesperado,
            other => ErrorFrontend::TokenInesperado {
                esperado: "tipo".into(),
                encontrado: desc(other),
                span: span_of(other),
            },
        }),
    }
}

// =============================================================================
// Tests M2
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lexear;

    fn parse(src: &str) -> Result<Program, ErrorFrontend> {
        parsear(&lexear(src))
    }

    #[test]
    fn tarea_minima() {
        let p = parse("definir tarea f():\n    retornar 1\n").unwrap();
        assert_eq!(p.defs.len(), 1);
        match &p.defs[0] {
            Decl::Tarea(t) => assert_eq!(t.retorno, None),
            _ => panic!("esperaba tarea"),
        }
    }

    #[test]
    fn tarea_con_params_y_retorno() {
        let p = parse(
            "definir tarea sumar(x: Entero64, y: Entero64) -> Entero64:\n    retornar x + y\n",
        )
        .unwrap();
        match &p.defs[0] {
            Decl::Tarea(t) => {
                assert_eq!(t.params.len(), 2);
                assert_eq!(t.retorno, Some(TipoDato::Entero64));
            }
            _ => panic!("esperaba tarea"),
        }
    }

    #[test]
    fn verificar_asumir_demostrar() {
        let p = parse("definir tarea f(x: Entero64) -> Entero64:\n    asumir x > 0\n    verificar que x > 0\n    demostrar x > 0\n    retornar x\n").unwrap();
        match &p.defs[0] {
            Decl::Tarea(t) => assert_eq!(t.cuerpo.stmts.len(), 4),
            _ => panic!("esperaba tarea"),
        }
    }

    #[test]
    fn let_con_tipo_y_mut() {
        let p = parse("definir tarea f() -> Entero64:\n    let x: Entero64 = 1\n    let mut y = 2\n    retornar x\n").unwrap();
        match &p.defs[0] {
            Decl::Tarea(t) => assert_eq!(t.cuerpo.stmts.len(), 3),
            _ => panic!("esperaba tarea"),
        }
    }

    #[test]
    fn bloque_restricciones() {
        let p = parse("definir tarea f() -> Entero64:\n    retornar 1\n    bajo restricciones:\n        gestion_memoria: arena\n").unwrap();
        match &p.defs[0] {
            Decl::Tarea(t) => assert!(!t.cuerpo.restricciones.is_empty()),
            _ => panic!("esperaba tarea"),
        }
    }

    #[test]
    fn estructura_basica() {
        let p = parse("definir estructura Cliente:\n    id como Entero64\n    nombre como Texto\n")
            .unwrap();
        match &p.defs[0] {
            Decl::Estructura(e) => assert_eq!(e.campos.len(), 2),
            _ => panic!("esperaba estructura"),
        }
    }

    #[test]
    fn variante_basica() {
        let p = parse("definir variante R:\n    opcion Ok con datos (id: Texto)\n    opcion Err\n")
            .unwrap();
        match &p.defs[0] {
            Decl::Variante(v) => {
                assert_eq!(v.casos.len(), 2);
                assert_eq!(v.casos[0].datos.len(), 1);
            }
            _ => panic!("esperaba variante"),
        }
    }

    #[test]
    fn error_falta_definir() {
        let r = parse("tarea f():\n    retornar 1\n");
        assert!(r.is_err());
    }

    #[test]
    fn expresion_precedencia() {
        // 1 + 2 * 3 debe parsear como 1 + (2*3)
        let p = parse("definir tarea f() -> Entero64:\n    retornar 1 + 2 * 3\n").unwrap();
        match &p.defs[0] {
            Decl::Tarea(t) => match &t.cuerpo.stmts[0] {
                Stmt::Retornar {
                    expr: Some(Expr::BinOp { op: BinOp::Add, .. }),
                    ..
                } => {}
                other => panic!("esperaba Add en raíz, got {other:?}"),
            },
            _ => panic!("esperaba tarea"),
        }
    }
}
