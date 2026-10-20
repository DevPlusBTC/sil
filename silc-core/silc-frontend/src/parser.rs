//! Parser CNL: descenso recursivo sobre TokenFull → AST tipado.
//!
//! Cubre el núcleo EBNF: Programa, DefinicionTarea, Cuerpo, Sentencias
//! (verificar/asumir/demostrar/let/retornar), Expresiones con precedencia,
//! BloqueRestricciones, Estructura, Variante, Servicio, Modelo,
//! MemoriaPersistente, Capacidad, Modulo y Migracion.
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

    /// Offset final del último token consumido (fin del span en curso).
    fn fin(&self) -> usize {
        if self.pos == 0 {
            return 0;
        }
        fin_de(&self.toks[self.pos - 1])
    }
}

/// Offset final de un token estructural o léxico.
fn fin_de(t: &TokenFull) -> usize {
    match t {
        TokenFull::Tok(s) => s.span.end,
        TokenFull::Indent { span } | TokenFull::Dedent { span } | TokenFull::Newline { span } => {
            span.end
        }
        TokenFull::Eof => 0,
    }
}

/// Offset inicial del token actual (inicio del span de la expresión que empieza).
fn inicio_expr(p: &P) -> usize {
    match p.peek() {
        TokenFull::Tok(s) => s.span.start,
        TokenFull::Indent { span } | TokenFull::Dedent { span } | TokenFull::Newline { span } => {
            span.start
        }
        TokenFull::Eof => 0,
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
    // `modulo` y `migrar` son declaraciones que no llevan el prefijo "definir".
    if p.peek_is_kw(&Token::Modulo) {
        return Ok(Decl::Modulo(parse_modulo(p)?));
    }
    if p.peek_is_kw(&Token::Migrar) {
        return Ok(Decl::Migracion(parse_migracion(p)?));
    }

    p.expect_kw(Token::Definir)?;
    if p.peek_is_kw(&Token::Tarea) {
        Ok(Decl::Tarea(parse_tarea(p)?))
    } else if p.peek_is_kw(&Token::Estructura) {
        Ok(Decl::Estructura(parse_estructura(p)?))
    } else if p.peek_is_kw(&Token::Variante) {
        Ok(Decl::Variante(parse_variante(p)?))
    } else if p.peek_is_kw(&Token::Servicio) {
        Ok(Decl::Servicio(parse_servicio(p)?))
    } else if p.peek_is_kw(&Token::Modelo) {
        Ok(Decl::Modelo(parse_modelo(p)?))
    } else if p.peek_is_kw(&Token::MemoriaPersistente) {
        Ok(Decl::MemoriaPersistente(parse_memoria_persistente(p)?))
    } else if p.peek_is_kw(&Token::Capacidad) {
        Ok(Decl::Capacidad(parse_capacidad(p)?))
    } else {
        Err(match p.peek() {
            TokenFull::Eof => ErrorFrontend::FinInesperado,
            other => ErrorFrontend::TokenInesperado {
                esperado: "tarea | estructura | variante | servicio | modelo | memoria_persistente | capacidad".into(),
                encontrado: desc(other),
                span: span_of(other),
            },
        })
    }
}

// ListaParametros ::= Ident ":" Tipo { "," Ident ":" Tipo }
fn parse_params(p: &mut P) -> Result<Vec<Param>, ErrorFrontend> {
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
    Ok(params)
}

// DefinicionTarea ::= "definir" "tarea" Ident "(" [Params] ")" ["->" Tipo] ":" NL INDENT Cuerpo DEDENT
// (el "definir" ya fue consumido por parse_declaracion)
fn parse_tarea(p: &mut P) -> Result<TareaDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Tarea)?.start;
    let nombre = parse_ident(p)?;
    p.expect_sym(Token::LParen)?;
    let params = parse_params(p)?;
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

    let fin = p.fin();
    Ok(TareaDecl {
        nombre,
        params,
        retorno,
        cuerpo,
        span: inicio..fin,
    })
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
        // clave: valor (identificador o palabra clave del lenguaje)
        let (clave_nombre, ini) = match p.peek() {
            TokenFull::Tok(s) => match palabra_restriccion(&s.tok) {
                Some(n) => (n, s.span.start),
                None => {
                    return Err(ErrorFrontend::TokenInesperado {
                        esperado: "clave de restricción (identificador)".into(),
                        encontrado: desc(p.peek()),
                        span: span_of(p.peek()),
                    })
                }
            },
            other => {
                return Err(ErrorFrontend::TokenInesperado {
                    esperado: "clave de restricción".into(),
                    encontrado: desc(other),
                    span: span_of(other),
                })
            }
        };
        p.bump();
        p.expect_sym(Token::Colon)?;
        let valor = match p.peek() {
            TokenFull::Tok(s) => match palabra_restriccion(&s.tok) {
                Some(v) => {
                    p.bump();
                    v
                }
                None => {
                    return Err(ErrorFrontend::TokenInesperado {
                        esperado: "valor de restricción (identificador)".into(),
                        encontrado: desc(p.peek()),
                        span: span_of(p.peek()),
                    })
                }
            },
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
            span: ini..p.fin(),
        });
    }
    p.expect_dedent()?;
    Ok(out)
}

