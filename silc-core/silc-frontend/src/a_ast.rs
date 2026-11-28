//! Construcción del AST a partir de S-expressions.
//!
//! El lector (`reader`) produce átomos, textos y listas; este módulo les da
//! significado. El contrato de cada forma está en `docs/SPEC-v2.md`: una lista
//! es una forma, y su primer elemento dice cuál.

use crate::ast::{
    BinOp, Campo, CapacidadDecl, CasoVariante, ClaseExportada, ConfiguracionRed, Cuerpo, Decl,
    EstructuraDecl, Exportacion, Expr, Ident, Literal, ManejadorEvento, MemoriaPersistenteDecl,
    MigracionDecl, ModeloDecl, ModuloDecl, OpIOStmt, Param, PermisoCap, Program, Restriccion,
    ServicioDecl, Span, Stmt, TareaDecl, TipoDato, VarianteDecl,
};
use crate::error::ErrorFrontend;
use crate::reader::{self, Sexp};
use miette::SourceSpan;

/// Lee una fuente S-expression completa y devuelve su AST.
pub fn parsear(fuente: &str) -> Result<Program, ErrorFrontend> {
    programa(&reader::leer(fuente)?)
}

/// Convierte formas de nivel superior en un `Program`.
pub fn programa(formas: &[Sexp]) -> Result<Program, ErrorFrontend> {
    let mut defs = Vec::with_capacity(formas.len());
    for f in formas {
        defs.push(declaracion(f)?);
    }
    Ok(Program { defs })
}

// =============================================================================
// Acceso a formas
// =============================================================================

/// Lista con cabeza atómica: `(nombre hijo...)`.
struct Forma<'a> {
    nombre: &'a str,
    items: &'a [Sexp],
    span: &'a Span,
}

impl<'a> Forma<'a> {
    fn abrir(s: &'a Sexp) -> Result<Self, ErrorFrontend> {
        let items = s
            .items()
            .ok_or_else(|| inesperado("una forma (lista entre paréntesis)", s))?;
        let cabeza = items
            .first()
            .ok_or_else(|| inesperado("un nombre de forma dentro de la lista", s))?;
        let nombre = cabeza
            .atomo()
            .ok_or_else(|| inesperado("un nombre de forma (átomo)", cabeza))?;
        Ok(Self {
            nombre,
            items,
            span: s.span(),
        })
    }

    fn hijos(&self) -> &'a [Sexp] {
        &self.items[1..]
    }

    fn aridad(&self, min: usize, max: usize) -> Result<(), ErrorFrontend> {
        let n = self.items.len();
        if n < min || n > max {
            let esperado = if min == max {
                format!("{min} elementos")
            } else {
                format!("{min}..={max} elementos")
            };
            return Err(ErrorFrontend::AridadIncorrecta {
                forma: self.nombre.to_string(),
                esperado,
                obtenido: n,
                span: self.sspan(),
            });
        }
        Ok(())
    }

    fn exacta(&self, n: usize) -> Result<(), ErrorFrontend> {
        self.aridad(n, n)
    }

    fn sspan(&self) -> SourceSpan {
        (self.span.start, self.span.len()).into()
    }
}

fn sspan(s: &Sexp) -> SourceSpan {
    (s.span().start, s.span().len()).into()
}

fn describir(s: &Sexp) -> String {
    match s {
        Sexp::Atomo { texto, .. } => format!("átomo '{texto}'"),
        Sexp::Texto { .. } => "texto entre comillas".to_string(),
        Sexp::Lista { .. } => "lista".to_string(),
    }
}

fn inesperado(esperado: &str, s: &Sexp) -> ErrorFrontend {
    ErrorFrontend::TokenInesperado {
        esperado: esperado.to_string(),
        encontrado: describir(s),
        span: sspan(s),
    }
}

fn falta(f: &Forma, nombre: &str) -> ErrorFrontend {
    ErrorFrontend::TokenInesperado {
        esperado: format!("la forma '{nombre}'"),
        encontrado: "ausente".to_string(),
        span: f.sspan(),
    }
}

