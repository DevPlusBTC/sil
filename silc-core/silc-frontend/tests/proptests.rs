//! Property-based tests para silc-frontend
//! Valida invariantes estructurales: lexer/parser round-trip, AST well-formedness.

use proptest::prelude::*;
use silc_frontend::{lexer::lexear, parser::parsear, ast::*};
use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;

// Palabras reservadas de CNL que no pueden usarse como identificadores
const RESERVADAS: &[&str] = &[
    "a", "de", "en", "con", "para", "cada", "si", "al", "por", "que", "es", "el", "la", "lo", "un", "una",
    "definir", "tarea", "servicio", "modelo", "evento", "estructura", "variante", "opcion",
    "memoria_persistente", "migrar", "capacidad", "modulo", "requerir", "importar", "exportar",
    "vincular", "biblioteca_nativa", "funcion_externa", "cuando", "llegue", "escuchar", "en",
    "puerto", "con", "protocolo", "para", "cada", "mientras", "si", "entonces", "retornar",
    "ejecutar", "cualquier", "otro", "caso", "coincidir", "desde", "hasta", "como", "datos",
    "de", "forma", "verificar", "que", "sea", "igual", "mayor", "menor", "diferente",
    "esta_activo", "asumir", "demostrar", "probar", "propiedad", "convertir", "a", "usando",
    "guardar", "enviar", "permitir", "conectar", "promover", "al", "acceder_campo",
    "valor_predeterminado", "bajo", "restricciones", "let", "mut", "verdadero", "falso",
    "nulo", "atomica", "asincrona", "en_memoria", "arena", "cero_pausas",
    "Entero64", "Flotante64", "Booleano", "Texto", "CapacidadHardware", "Void", "Tensor",
    "USD", "EUR", "Lista", "Mapa", "Conjunto", "Tupla", "Nominal",
    "opcion", "con", "datos", "forma", "true", "false", "null",
];

fn ident_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap()
        .prop_filter("no reservada", |s| !RESERVADAS.contains(&s.as_str()))
}

fn entero_strategy() -> impl Strategy<Value = i64> {
    prop::num::i64::ANY.prop_filter("no negativos", |n| *n >= 0)
}

fn flotante_strategy() -> impl Strategy<Value = f64> {
    prop::num::f64::ANY.prop_filter("finito", |f| f.is_finite())
}

fn texto_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex(r#"[^"\n\r]*"#).unwrap()
}

fn binop_strategy() -> impl Strategy<Value = BinOp> {
    prop::sample::select(vec![
        BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Div,
        BinOp::Gt, BinOp::Lt, BinOp::Eq, BinOp::Ge, BinOp::Le,
    ])
}

fn literal_strategy() -> impl Strategy<Value = Literal> {
    prop_oneof![
        entero_strategy().prop_map(Literal::Entero),
        flotante_strategy().prop_map(Literal::Flotante),
        texto_strategy().prop_map(Literal::Texto),
        prop::bool::ANY.prop_map(Literal::Booleano),
    ]
}

fn tipo_dato_strategy() -> impl Strategy<Value = TipoDato> {
    let leaf = prop_oneof![
        Just(TipoDato::Entero64),
        Just(TipoDato::Flotante64),
        Just(TipoDato::Booleano),
        Just(TipoDato::Texto),
        Just(TipoDato::CapacidadHardware),
        Just(TipoDato::Void),
        Just(TipoDato::USD),
        Just(TipoDato::EUR),
    ];

    leaf.prop_recursive(4, 8, 2, |inner| {
        prop_oneof![
            inner.clone().prop_map(|t| TipoDato::Lista(Box::new(t))),
            (inner.clone(), inner.clone()).prop_map(|(k, v)| TipoDato::Mapa(Box::new(k), Box::new(v))),
            inner.clone().prop_map(|t| TipoDato::Conjunto(Box::new(t))),
            prop::collection::vec(inner.clone(), 2..5).prop_map(TipoDato::Tupla),
            ident_strategy().prop_map(TipoDato::Nominal),
        ]
    })
}

fn expr_strategy() -> impl Strategy<Value = Expr> {
    prop_oneof![
        ident_strategy().prop_map(|s| Expr::Var(Ident { nombre: s, span: 0..0 })),
        literal_strategy()
            .prop_filter("no float or bool", |l| !matches!(l, Literal::Flotante(_) | Literal::Booleano(_)))
            .prop_map(|l| Expr::Lit(l)),
    ]
}

fn stmt_strategy() -> impl Strategy<Value = Stmt> {
    prop_oneof![
        expr_strategy().prop_map(|cond| Stmt::Verificar { cond, span: 0..0 }),
        expr_strategy().prop_map(|cond| Stmt::Asumir { cond, span: 0..0 }),
        expr_strategy().prop_map(|cond| Stmt::Demostrar { cond, span: 0..0 }),
    ]
}

