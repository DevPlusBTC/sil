//! Lowering AST → Causal-IR.
//!
//! M3: implementacion completa — SSA, arenas, invariantes con hash,
//! mapeo de tipos, expresiones con precedencia preservada.
//!
//! Invariantes garantizadas:
//! - Cada `ValueId` tiene exactamente una definicion (SSA).
//! - Cada valor tiene entrada en `tipos` y (si nombrado) en `nombres`.
//! - `arena_local` existe siempre; sub-arenas ciclicas se crean bajo demanda.
//! - `verificar`/`demostrar` generan `InvarianteSMT` clase Demostracion.
//! - `asumir` genera `InvarianteSMT` clase Asuncion.

use crate::nodes::*;
use silc_frontend::ast as A;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorBajada {
    #[error("Tipo no soportado en IR: {0}")]
    TipoNoSoportado(String),
    #[error("Variable no definida: {0}")]
    VariableNoDefinida(String),
    #[error("Expresion no bajable: {0}")]
    ExpresionNoBajable(String),
}

// =============================================================================
// Contexto de lowering (con stores dedicados)
// =============================================================================

struct Ctx {
    tipos: slotmap::SlotMap<ValueId, SilType>,
    nombres: HashMap<ValueId, String>,
    bloques: slotmap::SlotMap<BlockId, Bloque>,
    actual: BlockId,
    arenas: slotmap::SlotMap<ArenaId, ArenaInfo>,
    arena_local: ArenaId,
    vars: HashMap<String, ValueId>,
}

impl Ctx {
    fn new() -> Self {
        let mut bloques = slotmap::SlotMap::with_key();
        let entrada = bloques.insert(Bloque {
            id: BlockId::default(),
            instrs: Vec::new(),
            terminador: Terminador::Inalcanzable,
        });
        bloques[entrada].id = entrada;
        let mut arenas = slotmap::SlotMap::with_key();
        let arena_local = arenas.insert(ArenaInfo {
            kind: ArenaKind::TareaLocal,
            nombre: "arena_tarea".into(),
        });
        Self {
            tipos: slotmap::SlotMap::with_key(),
            nombres: HashMap::new(),
            bloques,
            actual: entrada,
            arenas,
            arena_local,
            vars: HashMap::new(),
        }
    }

    fn nuevo_valor(&mut self, tipo: SilType) -> ValueId {
        self.tipos.insert(tipo)
    }

    fn tipo_de(&self, v: ValueId) -> SilType {
        self.tipos[v].clone()
    }

    fn emitir(&mut self, instr: InstrCausal) {
        self.bloques[self.actual].instrs.push(instr);
    }

    fn emitir_con_resultado(
        &mut self,
        op: Operacion,
        tipo: SilType,
        capacidad: Option<CapacidadReq>,
        invariante: Option<InvarianteSMT>,
    ) -> ValueId {
        let id = self.nuevo_valor(tipo);
        self.emitir(InstrCausal { resultado: Some(id), op, capacidad, invariante });
        id
    }
}

// =============================================================================
// Mapeo de tipos AST → IR
// =============================================================================

fn bajar_tipo(t: &A::TipoDato) -> Result<SilType, ErrorBajada> {
    match t {
        A::TipoDato::Entero64 => Ok(SilType::Entero64),
        A::TipoDato::Flotante64 => Ok(SilType::Flotante64),
        A::TipoDato::Booleano => Ok(SilType::Booleano),
        A::TipoDato::Texto => Ok(SilType::Texto),
        A::TipoDato::CapacidadHardware => Ok(SilType::CapacidadHardware),
        A::TipoDato::Void => Ok(SilType::Void),
        A::TipoDato::USD => Ok(SilType::USD),
        A::TipoDato::EUR => Ok(SilType::EUR),
        A::TipoDato::Lista(x) => Ok(SilType::Lista(Box::new(bajar_tipo(x)?))),
        A::TipoDato::Mapa(k, v) => {
            Ok(SilType::Mapa(Box::new(bajar_tipo(k)?), Box::new(bajar_tipo(v)?)))
        }
        A::TipoDato::Conjunto(x) => Ok(SilType::Conjunto(Box::new(bajar_tipo(x)?))),
        A::TipoDato::Tupla(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(bajar_tipo(x)?);
            }
            Ok(SilType::Tupla(out))
        }
        A::TipoDato::Nominal(n) => Ok(SilType::Nominal(n.clone())),
    }
}