fn duplicada(g: &Forma) -> ErrorFrontend {
    ErrorFrontend::TokenInesperado {
        esperado: format!("una sola forma '{}'", g.nombre),
        encontrado: format!("'{}' repetida", g.nombre),
        span: g.sspan(),
    }
}

fn aridad_lista(s: &Sexp, forma: &str, esperado: usize) -> ErrorFrontend {
    ErrorFrontend::AridadIncorrecta {
        forma: forma.to_string(),
        esperado: format!("{esperado} elementos"),
        obtenido: s.items().map_or(0, |i| i.len()),
        span: sspan(s),
    }
}

/// Elementos de una lista cuyo primer elemento no es un nombre de forma
/// (parámetro, campo, caso de variante, restricción).
fn elementos(s: &Sexp) -> Result<&[Sexp], ErrorFrontend> {
    s.items().ok_or_else(|| inesperado("una lista", s))
}

fn es_identificador(t: &str) -> bool {
    let mut cs = t.chars();
    match cs.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn ident_de(s: &Sexp) -> Result<Ident, ErrorFrontend> {
    let texto = s.atomo().ok_or_else(|| inesperado("un identificador", s))?;
    if !es_identificador(texto) || matches!(texto, "verdadero" | "falso") {
        return Err(ErrorFrontend::IdentificadorInvalido {
            nombre: texto.to_string(),
            span: sspan(s),
        });
    }
    Ok(Ident {
        nombre: texto.to_string(),
        span: s.span().clone(),
    })
}

fn texto_de(s: &Sexp) -> Result<String, ErrorFrontend> {
    match s {
        Sexp::Texto { valor, .. } => Ok(valor.clone()),
        otro => Err(inesperado("un texto entre comillas", otro)),
    }
}

/// Entero no negativo para puertos, capacidades y `ttl`.
fn entero_u64(s: &Sexp) -> Result<u64, ErrorFrontend> {
    match s.atomo().and_then(|t| t.parse::<u64>().ok()) {
        Some(n) => Ok(n),
        None => Err(inesperado("un entero no negativo", s)),
    }
}

// =============================================================================
// Nivel superior
// =============================================================================

fn declaracion(s: &Sexp) -> Result<Decl, ErrorFrontend> {
    let f = Forma::abrir(s)?;
    match f.nombre {
        "task" => Ok(Decl::Tarea(tarea(&f)?)),
        "struct" => Ok(Decl::Estructura(EstructuraDecl {
            nombre: ident_de(&f.items[1])?,
            campos: campos_en(hijo(&f, 2, "fields")?)?,
            span: f.span.clone(),
        })),
        "model" => Ok(Decl::Modelo(ModeloDecl {
            nombre: ident_de(&f.items[1])?,
            campos: campos_en(hijo(&f, 2, "fields")?)?,
            span: f.span.clone(),
        })),
        "memory" => Ok(Decl::MemoriaPersistente(MemoriaPersistenteDecl {
            nombre: ident_de(&f.items[1])?,
            campos: campos_en(hijo(&f, 2, "fields")?)?,
            span: f.span.clone(),
        })),
        "variant" => Ok(Decl::Variante(variante(&f)?)),
        "service" => Ok(Decl::Servicio(servicio(&f)?)),
        "capability" => Ok(Decl::Capacidad(capacidad(&f)?)),
        "module" => Ok(Decl::Modulo(modulo(&f)?)),
        "migration" => Ok(Decl::Migracion(migracion(&f)?)),
        otro => Err(forma_desconocida(otro, &f)),
    }
}

fn forma_desconocida(nombre: &str, f: &Forma) -> ErrorFrontend {
    ErrorFrontend::FormaDesconocida {
        nombre: nombre.to_string(),
        span: f.sspan(),
    }
}

/// Hijo posicional obligatorio, con nombre de forma comprobado.
fn hijo<'a>(f: &Forma<'a>, i: usize, nombre: &str) -> Result<&'a Sexp, ErrorFrontend> {
    let s = f.items.get(i).ok_or_else(|| falta(f, nombre))?;
    if s.items().and_then(|it| it.first()).and_then(Sexp::atomo) != Some(nombre) {
        return Err(inesperado(&format!("la forma '{nombre}'"), s));
    }
    Ok(s)
}