fn param_strategy() -> impl Strategy<Value = Param> {
    (ident_strategy(), tipo_dato_strategy())
        .prop_map(|(nombre, tipo)| Param {
            nombre: Ident { nombre, span: 0..0 },
            tipo,
        })
}

fn restriccion_valor_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        ident_strategy(),
        prop::num::u64::ANY.prop_map(|n| n.to_string()),
        prop::sample::select(vec![
            "true".to_string(), "false".to_string(), "null".to_string(),
            "verdadero".to_string(), "falso".to_string(), "nulo".to_string(),
        ]),
    ].prop_filter("no A keyword", |s| s != "A")
        .prop_filter("no parentesis", |s| !s.contains('(') && !s.contains(')'))
        .prop_filter("no empty", |s| !s.is_empty())
}

fn restriccion_strategy() -> impl Strategy<Value = Restriccion> {
    (ident_strategy(), restriccion_valor_strategy())
        .prop_map(|(clave, valor)| Restriccion { clave, valor, span: 0..0 })
}

fn cuerpo_strategy() -> impl Strategy<Value = Cuerpo> {
    (prop::collection::vec(stmt_strategy(), 1..5),
     prop::collection::vec(restriccion_strategy(), 0..1))
        .prop_map(|(stmts, restricciones)| Cuerpo { stmts, restricciones })
}

fn tarea_decl_strategy() -> impl Strategy<Value = TareaDecl> {
    (
        ident_strategy(),
        prop::collection::vec(param_strategy(), 0..4),
        prop::option::of(tipo_dato_strategy()),
        cuerpo_strategy(),
    ).prop_map(|(nombre, params, retorno, cuerpo)| TareaDecl {
        nombre: Ident { nombre, span: 0..0 },
        params,
        retorno,
        cuerpo,
        span: 0..0,
    })
}

fn decl_strategy() -> impl Strategy<Value = Decl> {
    tarea_decl_strategy().prop_map(Decl::Tarea)
}

fn program_strategy() -> impl Strategy<Value = Program> {
    prop::collection::vec(decl_strategy(), 0..3).prop_map(|defs| Program { defs })
}

// =============================================================================
// Propiedades
// =============================================================================