/// Palabra aceptada como clave/valor de una restricción.
///
/// Los valores del lenguaje (`arena`, `atomica`, `asincrona`, `en_memoria`,
/// `cero_pausas`) son palabras clave del lexer, no identificadores.
fn palabra_restriccion(t: &Token) -> Option<String> {
    match t {
        Token::Ident(n) => Some(n.clone()),
        Token::Arena => Some("arena".into()),
        Token::Atomica => Some("atomica".into()),
        Token::Asincrona => Some("asincrona".into()),
        Token::EnMemoria => Some("en_memoria".into()),
        Token::CeroPausas => Some("cero_pausas".into()),
        _ => None,
    }
}

// CamposBlque ::= ":" NL INDENT Campo { NL Campo } DEDENT
fn parse_campos(p: &mut P) -> Result<Vec<Campo>, ErrorFrontend> {
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
    Ok(campos)
}

// Estructura ::= "definir" "estructura" Ident CamposBlque
fn parse_estructura(p: &mut P) -> Result<EstructuraDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Estructura)?.start;
    let nombre = parse_ident(p)?;
    let campos = parse_campos(p)?;
    Ok(EstructuraDecl {
        nombre,
        campos,
        span: inicio..p.fin(),
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
        span: inicio..p.fin(),
    })
}

// =============================================================================
// Declaraciones de servicio, modelo, memoria persistente, capacidad, módulo y migración
// =============================================================================

// Servicio ::= "definir" "servicio" Ident ":" NL INDENT
//              [ ConfiguracionRed ] { ManejadorEvento | DefinicionTarea }
//              [ BloqueRestricciones ] DEDENT
fn parse_servicio(p: &mut P) -> Result<ServicioDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Servicio)?.start;
    let nombre = parse_ident(p)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;

    let mut red = None;
    let mut manejadores = Vec::new();
    let mut tareas = Vec::new();
    let mut restricciones = Vec::new();

    loop {
        p.skip_newlines();
        match p.peek() {
            TokenFull::Dedent { .. } | TokenFull::Eof => break,
            TokenFull::Tok(s) if s.tok == Token::Escuchar => {
                red = Some(parse_configuracion_red(p)?);
            }
            TokenFull::Tok(s) if s.tok == Token::Cuando => {
                manejadores.push(parse_manejador_evento(p)?);
            }
            TokenFull::Tok(s) if s.tok == Token::Definir => {
                p.bump();
                tareas.push(parse_tarea(p)?);
            }
            TokenFull::Tok(s) if s.tok == Token::Bajo => {
                restricciones = parse_restricciones(p)?;
                p.skip_newlines();
                break;
            }
            other => {
                return Err(ErrorFrontend::TokenInesperado {
                    esperado: "escuchar | cuando | definir tarea | bajo restricciones".into(),
                    encontrado: desc(other),
                    span: span_of(other),
                })
            }
        }
    }
    p.expect_dedent()?;
    Ok(ServicioDecl {
        nombre,
        red,
        manejadores,
        tareas,
        restricciones,
        span: inicio..p.fin(),
    })
}

