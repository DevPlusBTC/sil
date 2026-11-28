//! Backend Rust: Genera código Rust idiomático legible para humanos.
//!
//! Este backend es **opcional** y solo se activa para auditorías y visualización.
//! El compilador principal genera C99 (ultra-optimizado) y WASM; este genera
//! código Rust para que humanos puedan entender la lógica generada.
//!
//! No se usa en producción: las precondiciones pueden inspeccionarse como
//! código Rust estándar con tipos explícitos.

use silc_causal_ir::nodes::{
    BlockId, Constante, InstrCausal, OpArit, OpCmp, Operacion, SilType, TareaIR, Terminador,
    ValueId,
};
use slotmap::Key;
use std::collections::HashMap;

pub fn emitir_tarea(tarea: &TareaIR) -> String {
    let mut output = Vec::new();

    output.push("// SIL-UD convertido a Rust para auditoría".to_string());
    output.push(format!("// Tarea: {}", tarea.nombre));

    let params: Vec<String> = tarea
        .params
        .iter()
        .map(|(nombre, t)| format!("{}: {}", nombre, tipo_a_rust(t)))
        .collect();
    let retorno = tipo_a_rust(&tarea.retorno);
    output.push(format!(
        "pub fn {}({}) -> {} {{",
        tarea.nombre,
        params.join(", "),
        retorno
    ));

    if tarea.bloques.len() <= 1 {
        for (_, bloque) in tarea.bloques.iter() {
            for instr in &bloque.instrs {
                generar_instruccion(instr, &tarea.nombres, "    ", false, &mut output);
            }
            emitir_terminador(
                &bloque.terminador,
                &tarea.nombres,
                None,
                "    ",
                &mut output,
            );
        }
    } else {
        // Multi-bloque (si/sino/mientras): declarar temporales SSA arriba y despachar vía loop+match.
        let mut orden: Vec<(BlockId, usize)> = Vec::with_capacity(tarea.bloques.len());
        let mut idx_map: HashMap<BlockId, usize> = HashMap::new();
        idx_map.insert(tarea.entrada, 0);
        orden.push((tarea.entrada, 0));
        for (bid, _) in tarea.bloques.iter() {
            let idx = orden.len();
            let entry = idx_map.entry(bid);
            if let std::collections::hash_map::Entry::Vacant(e) = entry {
                e.insert(idx);
                orden.push((bid, idx));
            }
        }

        // Pre-declarar resultados SSA para que crucen límites de bloque.
        let mut declarados = std::collections::HashSet::new();
        for (nombre, _) in &tarea.params {
            declarados.insert(nombre.clone());
        }
        for (_, bloque) in tarea.bloques.iter() {
            for instr in &bloque.instrs {
                if let Some(r) = instr.resultado {
                    if matches!(instr.op, Operacion::Param { .. }) {
                        continue;
                    }
                    let nombre = nombre_valor(r, &tarea.nombres);
                    if declarados.insert(nombre.clone()) {
                        let ty = tarea
                            .tipos
                            .get(&r)
                            .map(tipo_a_rust)
                            .unwrap_or_else(|| "i64".to_string());
                        output.push(format!("    let mut {nombre}: {ty} = Default::default();"));
                    }
                }
            }
        }

        output.push("    let mut __bloque: usize = 0;".to_string());
        output.push("    loop {".to_string());
        output.push("        match __bloque {".to_string());
        for (bid, idx) in &orden {
            let bloque = &tarea.bloques[*bid];
            output.push(format!("            {idx} => {{"));
            for instr in &bloque.instrs {
                generar_instruccion(instr, &tarea.nombres, "                ", true, &mut output);
            }
            emitir_terminador(
                &bloque.terminador,
                &tarea.nombres,
                Some(&idx_map),
                "                ",
                &mut output,
            );
            output.push("            }".to_string());
        }
        output.push("            _ => unreachable!(),".to_string());
        output.push("        }".to_string());
        output.push("    }".to_string());
    }

    output.push("}".to_string());
    output.join("\n")
}

fn emitir_terminador(
    term: &Terminador,
    nombres: &HashMap<ValueId, String>,
    idx_map: Option<&HashMap<BlockId, usize>>,
    indent: &str,
    output: &mut Vec<String>,
) {
    match term {
        Terminador::Retorno(v) => {
            let valor = v
                .as_ref()
                .map(|vid| nombre_valor(*vid, nombres))
                .unwrap_or_else(|| "()".to_string());
            output.push(format!("{indent}return {valor};"));
        }
        Terminador::Salto(dst) => {
            if let Some(map) = idx_map {
                let idx = map[dst];
                output.push(format!("{indent}__bloque = {idx};"));
            }
        }
        Terminador::Rama {
            cond,
            entonces,
            sino,
        } => {
            if let Some(map) = idx_map {
                let cond_str = nombre_valor(*cond, nombres);
                let idx_then = map[entonces];
                let idx_else = map[sino];
                output.push(format!("{indent}if {cond_str} {{"));
                output.push(format!("{indent}    __bloque = {idx_then};"));
                output.push(format!("{indent}}} else {{"));
                output.push(format!("{indent}    __bloque = {idx_else};"));
                output.push(format!("{indent}}}"));
            }
        }
        Terminador::Inalcanzable => {
            output.push(format!("{indent}unreachable!();"));
        }
    }
}