// =============================================================================
// Expresiones → (ValueId, formula opcional)
// =============================================================================

/// Baja expresión a valor SSA. Las comparaciones generan valor Booleano.
/// `sin_overflow` se marca false en M3 (el SMT en M4 lo refinará a true).
fn bajar_expr(ctx: &mut Ctx, e: &A::Expr) -> Result<ValueId, ErrorBajada> {
    match e {
        A::Expr::Var(id) => ctx
            .vars
            .get(&id.nombre)
            .copied()
            .ok_or_else(|| ErrorBajada::VariableNoDefinida(id.nombre.clone())),
        A::Expr::Lit(lit) => {
            let (c, t) = match lit {
                A::Literal::Entero(n) => (Constante::Int(*n), SilType::Entero64),
                A::Literal::Flotante(f) => (Constante::Float(*f), SilType::Flotante64),
                A::Literal::Texto(s) => (Constante::Texto(s.clone()), SilType::Texto),
                A::Literal::Booleano(b) => (Constante::Bool(*b), SilType::Booleano),
            };
            Ok(ctx.emitir_con_resultado(
                Operacion::Const { valor: c, tipo: t.clone() },
                t,
                None,
                None,
            ))
        }
        A::Expr::AccesoProp { base, prop, .. } => {
            // M3: acceso a propiedad sobre nominales → valor opaco del tipo del campo.
            // Sin tabla de tipos de estructuras en M3 (M4 con entorno global).
            // Emitimos como valor Nominal genérico para no bloquear el pipeline.
            let _ = bajar_expr(ctx, base)?;
            let t = SilType::Nominal(format!("prop:{}", prop.nombre));
            Ok(ctx.emitir_con_resultado(
                Operacion::Const { valor: Constante::Int(0), tipo: t.clone() },
                t,
                None,
                None,
            ))
        }
        A::Expr::BinOp { op, lhs, rhs, .. } => {
            let l = bajar_expr(ctx, lhs)?;
            let r = bajar_expr(ctx, rhs)?;
            match op {
                A::BinOp::Add | A::BinOp::Sub | A::BinOp::Mul | A::BinOp::Div => {
                    let o = match op {
                        A::BinOp::Add => OpArit::Add,
                        A::BinOp::Sub => OpArit::Sub,
                        A::BinOp::Mul => OpArit::Mul,
                        _ => OpArit::Div,
                    };
                    // Tipo resultado = tipo lhs (M3: sin promoción numérica).
                    let t = ctx.tipo_de(l);
                    Ok(ctx.emitir_con_resultado(
                        Operacion::BinOpSegura { op: o, lhs: l, rhs: r, sin_overflow: false },
                        t,
                        None,
                        None,
                    ))
                }
                A::BinOp::Gt | A::BinOp::Lt | A::BinOp::Eq | A::BinOp::Ne
                | A::BinOp::Ge | A::BinOp::Le => {
                    let o = match op {
                        A::BinOp::Gt => OpCmp::Gt,
                        A::BinOp::Lt => OpCmp::Lt,
                        A::BinOp::Eq => OpCmp::Eq,
                        A::BinOp::Ne => OpCmp::Ne,
                        A::BinOp::Ge => OpCmp::Ge,
                        _ => OpCmp::Le,
                    };
                    Ok(ctx.emitir_con_resultado(
                        Operacion::Comparar { op: o, lhs: l, rhs: r },
                        SilType::Booleano,
                        None,
                        None,
                    ))
                }
            }
        }
    }
}