// ConfiguracionRed ::= "escuchar" "en" "puerto" NUM [ "con" "protocolo" Ident ] NL
fn parse_configuracion_red(p: &mut P) -> Result<ConfiguracionRed, ErrorFrontend> {
    p.expect_kw(Token::Escuchar)?;
    p.expect_kw(Token::En)?;
    p.expect_kw(Token::Puerto)?;
    let puerto = parse_entero_u64(p)?;
    let protocolo = if p.peek_is_kw(&Token::Con) {
        p.bump();
        p.expect_kw(Token::Protocolo)?;
        Some(parse_ident(p)?)
    } else {
        None
    };
    p.expect_newline()?;
    Ok(ConfiguracionRed { puerto, protocolo })
}

// ManejadorEvento ::= "cuando" "llegue" "evento" Ident "(" [Params] ")" ":" NL INDENT Cuerpo DEDENT
fn parse_manejador_evento(p: &mut P) -> Result<ManejadorEvento, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Cuando)?.start;
    p.expect_kw(Token::Llegue)?;
    p.expect_kw(Token::Evento)?;
    let evento = parse_ident(p)?;
    p.expect_sym(Token::LParen)?;
    let params = parse_params(p)?;
    p.expect_sym(Token::RParen)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;
    let cuerpo = parse_cuerpo(p)?;
    p.expect_dedent()?;
    Ok(ManejadorEvento {
        evento,
        params,
        cuerpo,
        span: inicio..p.fin(),
    })
}

// Modelo ::= "definir" "modelo" Ident CamposBlque
fn parse_modelo(p: &mut P) -> Result<ModeloDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Modelo)?.start;
    let nombre = parse_ident(p)?;
    let campos = parse_campos(p)?;
    Ok(ModeloDecl {
        nombre,
        campos,
        span: inicio..p.fin(),
    })
}

// MemoriaPersistente ::= "definir" "memoria_persistente" Ident CamposBlque
fn parse_memoria_persistente(p: &mut P) -> Result<MemoriaPersistenteDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::MemoriaPersistente)?.start;
    let nombre = parse_ident(p)?;
    let campos = parse_campos(p)?;
    Ok(MemoriaPersistenteDecl {
        nombre,
        campos,
        span: inicio..p.fin(),
    })
}

// Capacidad ::= "definir" "capacidad" Ident ":" NL INDENT
//               "permitir" "conectar" "a" STRING "en" "puerto" NUM NL
//               "limite_ancho_banda" ":" STRING NL DEDENT
fn parse_capacidad(p: &mut P) -> Result<CapacidadDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Capacidad)?.start;
    let nombre = parse_ident(p)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;

    p.expect_kw(Token::Permitir)?;
    p.expect_kw(Token::Conectar)?;
    p.expect_kw(Token::A)?;
    let recurso = parse_texto_lit(p)?;
    p.expect_kw(Token::En)?;
    p.expect_kw(Token::Puerto)?;
    let puerto = parse_entero_u64(p)?;
    p.expect_newline()?;

    p.skip_newlines();
    let clave = parse_ident(p)?;
    if clave.nombre != "limite_ancho_banda" {
        return Err(ErrorFrontend::TokenInesperado {
            esperado: "limite_ancho_banda".into(),
            encontrado: clave.nombre,
            span: (clave.span.start, clave.span.len()).into(),
        });
    }
    p.expect_sym(Token::Colon)?;
    let limite_ancho_banda = parse_texto_lit(p)?;
    p.skip_newlines();
    p.expect_dedent()?;

    Ok(CapacidadDecl {
        nombre,
        recurso,
        puerto,
        limite_ancho_banda,
        span: inicio..p.fin(),
    })
}

// Modulo ::= "modulo" Ident ":" NL INDENT { "exportar" Clase Ident NL } DEDENT
// (a diferencia del resto de declaraciones, no lleva el prefijo "definir")
fn parse_modulo(p: &mut P) -> Result<ModuloDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Modulo)?.start;
    let nombre = parse_ident(p)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;

    let mut exportaciones = Vec::new();
    loop {
        p.skip_newlines();
        if matches!(p.peek(), TokenFull::Dedent { .. } | TokenFull::Eof) {
            break;
        }
        p.expect_kw(Token::Exportar)?;
        let clase = match p.peek() {
            TokenFull::Tok(s) => match &s.tok {
                Token::Servicio => ClaseExportada::Servicio,
                Token::Tarea => ClaseExportada::Tarea,
                Token::Estructura => ClaseExportada::Estructura,
                Token::Variante => ClaseExportada::Variante,
                Token::Capacidad => ClaseExportada::Capacidad,
                _ => {
                    return Err(ErrorFrontend::TokenInesperado {
                        esperado: "servicio | tarea | estructura | variante | capacidad".into(),
                        encontrado: desc(p.peek()),
                        span: span_of(p.peek()),
                    })
                }
            },
            other => {
                return Err(ErrorFrontend::TokenInesperado {
                    esperado: "servicio | tarea | estructura | variante | capacidad".into(),
                    encontrado: desc(other),
                    span: span_of(other),
                })
            }
        };
        p.bump();
        let nombre_exp = parse_ident(p)?;
        exportaciones.push(Exportacion {
            clase,
            nombre: nombre_exp,
        });
        p.expect_newline()?;
    }
    p.expect_dedent()?;
    Ok(ModuloDecl {
        nombre,
        exportaciones,
        span: inicio..p.fin(),
    })
}