fn nombre_valor(vid: ValueId, nombres: &HashMap<ValueId, String>) -> String {
    nombres
        .get(&vid)
        .cloned()
        .unwrap_or_else(|| format!("v{}", vid.data().as_ffi()))
}

fn generar_instruccion(
    instr: &InstrCausal,
    nombres: &HashMap<ValueId, String>,
    indent: &str,
    predeclarado: bool,
    output: &mut Vec<String>,
) {
    let Some(r) = instr.resultado else {
        return;
    };
    let nombre_r = nombre_valor(r, nombres);

    match &instr.op {
        Operacion::Const { valor, tipo } => {
            let valor_str = match valor {
                Constante::Int(n) => n.to_string(),
                Constante::Float(f) => f.to_string(),
                Constante::Bool(b) => b.to_string(),
                Constante::Texto(t) => format!("\"{}\".to_string()", t.escape_default()),
            };
            if predeclarado {
                output.push(format!("{indent}{nombre_r} = {valor_str};"));
            } else {
                output.push(format!(
                    "{indent}let mut {nombre_r}: {} = {valor_str};",
                    tipo_a_rust(tipo)
                ));
            }
        }
        Operacion::AsignarArena { valor, .. } => {
            let valor_str = nombre_valor(*valor, nombres);
            if predeclarado {
                output.push(format!("{indent}{nombre_r} = {valor_str};"));
            } else {
                output.push(format!("{indent}let {nombre_r} = {valor_str};"));
            }
        }
        Operacion::BinOpSegura { op, lhs, rhs, .. } => {
            let lhs_str = nombre_valor(*lhs, nombres);
            let rhs_str = nombre_valor(*rhs, nombres);
            let op_str = match op {
                OpArit::Add => "+",
                OpArit::Sub => "-",
                OpArit::Mul => "*",
                OpArit::Div => "/",
            };
            if predeclarado {
                output.push(format!(
                    "{indent}{nombre_r} = {lhs_str} {op_str} {rhs_str};"
                ));
            } else {
                output.push(format!(
                    "{indent}let mut {nombre_r} = {lhs_str} {op_str} {rhs_str};"
                ));
            }
        }
        Operacion::Comparar { op, lhs, rhs, .. } => {
            let lhs_str = nombre_valor(*lhs, nombres);
            let rhs_str = nombre_valor(*rhs, nombres);
            let op_str = match op {
                OpCmp::Gt => ">",
                OpCmp::Lt => "<",
                OpCmp::Eq => "==",
                OpCmp::Ne => "!=",
                OpCmp::Ge => ">=",
                OpCmp::Le => "<=",
            };
            if predeclarado {
                output.push(format!(
                    "{indent}{nombre_r} = {lhs_str} {op_str} {rhs_str};"
                ));
            } else {
                output.push(format!(
                    "{indent}let {nombre_r}: bool = {lhs_str} {op_str} {rhs_str};"
                ));
            }
        }
        _ => {}
    }
}

fn tipo_a_rust(t: &SilType) -> String {
    match t {
        SilType::Entero64 => "i64".to_string(),
        SilType::Flotante64 => "f64".to_string(),
        SilType::Booleano => "bool".to_string(),
        SilType::Texto => "String".to_string(),
        SilType::Void => "()".to_string(),
        SilType::USD => "USD".to_string(),
        SilType::EUR => "EUR".to_string(),
        SilType::Lista(_) => "Vec<_>".to_string(),
        SilType::Mapa(_, _) => "HashMap<_, _>".to_string(),
        SilType::Conjunto(_) => "HashSet<_>".to_string(),
        SilType::Tupla(_) => "(_, ...)".to_string(),
        SilType::CapacidadHardware => "HardwareCapability".to_string(),
        SilType::Nominal(n) => n.clone(),
    }
}

pub fn tipo_rust(t: &SilType) -> String {
    tipo_a_rust(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use silc_causal_ir::lower::bajar_tarea;
    use silc_frontend::a_ast::parsear;

    fn emitir_src(src: &str) -> String {
        let prog = parsear(src).expect("parse OK");
        let silc_frontend::ast::Decl::Tarea(t) = &prog.defs[0] else {
            panic!()
        };
        let ir = bajar_tarea(t).expect("lower OK");
        emitir_tarea(&ir)
    }

    #[test]
    fn emite_funcion_plana_sin_ramas() {
        let r = emitir_src(
            "(task sumar
  (params (a i64) (b i64))
  (returns i64)
  (body (return (+ a b))))",
        );
        assert!(r.contains("pub fn sumar(a: i64, b: i64) -> i64 {"));
        assert!(!r.contains("__bloque"), "no debe emitir loop en 1 bloque");
        assert!(r.contains("return "));
    }

    #[test]
    fn emite_control_de_flujo_en_ramas() {
        let r = emitir_src(
            "(task abs
  (params (x i64))
  (returns i64)
  (body
    (if (>= x 0)
      (body (return x))
      (else (body (return (- 0 x)))))))",
        );
        assert!(r.contains("loop {"), "debe emitir dispatch loop:\n{r}");
        assert!(r.contains("if "), "debe emitir condición if:\n{r}");
        assert!(
            !r.contains("debug_assert!"),
            "Comparar no debe ser debug_assert:\n{r}"
        );
    }
}