fn tarea(f: &Forma) -> Result<TareaDecl, ErrorFrontend> {
    f.aridad(2, usize::MAX)?;
    let nombre = ident_de(&f.items[1])?;
    let mut params = None;
    let mut retorno = None;
    let mut precondiciones = Vec::new();
    let mut postcondiciones = Vec::new();
    let mut cuerpo = None;

    for h in f.hijos().iter().skip(1) {
        let g = Forma::abrir(h)?;
        match g.nombre {
            "params" => {
                if params.is_some() {
                    return Err(duplicada(&g));
                }
                params = Some(params_de(&g)?);
            }
            "returns" => {
                if retorno.is_some() {
                    return Err(duplicada(&g));
                }
                g.exacta(2)?;
                retorno = Some(tipo_de(&g.items[1])?);
            }
            "requires" => {
                for e in g.hijos() {
                    precondiciones.push(expr_de(e)?);
                }
            }
            "ensures" => {
                for e in g.hijos() {
                    postcondiciones.push(expr_de(e)?);
                }
            }
            "body" => {
                if cuerpo.is_some() {
                    return Err(duplicada(&g));
                }
                cuerpo = Some(cuerpo_de(h)?);
            }
            otro => return Err(forma_desconocida(otro, &g)),
        }
    }

    Ok(TareaDecl {
        nombre,
        params: params.ok_or_else(|| falta(f, "params"))?,
        retorno,
        cuerpo: cuerpo.ok_or_else(|| falta(f, "body"))?,
        span: f.span.clone(),
        precondiciones,
        postcondiciones,
    })
}

fn params_de(g: &Forma) -> Result<Vec<Param>, ErrorFrontend> {
    g.hijos()
        .iter()
        .map(|h| {
            let it = elementos(h)?;
            if it.len() != 2 {
                return Err(aridad_lista(h, "parámetro", 2));
            }
            Ok(Param {
                nombre: ident_de(&it[0])?,
                tipo: tipo_de(&it[1])?,
            })
        })
        .collect()
}

fn campos_en(s: &Sexp) -> Result<Vec<Campo>, ErrorFrontend> {
    let g = Forma::abrir(s)?;
    if g.nombre != "fields" {
        return Err(inesperado("la forma 'fields'", s));
    }
    g.hijos()
        .iter()
        .map(|h| {
            let it = elementos(h)?;
            if it.len() != 2 {
                return Err(aridad_lista(h, "campo", 2));
            }
            Ok(Campo {
                nombre: ident_de(&it[0])?,
                tipo: tipo_de(&it[1])?,
            })
        })
        .collect()
}

fn variante(f: &Forma) -> Result<VarianteDecl, ErrorFrontend> {
    f.aridad(2, usize::MAX)?;
    let mut casos = Vec::new();
    for h in f.hijos().iter().skip(1) {
        let g = Forma::abrir(h)?;
        if g.nombre != "case" {
            return Err(inesperado("la forma 'case'", h));
        }
        g.exacta(3)?;
        casos.push(CasoVariante {
            nombre: ident_de(&g.items[1])?,
            datos: campos_en(&g.items[2])?,
        });
    }
    Ok(VarianteDecl {
        nombre: ident_de(&f.items[1])?,
        casos,
        span: f.span.clone(),
    })
}