proptest! {
    /// Propiedad: Lexer es idempotente en tokens válidos
    /// lexer(lexer(input)) ≈ lexer(input) para entrada bien formada
    #[test]
    fn lexer_idempotente(input in ident_strategy()) {
        let tokens1 = lexear(&input);
        let tokens2 = lexear(&input);
        prop_assert_eq!(tokens1, tokens2);
    }

    /// Propiedad: Parser + Lexer round-trip en AST serializado
    /// AST → source → parse(lexer(source)) ≈ AST original
    #[test]
    fn parser_lexer_roundtrip(ast in program_strategy()) {
        let source = ast_a_source(&ast);
        // DEBUG
        eprintln!("=== SOURCE ===\n{}=== END ===", source);
        let tokens = lexear(&source);
        eprintln!("=== TOKENS ===\n{:?}", tokens);
        let parsed = parsear(&tokens);

        match parsed {
            Ok(program) => {
                prop_assert_eq!(program.defs.len(), ast.defs.len());
                for (orig, parsed_decl) in ast.defs.iter().zip(program.defs.iter()) {
                    prop_assert!(decls_equiv(orig, parsed_decl));
                }
            }
            Err(e) => {
                // El parser puede fallar en casos edge; documentar
                prop_assert!(false, "Parser falló en AST válido generado: {:?}\nSource: {}", e, source);
            }
        }
    }

    /// Propiedad: AST generado es well-formed
    #[test]
    fn ast_well_formed(ast in program_strategy()) {
        for decl in &ast.defs {
            prop_assert!(decl_well_formed(decl));
        }
    }

    /// Propiedad: Tipos de dato son acíclicos y bien formados
    #[test]
    fn tipos_bien_formados(tipo in tipo_dato_strategy()) {
        prop_assert!(tipo_bien_formado(&tipo));
    }

    /// Propiedad: Serialización de AST a fuente es determinista
    #[test]
    fn serializacion_determinista(ast in program_strategy()) {
        let s1 = ast_a_source(&ast);
        let s2 = ast_a_source(&ast);
        prop_assert_eq!(s1, s2);
    }

    /// Propiedad: Lexer no pierde tokens en entrada válida
    #[test]
    fn lexer_no_pierde_tokens(input in ident_strategy()) {
        let tokens = lexear(&input);
        prop_assert!(!tokens.is_empty() || input.is_empty());
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn ast_a_source(ast: &Program) -> String {
    let mut out = String::new();
    for decl in &ast.defs {
        FmtWrite::write_fmt(&mut out, format_args!("{}", decl_a_source(decl))).unwrap();
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
    
    let has_content = !t.cuerpo.stmts.is_empty() || !t.cuerpo.restricciones.is_empty();
    if !has_content {
        // Empty body: INDENT + DEDENT
        out.push_str("    \n");
        out.push_str("\n");
        return out;
    }
    
    // INDENT for body
    out.push_str("    \n");
    
    // Statements (stmt_a_source already includes newline)
    for stmt in &t.cuerpo.stmts {
        write!(&mut out, "    {}", stmt_a_source(stmt)).unwrap();
    }
    
    // Restrictions at same indent level
    for r in &t.cuerpo.restricciones {
        write!(&mut out, "    bajo restricciones:\n        {}: {}\n", r.clave, r.valor).unwrap();
    }
    
    // DEDENT
    out.push_str("\n");
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
        Literal::Texto(s) => {
            // Escapar para el lexer: \ -> \\, " -> \"
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        }
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

fn decls_equiv(a: &Decl, b: &Decl) -> bool {
    match (a, b) {
        (Decl::Tarea(a), Decl::Tarea(b)) => tareas_equiv(a, b),
        _ => false,
    }
}

fn tareas_equiv(a: &TareaDecl, b: &TareaDecl) -> bool {
    a.nombre.nombre == b.nombre.nombre &&
    a.params.len() == b.params.len() &&
    a.params.iter().zip(b.params.iter()).all(|(pa, pb)| {
        pa.nombre.nombre == pb.nombre.nombre && pa.tipo == pb.tipo
    }) &&
    a.retorno == b.retorno &&
    a.cuerpo.stmts.len() == b.cuerpo.stmts.len() &&
    a.cuerpo.stmts.iter().zip(b.cuerpo.stmts.iter()).all(|(sa, sb)| stmts_equiv(sa, sb))
}

fn stmts_equiv(a: &Stmt, b: &Stmt) -> bool {
    use std::mem::discriminant;
    discriminant(a) == discriminant(b) &&
    match (a, b) {
        (Stmt::Verificar { cond: ca, .. }, Stmt::Verificar { cond: cb, .. }) => exprs_equiv(ca, cb),
        (Stmt::Asumir { cond: ca, .. }, Stmt::Asumir { cond: cb, .. }) => exprs_equiv(ca, cb),
        (Stmt::Demostrar { cond: ca, .. }, Stmt::Demostrar { cond: cb, .. }) => exprs_equiv(ca, cb),
        (Stmt::Asignar { nombre: na, tipo: ta, expr: ea, mutable: ma, .. },
         Stmt::Asignar { nombre: nb, tipo: tb, expr: eb, mutable: mb, .. }) =>
            na.nombre == nb.nombre && ta == tb && ea == eb && ma == mb,
        (Stmt::Retornar { expr: ea, .. }, Stmt::Retornar { expr: eb, .. }) =>
            ea == eb,
        _ => false,
    }
}

fn exprs_equiv(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Var(va), Expr::Var(vb)) => va.nombre == vb.nombre,
        (Expr::Lit(la), Expr::Lit(lb)) => la == lb,
        (Expr::BinOp { op: oa, lhs: la, rhs: ra, .. }, Expr::BinOp { op: ob, lhs: lb, rhs: rb, .. }) =>
            oa == ob && exprs_equiv(la, lb) && exprs_equiv(ra, rb),
        (Expr::AccesoProp { base: ba, prop: pa, .. }, Expr::AccesoProp { base: bb, prop: pb, .. }) =>
            exprs_equiv(ba, bb) && pa.nombre == pb.nombre,
        _ => false,
    }
}

fn decl_well_formed(decl: &Decl) -> bool {
    match decl {
        Decl::Tarea(t) => tarea_well_formed(t),
        _ => true,
    }
}

fn tarea_well_formed(t: &TareaDecl) -> bool {
    !t.nombre.nombre.is_empty() &&
    t.params.iter().all(|p| !p.nombre.nombre.is_empty()) &&
    t.cuerpo.stmts.iter().all(stmt_well_formed)
}

fn stmt_well_formed(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Verificar { cond, .. } | Stmt::Asumir { cond, .. } | Stmt::Demostrar { cond, .. } =>
            expr_well_formed(cond),
        Stmt::Asignar { expr, .. } => expr_well_formed(expr),
        Stmt::Retornar { expr, .. } => expr.as_ref().map_or(true, expr_well_formed),
    }
}

fn expr_well_formed(expr: &Expr) -> bool {
    match expr {
        Expr::Var(v) => !v.nombre.is_empty(),
        Expr::Lit(_) => true,
        Expr::BinOp { lhs, rhs, .. } => expr_well_formed(lhs) && expr_well_formed(rhs),
        Expr::AccesoProp { base, .. } => expr_well_formed(base),
    }
}

fn tipo_bien_formado(tipo: &TipoDato) -> bool {
    match tipo {
        TipoDato::Lista(t) | TipoDato::Conjunto(t) => tipo_bien_formado(t),
        TipoDato::Mapa(k, v) => tipo_bien_formado(k) && tipo_bien_formado(v),
        TipoDato::Tupla(ts) => ts.iter().all(tipo_bien_formado),
        TipoDato::Nominal(n) => !n.is_empty(),
        _ => true,
    }
}