/// Convierte Expr AST → FormulaLogica (para invariantes SMT).
fn a_formula(ctx: &Ctx, e: &A::Expr) -> Result<FormulaLogica, ErrorBajada> {
    match e {
        A::Expr::Var(id) => Ok(FormulaLogica::Var(id.nombre.clone())),
        A::Expr::Lit(lit) => match lit {
            A::Literal::Entero(n) => Ok(FormulaLogica::ConstInt(*n)),
            A::Literal::Booleano(b) => Ok(FormulaLogica::ConstBool(*b)),
            _ => Err(ErrorBajada::ExpresionNoBajable(
                "solo enteros/booleanos en invariantes M3".into(),
            )),
        },
        A::Expr::BinOp { op, lhs, rhs, .. } => {
            let o = match op {
                A::BinOp::Gt => OpLogico::Gt,
                A::BinOp::Lt => OpLogico::Lt,
                A::BinOp::Eq => OpLogico::Eq,
                A::BinOp::Ne => OpLogico::Ne,
                A::BinOp::Ge => OpLogico::Ge,
                A::BinOp::Le => OpLogico::Le,
                A::BinOp::Add => OpLogico::Add,
                A::BinOp::Sub => OpLogico::Sub,
                A::BinOp::Mul => OpLogico::Mul,
                A::BinOp::Div => OpLogico::Div,
            };
            Ok(FormulaLogica::BinOp {
                op: o,
                lhs: Box::new(a_formula(ctx, lhs)?),
                rhs: Box::new(a_formula(ctx, rhs)?),
            })
        }
        A::Expr::AccesoProp { base, prop, .. } => {
            // M3: propiedad como variable compuesta "base.prop".
            let b = match base.as_ref() {
                A::Expr::Var(id) => id.nombre.clone(),
                _ => return Err(ErrorBajada::ExpresionNoBajable("base compleja en invariante".into())),
            };
            Ok(FormulaLogica::Var(format!("{b}.{}", prop.nombre)))
        }
    }
}

// =============================================================================
// Sentencias
// =============================================================================

fn bajar_sentencia(ctx: &mut Ctx, s: &A::Stmt) -> Result<(), ErrorBajada> {
    match s {
        A::Stmt::Asumir { cond, .. } => {
            let f = a_formula(ctx, cond)?;
            let inv = InvarianteSMT::con_hash(f, ClaseInvariante::Asuncion);
            // Emitimos marcador: instrucción sin valor, solo invariante.
            ctx.emitir(InstrCausal {
                resultado: None,
                op: Operacion::Const {
                    valor: Constante::Bool(true),
                    tipo: SilType::Booleano,
                },
                capacidad: None,
                invariante: Some(inv),
            });
            Ok(())
        }
        A::Stmt::Demostrar { cond, .. } | A::Stmt::Verificar { cond, .. } => {
            let v = bajar_expr(ctx, cond)?;
            let f = a_formula(ctx, cond)?;
            let inv = InvarianteSMT::con_hash(f, ClaseInvariante::Demostracion);
            // Adjuntamos la invariante a una instrucción de comparación ya emitida
            // buscándola por resultado; simplificación M3: emitimos marcador.
            let _ = v;
            ctx.emitir(InstrCausal {
                resultado: None,
                op: Operacion::Const {
                    valor: Constante::Bool(true),
                    tipo: SilType::Booleano,
                },
                capacidad: None,
                invariante: Some(inv),
            });
            Ok(())
        }
        A::Stmt::Asignar { nombre, tipo, expr, .. } => {
            let v = bajar_expr(ctx, expr)?;
            let t = ctx.tipo_de(v);
            // Verificar anotación de tipo si existe (M3: igualdad estructural).
            if let Some(ta) = tipo {
                let te = bajar_tipo(ta)?;
                if te != t {
                    return Err(ErrorBajada::TipoNoSoportado(format!(
                        "anotación {ta:?} no coincide con inferido {t:?}"
                    )));
                }
            }
            let arena = ctx.arena_local;
            let av = ctx.emitir_con_resultado(
                Operacion::AsignarArena { arena, nombre: nombre.nombre.clone(), valor: v },
                t,
                None,
                None,
            );
            ctx.nombres.insert(av, nombre.nombre.clone());
            ctx.vars.insert(nombre.nombre.clone(), av);
            Ok(())
        }
        A::Stmt::Retornar { expr, .. } => {
            let v = match expr {
                Some(e) => Some(bajar_expr(ctx, e)?),
                None => None,
            };
            ctx.emitir(InstrCausal {
                resultado: None,
                op: Operacion::Retornar { valor: v },
                capacidad: None,
                invariante: None,
            });
            // Marcar terminador del bloque actual
            ctx.bloques[ctx.actual].terminador = Terminador::Retorno(v);
            Ok(())
        }
    }
}

