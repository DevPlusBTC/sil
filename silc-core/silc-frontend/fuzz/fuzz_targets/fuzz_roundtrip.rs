#![no_main]

use libfuzzer_sys::fuzz_target;
use silc_frontend::{lexer::lexear, parser::parsear, ast::*};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let tokens = lexear(s);
        if let Ok(ast) = parsear(&tokens) {
            // Round-trip: AST -> source -> parse(lexer(source))
            let source = ast_a_source(&ast);
            let tokens2 = lexear(&source);
            let _ = parsear(&tokens2);
        }
    }
});

fn ast_a_source(ast: &Program) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for decl in &ast.defs {
        write!(&mut out, "{}", decl_a_source(decl)).unwrap();
        out.push('\n');
    }
    out
}

fn decl_a_source(decl: &Decl) -> String {
    match decl {
        Decl::Tarea(t) => tarea_a_source(t),
        _ => "/* decl no soportado */".to_string(),
    }
}

fn tarea_a_source(t: &TareaDecl) -> String {
    let mut out = format!("definir tarea {}(", t.nombre.nombre);
    for (i, p) in t.params.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        write!(&mut out, "{}: {}", p.nombre.nombre, tipo_a_source(&p.tipo)).unwrap();
    }
    out.push(')');
    if let Some(ret) = &t.retorno {
        write!(&mut out, " -> {}", tipo_a_source(ret)).unwrap();
    }
    out.push_str(":\n");
    out.push_str("    \n");  // INDENT (cuerpo vacío)
    for stmt in &t.cuerpo.stmts {
        write!(&mut out, "    {}\n", stmt_a_source(stmt)).unwrap();
    }
    for r in &t.cuerpo.restricciones {
        write!(&mut out, "    bajo restricciones:\n        {}: {}\n", r.clave, r.valor).unwrap();
    }
    out
}

fn stmt_a_source(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Verificar { cond, .. } => format!("verificar que {}\n", expr_a_source(cond)),
        Stmt::Asumir { cond, .. } => format!("asumir {}\n", expr_a_source(cond)),
        Stmt::Demostrar { cond, .. } => format!("demostrar {}\n", expr_a_source(cond)),
        Stmt::Asignar { nombre, tipo, expr, mutable, .. } => {
            let mut s = if *mutable { "let mut " } else { "let " }.to_string();
            s.push_str(&nombre.nombre);
            if let Some(t) = tipo { write!(&mut s, ": {}", tipo_a_source(t)).unwrap(); }
            write!(&mut s, " = {}", expr_a_source(expr)).unwrap();
            s.push('\n');
            s
        }
        Stmt::Retornar { expr, .. } => {
            if let Some(e) = expr {
                format!("retornar {}\n", expr_a_source(e))
            } else {
                "retornar\n".to_string()
            }
        }
    }
}

fn expr_a_source(expr: &Expr) -> String {
    match expr {
        Expr::Var(v) => v.nombre.clone(),
        Expr::Lit(l) => literal_a_source(l),
        Expr::BinOp { op, lhs, rhs, .. } => {
            let op_str = match op {
                BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*", BinOp::Div => "/",
                BinOp::Gt => ">", BinOp::Lt => "<", BinOp::Eq => "==", BinOp::Ne => "!=",
                BinOp::Ge => ">=", BinOp::Le => "<=",
            };
            format!("({} {} {})", expr_a_source(lhs), op_str, expr_a_source(rhs))
        }
        Expr::AccesoProp { base, prop, .. } => {
            format!("{}.{}", expr_a_source(base), prop.nombre)
        }
    }
}

fn literal_a_source(lit: &Literal) -> String {
    match lit {
        Literal::Entero(n) => n.to_string(),
        Literal::Flotante(f) => f.to_string(),
        Literal::Texto(s) => format!("\"{}\"", s),
        Literal::Booleano(b) => b.to_string(),
    }
}

fn tipo_a_source(t: &TipoDato) -> String {
    match t {
        TipoDato::Entero64 => "Entero64".to_string(),
        TipoDato::Flotante64 => "Flotante64".to_string(),
        TipoDato::Booleano => "Booleano".to_string(),
        TipoDato::Texto => "Texto".to_string(),
        TipoDato::CapacidadHardware => "CapacidadHardware".to_string(),
        TipoDato::Void => "Void".to_string(),
        TipoDato::USD => "USD".to_string(),
        TipoDato::EUR => "EUR".to_string(),
        TipoDato::Lista(t) => format!("Lista de {}", tipo_a_source(t)),
        TipoDato::Mapa(k, v) => format!("Mapa de {} a {}", tipo_a_source(k), tipo_a_source(v)),
        TipoDato::Conjunto(t) => format!("Conjunto de {}", tipo_a_source(t)),
        TipoDato::Tupla(ts) => {
            let mut s = "Tupla(".to_string();
            for (i, t) in ts.iter().enumerate() {
                if i > 0 { s.push_str(", "); }
                s.push_str(&tipo_a_source(t));
            }
            s.push(')');
            s
        }
        TipoDato::Nominal(n) => n.clone(),
    }
}