fn servicio(f: &Forma) -> Result<ServicioDecl, ErrorFrontend> {
    f.aridad(2, usize::MAX)?;
    let nombre = ident_de(&f.items[1])?;
    let mut red = None;
    let mut manejadores = Vec::new();
    let mut tareas = Vec::new();
    let mut restricciones: Option<Vec<Restriccion>> = None;

    for h in f.hijos().iter().skip(1) {
        let g = Forma::abrir(h)?;
        match g.nombre {
            "net" => {
                if red.is_some() {
                    return Err(duplicada(&g));
                }
                g.aridad(2, 3)?;
                let protocolo = match g.items.get(2) {
                    None => None,
                    Some(p) => {
                        let pg = Forma::abrir(p)?;
                        if pg.nombre != "protocol" {
                            return Err(inesperado("la forma 'protocol'", p));
                        }
                        pg.exacta(2)?;
                        Some(ident_de(&pg.items[1])?)
                    }
                };
                red = Some(ConfiguracionRed {
                    puerto: entero_u64(&g.items[1])?,
                    protocolo,
                });
            }
            "handler" => manejadores.push(manejador_de(&g)?),
            "task" => tareas.push(tarea(&g)?),
            "restrictions" => {
                if restricciones.is_some() {
                    return Err(duplicada(&g));
                }
                restricciones = Some(restricciones_de(&g)?);
            }
            otro => return Err(forma_desconocida(otro, &g)),
        }
    }

    Ok(ServicioDecl {
        nombre,
        red,
        manejadores,
        tareas,
        restricciones: restricciones.unwrap_or_default(),
        span: f.span.clone(),
    })
}

fn manejador_de(g: &Forma) -> Result<ManejadorEvento, ErrorFrontend> {
    g.exacta(4)?;
    Ok(ManejadorEvento {
        evento: ident_de(&g.items[1])?,
        params: params_de(&Forma::abrir(&g.items[2])?)?,
        cuerpo: cuerpo_de(&g.items[3])?,
        span: g.span.clone(),
    })
}

fn capacidad(f: &Forma) -> Result<CapacidadDecl, ErrorFrontend> {
    f.exacta(5)?;
    let resource = Forma::abrir(hijo(f, 2, "resource")?)?;
    resource.exacta(2)?;
    let port = Forma::abrir(hijo(f, 3, "port")?)?;
    port.exacta(2)?;
    let bandwidth = Forma::abrir(hijo(f, 4, "bandwidth")?)?;
    bandwidth.exacta(2)?;
    Ok(CapacidadDecl {
        nombre: ident_de(&f.items[1])?,
        recurso: texto_de(&resource.items[1])?,
        puerto: entero_u64(&port.items[1])?,
        limite_ancho_banda: texto_de(&bandwidth.items[1])?,
        span: f.span.clone(),
    })
}

fn modulo(f: &Forma) -> Result<ModuloDecl, ErrorFrontend> {
    f.aridad(2, usize::MAX)?;
    let mut exportaciones = Vec::new();
    for h in f.hijos().iter().skip(1) {
        let g = Forma::abrir(h)?;
        if g.nombre != "export" {
            return Err(inesperado("la forma 'export'", h));
        }
        g.exacta(3)?;
        let clase = match g.items[1].atomo() {
            Some("service") => ClaseExportada::Servicio,
            Some("task") => ClaseExportada::Tarea,
            Some("struct") => ClaseExportada::Estructura,
            Some("variant") => ClaseExportada::Variante,
            Some("capability") => ClaseExportada::Capacidad,
            _ => {
                return Err(inesperado(
                    "una clase exportable (service, task, struct, variant, capability)",
                    &g.items[1],
                ))
            }
        };
        exportaciones.push(Exportacion {
            clase,
            nombre: ident_de(&g.items[2])?,
        });
    }
    Ok(ModuloDecl {
        nombre: ident_de(&f.items[1])?,
        exportaciones,
        span: f.span.clone(),
    })
}