// Migracion ::= "migrar" Ident "desde" Ident "a" Ident ":" NL INDENT
//               "al" "acceder_campo" STRING "en" "registro" Ident ":" NL INDENT
//               "retornar" "valor_predeterminado" Expresion NL
//               [ BloqueRestricciones ] DEDENT DEDENT
fn parse_migracion(p: &mut P) -> Result<MigracionDecl, ErrorFrontend> {
    let inicio = p.expect_kw(Token::Migrar)?.start;
    let origen = parse_ident(p)?;
    p.expect_kw(Token::Desde)?;
    let version_desde = parse_ident(p)?;
    p.expect_kw(Token::A)?;
    let version_a = parse_ident(p)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;

    p.skip_newlines();
    p.expect_kw(Token::Al)?;
    p.expect_kw(Token::AccederCampo)?;
    let campo = parse_texto_lit(p)?;
    p.expect_kw(Token::En)?;
    // `registro` no es palabra reservada del lexer: se valida como identificador literal.
    let registro_kw = parse_ident(p)?;
    if registro_kw.nombre != "registro" {
        return Err(ErrorFrontend::TokenInesperado {
            esperado: "registro".into(),
            encontrado: registro_kw.nombre,
            span: (registro_kw.span.start, registro_kw.span.len()).into(),
        });
    }
    let registro = parse_ident(p)?;
    p.expect_sym(Token::Colon)?;
    p.expect_newline()?;
    p.expect_indent()?;

    p.skip_newlines();
    p.expect_kw(Token::Retornar)?;
    p.expect_kw(Token::ValorPredeterminado)?;
    let valor_predeterminado = parse_expr(p)?;
    p.skip_newlines();

    let mut restricciones = Vec::new();
    if p.peek_is_kw(&Token::Bajo) {
        restricciones = parse_restricciones(p)?;
        p.skip_newlines();
    }
    p.expect_dedent()?;
    p.skip_newlines();
    p.expect_dedent()?;

    Ok(MigracionDecl {
        origen,
        version_desde,
        version_a,
        campo,
        registro,
        valor_predeterminado,
        restricciones,
        span: inicio..p.fin(),
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
                    span: sp.start..p.fin(),
                })
            }
            Token::Asumir => {
                let sp = s.span.clone();
                p.bump();
                // "asumir" puede ir seguido opcionalmente de "que" para consistencia con "verificar"
                if p.peek_is_kw(&Token::Que) {
                    p.bump();
                }
                let c = parse_expr_logica(p)?;
                Ok(Stmt::Asumir {
                    cond: c,
                    span: sp.start..p.fin(),
                })
            }