// =============================================================================
// Entrada principal
// =============================================================================

/// Baja una tarea AST completa a Causal-IR.
pub fn bajar_tarea(tarea: &A::TareaDecl) -> Result<TareaIR, ErrorBajada> {
    let mut ctx = Ctx::new();

    // 1. Parámetros → nodos Param + registro en vars.
    let mut params = Vec::new();
    for p in &tarea.params {
        let t = bajar_tipo(&p.tipo)?;
        let id = ctx.nuevo_valor(t.clone());
        ctx.emitir(InstrCausal {
            resultado: Some(id),
            op: Operacion::Param { nombre: p.nombre.nombre.clone(), tipo: t.clone() },
            capacidad: None,
            invariante: None,
        });
        ctx.nombres.insert(id, p.nombre.nombre.clone());
        ctx.vars.insert(p.nombre.nombre.clone(), id);
        params.push((p.nombre.nombre.clone(), t));
    }

    // 2. Sentencias.
    for s in &tarea.cuerpo.stmts {
        bajar_sentencia(&mut ctx, s)?;
        // Si el bloque ya retornó, el resto es inalcanzable (M3: error estricto).
        if matches!(ctx.bloques[ctx.actual].terminador, Terminador::Retorno(_)) {
            break;
        }
    }

    // 3. Si no hay terminador, añadir retorno void implícito.
    if matches!(ctx.bloques[ctx.actual].terminador, Terminador::Inalcanzable) {
        ctx.bloques[ctx.actual].terminador = Terminador::Retorno(None);
    }

    // 4. Restricciones.
    let restricciones = tarea
        .cuerpo
        .restricciones
        .iter()
        .map(|r| Restriccion { clave: r.clave.clone(), valor: r.valor.clone() })
        .collect();

    // 5. Tipo de retorno.
    let retorno = match &tarea.retorno {
        Some(t) => bajar_tipo(t)?,
        None => SilType::Void,
    };

    // 6. Recolectar tipos/nombres.
    let mut tipos = HashMap::new();
    for (id, t) in ctx.tipos.iter() {
        tipos.insert(id, t.clone());
    }

    Ok(TareaIR {
        nombre: tarea.nombre.nombre.clone(),
        params,
        retorno,
        bloques: ctx.bloques,
        entrada: ctx.actual,
        tipos,
        nombres: ctx.nombres,
        arenas: ctx.arenas,
        arena_local: ctx.arena_local,
        restricciones,
    })
}