fn migracion(f: &Forma) -> Result<MigracionDecl, ErrorFrontend> {
    f.aridad(3, usize::MAX)?;
    let origen = ident_de(&f.items[1])?;
    let mut desde = None;
    let mut hacia = None;
    let mut campo = None;
    let mut registro = None;
    let mut predeterminado = None;
    let mut restricciones: Option<Vec<Restriccion>> = None;

    for h in f.hijos().iter().skip(1) {
        let g = Forma::abrir(h)?;
        if g.nombre != "restrictions" {
            g.exacta(2)?;
        }
        match g.nombre {
            "from" => {
                if desde.is_some() {
                    return Err(duplicada(&g));
                }
                desde = Some(ident_de(&g.items[1])?);
            }
            "to" => {
                if hacia.is_some() {
                    return Err(duplicada(&g));
                }
                hacia = Some(ident_de(&g.items[1])?);
            }
            "field" => {
                if campo.is_some() {
                    return Err(duplicada(&g));
                }
                campo = Some(texto_de(&g.items[1])?);
            }
            "record" => {
                if registro.is_some() {
                    return Err(duplicada(&g));
                }
                registro = Some(ident_de(&g.items[1])?);
            }
            "default" => {
                if predeterminado.is_some() {
                    return Err(duplicada(&g));
                }
                predeterminado = Some(expr_de(&g.items[1])?);
            }
            "restrictions" => {
                if restricciones.is_some() {
                    return Err(duplicada(&g));
                }
                restricciones = Some(restricciones_de(&g)?);
            }
            otro => return Err(forma_desconocida(otro, &g)),
        }
    }

    Ok(MigracionDecl {
        origen,
        version_desde: desde.ok_or_else(|| falta(f, "from"))?,
        version_a: hacia.ok_or_else(|| falta(f, "to"))?,
        campo: campo.ok_or_else(|| falta(f, "field"))?,
        registro: registro.ok_or_else(|| falta(f, "record"))?,
        valor_predeterminado: predeterminado.ok_or_else(|| falta(f, "default"))?,
        restricciones: restricciones.unwrap_or_default(),
        span: f.span.clone(),
    })
}

// =============================================================================
// Cuerpos y sentencias
// =============================================================================

fn cuerpo_de(s: &Sexp) -> Result<Cuerpo, ErrorFrontend> {
    let f = Forma::abrir(s)?;
    if f.nombre != "body" {
        return Err(inesperado("la forma 'body'", s));
    }
    let hijos = f.hijos();
    let mut stmts = Vec::new();
    let mut restricciones = Vec::new();
    for (i, h) in hijos.iter().enumerate() {
        let es_restricciones =
            h.items().and_then(|it| it.first()).and_then(Sexp::atomo) == Some("restrictions");
        if es_restricciones {
            if i + 1 != hijos.len() {
                return Err(inesperado("'restrictions' como último hijo del cuerpo", h));
            }
            restricciones = restricciones_de(&Forma::abrir(h)?)?;
        } else {
            stmts.push(sentencia_de(h)?);
        }
    }
    Ok(Cuerpo {
        stmts,
        restricciones,
    })
}

fn restricciones_de(g: &Forma) -> Result<Vec<Restriccion>, ErrorFrontend> {
    g.hijos()
        .iter()
        .map(|h| {
            let it = elementos(h)?;
            if it.len() != 2 {
                return Err(aridad_lista(h, "restricción", 2));
            }
            Ok(Restriccion {
                clave: palabra_restriccion(&it[0])?,
                valor: palabra_restriccion(&it[1])?,
                span: h.span().clone(),
            })
        })
        .collect()
}

fn palabra_restriccion(s: &Sexp) -> Result<String, ErrorFrontend> {
    match s.atomo() {
        Some(t) if es_identificador(t) => Ok(t.to_string()),
        _ => Err(inesperado(
            "una palabra de restricción (átomo no numérico)",
            s,
        )),
    }
}