Token::Demostrar => {
                let sp = s.span.clone();
                p.bump();
                // "demostrar" puede ir seguido opcionalmente de "que" para consistencia con "verificar"
                if p.peek_is_kw(&Token::Que) {
                    p.bump();
                }
                let c = parse_expr_logica(p)?;
                Ok(Stmt::Demostrar {
                    cond: c,
                    span: sp.start..p.fin(),
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
                    span: sp.start..p.fin(),
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
                    span: sp.start..p.fin(),
                })
            }
            Token::Solicitar => {
                let sp = s.span.clone();
                p.bump();
                p.expect_kw(Token::Capacidad)?;
                let permiso = parse_permiso_cap(p)?;
                p.expect_kw(Token::Para)?;
                let recurso = parse_texto_lit(p)?;
                let ttl_ns = if p.peek_is_kw(&Token::Con) {
                    p.bump();
                    p.expect_kw(Token::Ttl)?;
                    Some(parse_entero_u64(p)?)
                } else {
                    None
                };
                let guardar_como = if p.peek_is_kw(&Token::Como) {
                    p.bump();
                    Some(parse_ident(p)?)
                } else {
                    None
                };
                Ok(Stmt::SolicitarCapacidad {
                    permiso,
                    recurso,
                    ttl_ns,
                    guardar_como,
                    span: sp.start..p.fin(),
                })
            }
            Token::Validar => {
                let sp = s.span.clone();
                p.bump();
                let token = parse_ident(p)?;
                Ok(Stmt::ValidarCapacidad {
                    token,
                    span: sp.start..p.fin(),
                })
            }
            Token::Enviar => {
                let sp = s.span.clone();
                p.bump();
                // enviar <expr> en <canal>  |  enviar <expr> en red "<dest>" con <cap>
                let datos = parse_expr(p)?;
                p.expect_kw(Token::En)?;
                if p.peek_is_kw(&Token::Red) {
                    p.bump();
                    let ruta = parse_texto_lit(p)?;
                    p.expect_kw(Token::Con)?;
                    let capacidad = parse_ident(p)?;
                    Ok(Stmt::OperarIO {
                        operacion: OpIOStmt::EnviarRed,
                        ruta,
                        datos: Some(datos),
                        capacidad,
                        guardar_como: None,
                        span: sp.start..p.fin(),
                    })
                } else {
                    let canal = parse_ident(p)?;
                    Ok(Stmt::EnviarCanal {
                        valor: datos,
                        canal,
                        span: sp.start..p.fin(),
                    })
                }
            }
            Token::Recibir => {
                let sp = s.span.clone();
                p.bump();
                // recibir de <canal> como <nombre> | recibir de red "<d>" con <cap> como <nombre>
                p.expect_kw(Token::De)?;
                if p.peek_is_kw(&Token::Red) {
                    // recibir de red "host:puerto" con <cap> como <nombre>
                    p.bump();
                    let ruta = parse_texto_lit(p)?;
                    p.expect_kw(Token::Con)?;
                    let capacidad = parse_ident(p)?;
                    p.expect_kw(Token::Como)?;
                    let guardar_como = parse_ident(p)?;
                    Ok(Stmt::OperarIO {
                        operacion: OpIOStmt::RecibirRed,
                        ruta,
                        datos: None,
                        capacidad,
                        guardar_como: Some(guardar_como),
                        span: sp.start..p.fin(),
                    })
                } else {
                    let canal = parse_ident(p)?;
                    p.expect_kw(Token::Como)?;
                    let guardar_como = parse_ident(p)?;
                    Ok(Stmt::RecibirCanal {
                        canal,
                        guardar_como,
                        span: sp.start..p.fin(),
                    })
                }
            }
            Token::Generar => {
                let sp = s.span.clone();
                p.bump();
                let tarea = parse_ident(p)?;
                Ok(Stmt::GenerarFibra {
                    tarea,
                    span: sp.start..p.fin(),
                })
            }
            Token::Crear => {
                let sp = s.span.clone();
                p.bump();
                p.expect_kw(Token::Canal)?;
                let capacidad = if p.peek_is_kw(&Token::Con) {
                    p.bump();
                    // `con capacidad <n>`
                    p.expect_kw(Token::Capacidad)?;
                    Some(parse_entero_u64(p)?)
                } else {
                    None
                };
                p.expect_kw(Token::Como)?;
                let guardar_como = parse_ident(p)?;
                Ok(Stmt::CrearCanal {
                    capacidad,
                    guardar_como,
                    span: sp.start..p.fin(),
                })
            }
            Token::Leer => {
                let sp = s.span.clone();
                p.bump();
                p.expect_kw(Token::Archivo)?;
                let ruta = parse_texto_lit(p)?;
                p.expect_kw(Token::Con)?;
                let capacidad = parse_ident(p)?;
                p.expect_kw(Token::Como)?;
                let guardar_como = parse_ident(p)?;
                Ok(Stmt::OperarIO {
                    operacion: OpIOStmt::LeerArchivo,
                    ruta,
                    datos: None,
                    capacidad,
                    guardar_como: Some(guardar_como),
                    span: sp.start..p.fin(),
                })
            }
            Token::Escribir => {
                let sp = s.span.clone();
                p.bump();
                let datos = parse_expr(p)?;
                p.expect_kw(Token::En)?;
                p.expect_kw(Token::Archivo)?;
                let ruta = parse_texto_lit(p)?;
                p.expect_kw(Token::Con)?;
                let capacidad = parse_ident(p)?;
                Ok(Stmt::OperarIO {
                    operacion: OpIOStmt::EscribirArchivo,
                    ruta,
                    datos: Some(datos),
                    capacidad,
                    guardar_como: None,
                    span: sp.start..p.fin(),
                })
            }
            _ => Err(ErrorFrontend::TokenInesperado {
                esperado: "verificar | asumir | demostrar | let | retornar | solicitar | validar | enviar | recibir | generar | leer | escribir".into(),
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
    let inicio = inicio_expr(p);
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
        let span = inicio..p.fin();
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
    let inicio = inicio_expr(p);
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
        let span = inicio..p.fin();
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
    let inicio = inicio_expr(p);
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
        let span = inicio..p.fin();
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
                let inicio = s.span.start;
                p.bump();
                let rhs = parse_primary(p)?;
                let span = inicio..p.fin();
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
                        span: sp.start..p.fin(),
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

/// Lee un literal de texto ("...") y retorna su contenido.
fn parse_texto_lit(p: &mut P) -> Result<String, ErrorFrontend> {
    match p.peek().clone() {
        TokenFull::Tok(s) => match s.tok {
            Token::Texto(st) => {
                p.bump();
                Ok(st)
            }
            _ => Err(ErrorFrontend::TokenInesperado {
                esperado: "texto entre comillas".into(),
                encontrado: desc(p.peek()),
                span: span_of(p.peek()),
            }),
        },
        TokenFull::Eof => Err(ErrorFrontend::FinInesperado),
        other => Err(ErrorFrontend::TokenInesperado {
            esperado: "texto entre comillas".into(),
            encontrado: desc(&other),
            span: span_of(&other),
        }),
    }
}

/// Lee un entero no negativo (para ttl).
fn parse_entero_u64(p: &mut P) -> Result<u64, ErrorFrontend> {
    match p.peek().clone() {
        TokenFull::Tok(s) => match s.tok {
            Token::Entero(n) => {
                p.bump();
                u64::try_from(n).map_err(|_| ErrorFrontend::TokenInesperado {
                    esperado: "entero no negativo".into(),
                    encontrado: format!("{n}"),
                    span: span_of(p.peek()),
                })
            }
            _ => Err(ErrorFrontend::TokenInesperado {
                esperado: "entero no negativo".into(),
                encontrado: desc(p.peek()),
                span: span_of(p.peek()),
            }),
        },
        TokenFull::Eof => Err(ErrorFrontend::FinInesperado),
        other => Err(ErrorFrontend::TokenInesperado {
            esperado: "entero no negativo".into(),
            encontrado: desc(&other),
            span: span_of(&other),
        }),
    }
}

/// Lee un permiso de capacidad (lectura_archivo | escritura_archivo | red | ejecucion).
fn parse_permiso_cap(p: &mut P) -> Result<PermisoCap, ErrorFrontend> {
    // `red` es keyword del lenguaje; el resto son identificadores.
    if p.peek_is_kw(&Token::Red) {
        p.bump();
        return Ok(PermisoCap::Red);
    }
    let id = parse_ident(p)?;
    match id.nombre.as_str() {
        "lectura_archivo" => Ok(PermisoCap::LecturaArchivo),
        "escritura_archivo" => Ok(PermisoCap::EscrituraArchivo),
        "red" => Ok(PermisoCap::Red),
        "ejecucion" => Ok(PermisoCap::Ejecucion),
        _ => Err(ErrorFrontend::TokenInesperado {
            esperado: "lectura_archivo | escritura_archivo | red | ejecucion".into(),
            encontrado: id.nombre,
            span: span_of(p.peek()),
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
    fn restriccion_exige_valor_identificador() {
        let r = parse("definir tarea f() -> Entero64:\n    retornar 1\n    bajo restricciones:\n        latencia_maxima: 200\n");
        assert!(r.is_err(), "un valor numérico no es un identificador");
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