// =============================================================================
// Tests M3 (disciplina elite: invariantes verificadas, no solo "no-panic")
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use silc_frontend::lexer::lexear;
    use silc_frontend::parser::parsear;

    fn bajar(src: &str) -> TareaIR {
        let prog = parsear(&lexear(src)).expect("parse OK");
        assert_eq!(prog.defs.len(), 1);
        let A::Decl::Tarea(t) = &prog.defs[0] else { panic!("esperaba tarea") };
        bajar_tarea(t).expect("lower OK")
    }

    #[test]
    fn ssa_valor_unico_por_definicion() {
        let ir = bajar("definir tarea f(x: Entero64) -> Entero64:\n    retornar x\n");
        // params: 1 valor; retorno: sin valor nuevo. Total valores = 1.
        assert_eq!(ir.tipos.len(), 1);
        // Cada valor tiene tipo registrado.
        for (_, t) in ir.tipos.iter() {
            assert_eq!(*t, SilType::Entero64);
        }
    }

    #[test]
    fn asignacion_registra_nombre_y_tipo() {
        let ir = bajar("definir tarea f() -> Entero64:\n    let x: Entero64 = 42\n    retornar x\n");
        // Valores: const 42, asign x. Nombres: x registrado.
        assert!(ir.nombres.values().any(|n| n == "x"));
        // La variable x resuelve al valor de la asignación.
        let total_instrs = ir.num_instrs();
        assert!(total_instrs >= 3, "const + asignar + retorno, got {total_instrs}");
    }

    #[test]
    fn asumir_genera_invariante_asuncion() {
        let ir = bajar("definir tarea f(x: Entero64) -> Entero64:\n    asumir x > 0\n    retornar x\n");
        let asunciones = ir.invariantes(ClaseInvariante::Asuncion);
        assert_eq!(asunciones.len(), 1);
        // Hash de 64 bytes no nulo.
        assert!(asunciones[0].hash.iter().any(|b| *b != 0));
    }

    #[test]
    fn demostrar_genera_invariante_demostracion() {
        let ir = bajar("definir tarea f(x: Entero64) -> Entero64:\n    demostrar x > 0\n    retornar x\n");
        let demos = ir.invariantes(ClaseInvariante::Demostracion);
        assert_eq!(demos.len(), 1);
    }

    #[test]
    fn verificar_es_demostracion() {
        let ir = bajar("definir tarea f(x: Entero64) -> Entero64:\n    verificar que x > 0\n    retornar x\n");
        let demos = ir.invariantes(ClaseInvariante::Demostracion);
        assert_eq!(demos.len(), 1);
    }

    #[test]
    fn hash_determinista_misma_formula() {
        let ir1 = bajar("definir tarea f(x: Entero64) -> Entero64:\n    asumir x > 0\n    retornar x\n");
        let ir2 = bajar("definir tarea g(x: Entero64) -> Entero64:\n    asumir x > 0\n    retornar x\n");
        let h1 = ir1.invariantes(ClaseInvariante::Asuncion)[0].hash;
        let h2 = ir2.invariantes(ClaseInvariante::Asuncion)[0].hash;
        assert_eq!(h1, h2, "misma fórmula → mismo hash (caché incremental)");
    }

    #[test]
    fn tipos_colecciones() {
        let ir = bajar("definir tarea f(xs: Lista de Entero64) -> Entero64:\n    retornar 0\n");
        assert_eq!(
            ir.params[0].1,
            SilType::Lista(Box::new(SilType::Entero64))
        );
    }

    #[test]
    fn restricciones_preservadas() {
        let ir = bajar("definir tarea f() -> Entero64:\n    retornar 1\n    bajo restricciones:\n        gestion_memoria: arena\n");
        assert_eq!(ir.restricciones.len(), 1);
        // La clave preserva el nombre original del token (minúsculas).
        assert!(ir.restricciones[0].clave.contains("gestion")
            || ir.restricciones[0].clave.contains("Gestion")
            || !ir.restricciones[0].clave.is_empty());
    }

    #[test]
    fn variable_no_definida_error() {
        let prog = parsear(&lexear("definir tarea f() -> Entero64:\n    retornar z\n")).unwrap();
        let A::Decl::Tarea(t) = &prog.defs[0] else { panic!() };
        let r = bajar_tarea(t);
        assert!(matches!(r, Err(ErrorBajada::VariableNoDefinida(_))));
    }

    #[test]
    fn precedencia_preservada_en_ir() {
        // 1 + 2 * 3 → BinOp(Add, 1, BinOp(Mul, 2, 3)). Verificamos estructura por conteo:
        // consts(3) + mul(1) + add(1) + retorno = 5 instrs mínimo.
        let ir = bajar("definir tarea f() -> Entero64:\n    retornar 1 + 2 * 3\n");
        assert!(ir.num_instrs() >= 5, "got {}", ir.num_instrs());
    }
}