fn sentencia_de(s: &Sexp) -> Result<Stmt, ErrorFrontend> {
    let f = Forma::abrir(s)?;
    let span = f.span.clone();
    match f.nombre {
        "if" => {
            f.aridad(3, 4)?;
            let sino = match f.items.get(3) {
                None => None,
                Some(h) => {
                    let g = Forma::abrir(h)?;
                    if g.nombre != "else" {
                        return Err(inesperado("la forma 'else'", h));
                    }
                    g.exacta(2)?;
                    Some(Box::new(cuerpo_de(&g.items[1])?))
                }
            };
            Ok(Stmt::Si {
                cond: expr_de(&f.items[1])?,
                cuerpo: cuerpo_de(&f.items[2])?,
                sino,
                span,
            })
        }
        "assert" => {
            f.exacta(2)?;
            Ok(Stmt::Verificar {
                cond: expr_de(&f.items[1])?,
                span,
            })
        }
        "assume" => {
            f.exacta(2)?;
            Ok(Stmt::Asumir {
                cond: expr_de(&f.items[1])?,
                span,
            })
        }
        "prove" => {
            f.exacta(2)?;
            Ok(Stmt::Demostrar {
                cond: expr_de(&f.items[1])?,
                span,
            })
        }
        "invariant" => {
            f.exacta(2)?;
            Ok(Stmt::InvarianteBucle {
                cond: expr_de(&f.items[1])?,
                span,
            })
        }
        "while" => {
            f.exacta(3)?;
            Ok(Stmt::Mientras {
                cond: expr_de(&f.items[1])?,
                cuerpo: cuerpo_de(&f.items[2])?,
                span,
            })
        }
        "for" => {
            f.exacta(4)?;
            Ok(Stmt::Para {
                variable: ident_de(&f.items[1])?.nombre,
                iterable: expr_de(&f.items[2])?,
                cuerpo: cuerpo_de(&f.items[3])?,
                span,
            })
        }
        "let" | "let-mut" => {
            f.aridad(3, 4)?;
            let tipo = match f.items.len() {
                4 => Some(tipo_de(&f.items[2])?),
                _ => None,
            };
            let expr = f.items.last().expect("aridad 3..=4");
            Ok(Stmt::Asignar {
                nombre: ident_de(&f.items[1])?,
                tipo,
                expr: expr_de(expr)?,
                mutable: f.nombre == "let-mut",
                span,
            })
        }
        "return" => {
            f.aridad(1, 2)?;
            Ok(Stmt::Retornar {
                expr: f.items.get(1).map(expr_de).transpose()?,
                span,
            })
        }
        "request" => {
            f.aridad(3, 5)?;
            let permiso = permiso_de(&f.items[1])?;
            let recurso = texto_de(&f.items[2])?;
            let mut ttl_ns = None;
            let mut guardar_como = None;
            for h in &f.items[3..] {
                let g = Forma::abrir(h)?;
                g.exacta(2)?;
                match g.nombre {
                    "ttl" => {
                        if ttl_ns.is_some() {
                            return Err(duplicada(&g));
                        }
                        ttl_ns = Some(entero_u64(&g.items[1])?);
                    }
                    "as" => {
                        if guardar_como.is_some() {
                            return Err(duplicada(&g));
                        }
                        guardar_como = Some(ident_de(&g.items[1])?);
                    }
                    otro => return Err(forma_desconocida(otro, &g)),
                }
            }
            Ok(Stmt::SolicitarCapacidad {
                permiso,
                recurso,
                ttl_ns,
                guardar_como,
                span,
            })
        }
        "validate" => {
            f.exacta(2)?;
            Ok(Stmt::ValidarCapacidad {
                token: ident_de(&f.items[1])?,
                span,
            })
        }
        "send" => {
            f.exacta(3)?;
            Ok(Stmt::EnviarCanal {
                valor: expr_de(&f.items[1])?,
                canal: ident_de(&f.items[2])?,
                span,
            })
        }
        "recv" => {
            f.exacta(3)?;
            Ok(Stmt::RecibirCanal {
                canal: ident_de(&f.items[1])?,
                guardar_como: ident_de(&f.items[2])?,
                span,
            })
        }
        "spawn" => {
            f.exacta(2)?;
            Ok(Stmt::GenerarFibra {
                tarea: ident_de(&f.items[1])?,
                span,
            })
        }
        "chan" => {
            f.aridad(2, 3)?;
            let capacidad = match f.items.get(2) {
                None => None,
                Some(h) => {
                    let g = Forma::abrir(h)?;
                    if g.nombre != "capacity" {
                        return Err(inesperado("la forma 'capacity'", h));
                    }
                    g.exacta(2)?;
                    Some(entero_u64(&g.items[1])?)
                }
            };
            Ok(Stmt::CrearCanal {
                capacidad,
                guardar_como: ident_de(&f.items[1])?,
                span,
            })
        }
        "io-read" => {
            f.exacta(4)?;
            Ok(Stmt::OperarIO {
                operacion: OpIOStmt::LeerArchivo,
                ruta: texto_de(&f.items[1])?,
                datos: None,
                capacidad: ident_de(&f.items[2])?,
                guardar_como: Some(ident_de(&f.items[3])?),
                span,
            })
        }
        "io-write" => {
            f.exacta(4)?;
            Ok(Stmt::OperarIO {
                operacion: OpIOStmt::EscribirArchivo,
                ruta: texto_de(&f.items[2])?,
                datos: Some(expr_de(&f.items[1])?),
                capacidad: ident_de(&f.items[3])?,
                guardar_como: None,
                span,
            })
        }
        "io-send" => {
            f.exacta(4)?;
            Ok(Stmt::OperarIO {
                operacion: OpIOStmt::EnviarRed,
                ruta: texto_de(&f.items[2])?,
                datos: Some(expr_de(&f.items[1])?),
                capacidad: ident_de(&f.items[3])?,
                guardar_como: None,
                span,
            })
        }
        "io-recv" => {
            f.exacta(4)?;
            Ok(Stmt::OperarIO {
                operacion: OpIOStmt::RecibirRed,
                ruta: texto_de(&f.items[1])?,
                datos: None,
                capacidad: ident_de(&f.items[2])?,
                guardar_como: Some(ident_de(&f.items[3])?),
                span,
            })
        }
        otro => Err(forma_desconocida(otro, &f)),
    }
}

fn permiso_de(s: &Sexp) -> Result<PermisoCap, ErrorFrontend> {
    match s.atomo() {
        Some("lectura_archivo") => Ok(PermisoCap::LecturaArchivo),
        Some("escritura_archivo") => Ok(PermisoCap::EscrituraArchivo),
        Some("red") => Ok(PermisoCap::Red),
        Some("ejecucion") => Ok(PermisoCap::Ejecucion),
        _ => Err(inesperado(
            "un permiso (lectura_archivo, escritura_archivo, red, ejecucion)",
            s,
        )),
    }
}

// =============================================================================
// Expresiones
// =============================================================================

fn expr_de(s: &Sexp) -> Result<Expr, ErrorFrontend> {
    match s {
        Sexp::Texto { valor, .. } => Ok(Expr::Lit(Literal::Texto(valor.clone()))),
        Sexp::Atomo { texto, span } => {
            if es_numero(texto) {
                return numero(texto, s);
            }
            match texto.as_str() {
                "verdadero" => Ok(Expr::Lit(Literal::Booleano(true))),
                "falso" => Ok(Expr::Lit(Literal::Booleano(false))),
                _ if es_identificador(texto) => Ok(Expr::Var(Ident {
                    nombre: texto.clone(),
                    span: span.clone(),
                })),
                _ => Err(ErrorFrontend::IdentificadorInvalido {
                    nombre: texto.clone(),
                    span: sspan(s),
                }),
            }
        }
        Sexp::Lista { .. } => {
            let f = Forma::abrir(s)?;
            match f.nombre {
                "get" => {
                    f.exacta(3)?;
                    Ok(Expr::AccesoProp {
                        base: Box::new(expr_de(&f.items[1])?),
                        prop: ident_de(&f.items[2])?,
                        span: f.span.clone(),
                    })
                }
                "+" | "-" | "*" | "/" | ">" | "<" | "==" | "!=" | ">=" | "<=" => {
                    f.exacta(3)?;
                    Ok(Expr::BinOp {
                        op: operador(f.nombre),
                        lhs: Box::new(expr_de(&f.items[1])?),
                        rhs: Box::new(expr_de(&f.items[2])?),
                        span: f.span.clone(),
                    })
                }
                otro => Err(forma_desconocida(otro, &f)),
            }
        }
    }
}

fn operador(simbolo: &str) -> BinOp {
    match simbolo {
        ">" => BinOp::Gt,
        "<" => BinOp::Lt,
        "==" => BinOp::Eq,
        "!=" => BinOp::Ne,
        ">=" => BinOp::Ge,
        "<=" => BinOp::Le,
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        "*" => BinOp::Mul,
        "/" => BinOp::Div,
        otro => unreachable!("operador validado por la rama: {otro}"),
    }
}

/// Un átomo empieza por dígito, o por `+`/`-` seguido de dígito.
fn es_numero(t: &str) -> bool {
    let mut cs = t.chars();
    match cs.next() {
        Some(c) if c.is_ascii_digit() => true,
        Some('+') | Some('-') => cs.next().is_some_and(|c| c.is_ascii_digit()),
        _ => false,
    }
}

fn numero(texto: &str, s: &Sexp) -> Result<Expr, ErrorFrontend> {
    let cuerpo = texto
        .strip_prefix('+')
        .or_else(|| texto.strip_prefix('-'))
        .unwrap_or(texto);
    let flotante = cuerpo.split_once('.');
    let forma_valida = match flotante {
        Some((entera, decimal)) => {
            !entera.is_empty()
                && !decimal.is_empty()
                && entera.bytes().all(|b| b.is_ascii_digit())
                && decimal.bytes().all(|b| b.is_ascii_digit())
        }
        None => !cuerpo.is_empty() && cuerpo.bytes().all(|b| b.is_ascii_digit()),
    };
    if forma_valida {
        if flotante.is_some() {
            if let Ok(f) = texto.parse::<f64>() {
                return Ok(Expr::Lit(Literal::Flotante(f)));
            }
        } else if let Ok(n) = texto.parse::<i64>() {
            return Ok(Expr::Lit(Literal::Entero(n)));
        }
    }
    Err(inesperado("un número válido (entero o flotante)", s))
}

// =============================================================================
// Tipos
// =============================================================================

fn tipo_de(s: &Sexp) -> Result<TipoDato, ErrorFrontend> {
    if let Some(texto) = s.atomo() {
        return Ok(match texto {
            "i64" => TipoDato::Entero64,
            "f64" => TipoDato::Flotante64,
            "booleano" => TipoDato::Booleano,
            "texto" => TipoDato::Texto,
            "capacidad" => TipoDato::CapacidadHardware,
            "void" => TipoDato::Void,
            "USD" => TipoDato::USD,
            "EUR" => TipoDato::EUR,
            otro if es_identificador(otro) => TipoDato::Nominal(otro.to_string()),
            _ => return Err(inesperado("un tipo", s)),
        });
    }
    let f = Forma::abrir(s)?;
    match f.nombre {
        "list" => {
            f.exacta(2)?;
            Ok(TipoDato::Lista(Box::new(tipo_de(&f.items[1])?)))
        }
        "map" => {
            f.exacta(3)?;
            Ok(TipoDato::Mapa(
                Box::new(tipo_de(&f.items[1])?),
                Box::new(tipo_de(&f.items[2])?),
            ))
        }
        "set" => {
            f.exacta(2)?;
            Ok(TipoDato::Conjunto(Box::new(tipo_de(&f.items[1])?)))
        }
        "tuple" => f
            .hijos()
            .iter()
            .map(tipo_de)
            .collect::<Result<Vec<_>, _>>()
            .map(TipoDato::Tupla),
        otro => Err(ErrorFrontend::FormaDesconocida {
            nombre: otro.to_string(),
            span: f.sspan(),
        }),
    }
}
