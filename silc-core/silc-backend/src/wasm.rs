//! Emisión WASM/WASI vía wasm-encoder (binario determinista).
//!
//! Garantías:
//! - Output es WASM binario válido (verificado con `wasmparser` en tests).
//! - Memoria importada del host ("sil.memory", 1 página inicial): así el
//!   start puede usar el segmento de datos durante la instanciación.
//! - Imports WASI: `proc_exit`, `fd_write`.
//! - Imports host `sil.*` (solo los que el programa usa): capacidades con
//!   HMAC, canales, E/S por archivo y red. El host de referencia viaja con
//!   el CLI (`sil_host.js` junto al .wasm).
//! - Funciones exportadas por nombre (índice real, no el de un import).
//! - Tipos: Entero64→i64, Flotante64→f64, Booleano→i32, CapacidadHardware→i64
//!   (handle), Texto→i64 empaquetado (ptr32 alto + len32 bajo).
//! - Literales de texto en segmento de datos desde el offset 1024.
//! - Aritmética: add/sub/mul/div_s (i64), comparaciones → i32 (0/1).
//!
//! Alcance documentado:
//! - `generar` exige programa multi-tarea (el CLI lo ensambla; emitir()
//!   de una sola tarea da error limpio).
//! - Sin `si` en el lenguaje aún: Saltar/Ramificar explícitos dan error limpio.

use silc_causal_ir::nodes::{
    Constante, OpArit, OpCmp, Operacion, SilType, TareaIR, Terminador, ValueId,
};
use std::collections::HashMap;
use thiserror::Error;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, FunctionSection, ImportSection, MemoryType, Module,
    TypeSection, ValType,
};

#[derive(Debug, Error)]
pub enum ErrorWasm {
    #[error("Error WASM: {0}")]
    Wasm(String),
    #[error("Tipo no soportado en WASM M6: {0:?}")]
    TipoNoSoportado(String),
}

/// Host de referencia en Node.js para los módulos generados (imports "sil.*").
/// El CLI lo escribe junto al .wasm como `<salida>.host.js`.
pub const HOST_JS: &str = include_str!("sil_host.js");

fn valtype(t: &SilType) -> Result<ValType, ErrorWasm> {
    match t {
        SilType::Entero64 | SilType::USD | SilType::EUR | SilType::CapacidadHardware => {
            Ok(ValType::I64)
        }
        SilType::Flotante64 => Ok(ValType::F64),
        SilType::Booleano => Ok(ValType::I32),
        // Texto empaquetado en i64: ptr32 alto + len32 bajo.
        SilType::Texto => Ok(ValType::I64),
        SilType::Void => Err(ErrorWasm::TipoNoSoportado(
            "Void como valor (solo retorno)".into(),
        )),
        SilType::Lista(_)
        | SilType::Mapa(..)
        | SilType::Conjunto(_)
        | SilType::Tupla(_)
        | SilType::Nominal(_) => Err(ErrorWasm::TipoNoSoportado(format!("{t:?}"))),
    }
}

/// Empaqueta (ptr32, len32) en un i64 para valores Texto.
fn empaquetar_texto(ptr: u32, len: u32) -> i64 {
    ((ptr as i64) << 32) | (len as i64)
}

/// Recolecta literales de texto únicos del IR en orden determinista:
/// literales Const más recursos de `solicitar capacidad` (viajan al segmento).
fn recolectar_textos(tarea: &TareaIR) -> Vec<String> {
    let mut vistos = std::collections::HashSet::new();
    let mut textos = Vec::new();
    let mut emitir_texto = |s: &String| {
        if vistos.insert(s.clone()) {
            textos.push(s.clone());
        }
    };
    let mut bids: Vec<_> = tarea.bloques.keys().collect();
    bids.sort_by_key(|k| format!("{k:?}"));
    for bid in bids {
        let b = &tarea.bloques[bid];
        for ins in &b.instrs {
            if let Operacion::Const {
                valor: Constante::Texto(s),
                ..
            } = &ins.op
            {
                emitir_texto(s);
            }
            if let Operacion::SolicitarCapacidad { req } = &ins.op {
                if let Some(rec) = &req.recurso {
                    emitir_texto(rec);
                }
            }
        }
    }
    textos
}

/// ¿Usa el programa operaciones que exigen cada import host?
#[derive(Default)]
struct UsoHost {
    cap_solicitar: bool,
    cap_validar: bool,
    canal: bool,
    io_archivo: bool,
    red: bool,
}

fn detectar_uso(tarea: &TareaIR) -> UsoHost {
    let mut u = UsoHost::default();
    for (_, b) in tarea.bloques.iter() {
        for ins in &b.instrs {
            match &ins.op {
                Operacion::SolicitarCapacidad { .. } => {
                    u.cap_solicitar = true;
                }
                Operacion::ValidarCapacidad { .. } => {
                    u.cap_validar = true;
                }
                Operacion::CrearCanal { .. }
                | Operacion::EnviarCanal { .. }
                | Operacion::RecibirCanal { .. } => {
                    u.canal = true;
                }
                Operacion::InvocarIO { operacion, .. } => {
                    use silc_causal_ir::nodes::OpIO;
                    match operacion {
                        OpIO::LeerArchivo | OpIO::EscribirArchivo => {
                            u.io_archivo = true;
                        }
                        OpIO::EnviarRed | OpIO::RecibirRed => {
                            u.red = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    u
}

/// Emite instrucciones WASM que evalúan una fórmula SMT y dejan el resultado (i32: 0/1) en la pila.
/// Usa los locales ya asignados (locales: ValueId -> u32).
#[allow(clippy::only_used_in_recursion)]
fn emitir_formula_wasm(
    f: &mut wasm_encoder::Function,
    locales: &HashMap<ValueId, u32>,
    formula: &silc_causal_ir::nodes::FormulaLogica,
) -> Result<(), ErrorWasm> {
    use silc_causal_ir::nodes::{FormulaLogica, OpLogico};
    let src = |v: ValueId| -> Result<u32, ErrorWasm> {
        locales
            .get(&v)
            .copied()
            .ok_or_else(|| ErrorWasm::Wasm("operando sin local".into()))
    };
    match formula {
        FormulaLogica::Var(_) => {
            // Var(String) no debería aparecer en fórmulas con VarId; fallback a 0
            f.instruction(&wasm_encoder::Instruction::I32Const(0));
        }
        FormulaLogica::VarId(v) => {
            let idx = src(*v)?;
            f.instruction(&wasm_encoder::Instruction::LocalGet(idx));
        }
        FormulaLogica::ConstInt(n) => {
            f.instruction(&wasm_encoder::Instruction::I64Const(*n));
            // Para uso en comparaciones, necesitamos i32; asumimos que la fórmula maneja tipos
            // En M6, las fórmulas SMT son enteros; para booleanos usamos 0/1
        }
        FormulaLogica::ConstBool(b) => {
            f.instruction(&wasm_encoder::Instruction::I32Const(i32::from(*b)));
        }
        FormulaLogica::BinOp { op, lhs, rhs } => {
            match op {
                OpLogico::Gt
                | OpLogico::Lt
                | OpLogico::Eq
                | OpLogico::Ne
                | OpLogico::Ge
                | OpLogico::Le => {
                    // Comparación: evalúa lhs, rhs, luego instrucción de comparación → i32
                    emitir_formula_wasm(f, locales, lhs)?;
                    emitir_formula_wasm(f, locales, rhs)?;
                    let instr = match op {
                        OpLogico::Gt => wasm_encoder::Instruction::I64GtS,
                        OpLogico::Lt => wasm_encoder::Instruction::I64LtS,
                        OpLogico::Eq => wasm_encoder::Instruction::I64Eq,
                        OpLogico::Ne => wasm_encoder::Instruction::I64Ne,
                        OpLogico::Ge => wasm_encoder::Instruction::I64GeS,
                        OpLogico::Le => wasm_encoder::Instruction::I64LeS,
                        _ => unreachable!(),
                    };
                    f.instruction(&instr);
                }
                OpLogico::And => {
                    emitir_formula_wasm(f, locales, lhs)?;
                    emitir_formula_wasm(f, locales, rhs)?;
                    // Ambos ya son i32 (0/1); AND lógico = multiplicación o and
                    f.instruction(&wasm_encoder::Instruction::I32And);
                }
                OpLogico::Or => {
                    emitir_formula_wasm(f, locales, lhs)?;
                    emitir_formula_wasm(f, locales, rhs)?;
                    f.instruction(&wasm_encoder::Instruction::I32Or);
                }
                OpLogico::Add => {
                    emitir_formula_wasm(f, locales, lhs)?;
                    emitir_formula_wasm(f, locales, rhs)?;
                    f.instruction(&wasm_encoder::Instruction::I64Add);
                }
                OpLogico::Sub => {
                    emitir_formula_wasm(f, locales, lhs)?;
                    emitir_formula_wasm(f, locales, rhs)?;
                    f.instruction(&wasm_encoder::Instruction::I64Sub);
                }
                OpLogico::Mul => {
                    emitir_formula_wasm(f, locales, lhs)?;
                    emitir_formula_wasm(f, locales, rhs)?;
                    f.instruction(&wasm_encoder::Instruction::I64Mul);
                }
                OpLogico::Div => {
                    emitir_formula_wasm(f, locales, lhs)?;
                    emitir_formula_wasm(f, locales, rhs)?;
                    f.instruction(&wasm_encoder::Instruction::I64DivS);
                }
            }
        }
        FormulaLogica::No(x) => {
            emitir_formula_wasm(f, locales, x)?;
            f.instruction(&wasm_encoder::Instruction::I32Const(1));
            f.instruction(&wasm_encoder::Instruction::I32Xor); // not = xor 1
        }
    }
    Ok(())
}

/// Emite trap condicional: si la fórmula evalúa a 0 (falso), emite `unreachable`.
fn emitir_trap_si_falso(
    f: &mut wasm_encoder::Function,
    locales: &HashMap<ValueId, u32>,
    formula: &silc_causal_ir::nodes::FormulaLogica,
) -> Result<(), ErrorWasm> {
    // Evalúa fórmula → deja i32 en pila
    emitir_formula_wasm(f, locales, formula)?;
    // if (top == 0) { unreachable }
    f.instruction(&wasm_encoder::Instruction::I32Eqz); // 1 si era 0, 0 si era 1
    f.instruction(&wasm_encoder::Instruction::If(
        wasm_encoder::BlockType::Empty,
    ));
    f.instruction(&wasm_encoder::Instruction::Unreachable);
    f.instruction(&wasm_encoder::Instruction::End);
    Ok(())
}

/// Emite módulo WASM binario para una tarea.
pub fn emitir(tarea: &TareaIR) -> Result<Vec<u8>, ErrorWasm> {
    let mut module = Module::new();

    // --- 1. TypeSection: firma de la función ---
    let mut types = TypeSection::new();
    let mut params: Vec<ValType> = Vec::new();
    for (_, tipo) in &tarea.params {
        params.push(valtype(tipo)?);
    }
    let results: Vec<ValType> = match &tarea.retorno {
        SilType::Void => vec![],
        t => vec![valtype(t)?],
    };
    let type_idx = 0;
    types.function(params.clone(), results.clone());
    module.section(&types);

    // --- 2. ImportSection: WASI (proc_exit, fd_write) ---
    let mut imports = ImportSection::new();
    imports.import(
        "wasi_snapshot_preview1",
        "proc_exit",
        wasm_encoder::EntityType::Function(1),
    );
    // Declarar tipos importados también (índices 1, 2).
    // Tipo proc_exit: (i32) -> ()
    // Tipo fd_write: (i32,i32,i32,i32) -> (i32)
    {
        // Re-abrir types para añadir: necesitamos reconstruir. En su lugar,
        // declaramos todo al inicio. Simplificación M6: añadir tipos primero.
    }
    // M6: imports declarados pero tipos ya fijados arriba; para validez,
    // re-emitimos la sección de tipos completa aquí correctamente:
    // (wasm-encoder requiere orden: Type, Import, Function, ...).
    // Reconstruimos el módulo en orden correcto abajo.
    let _ = imports;
    let _ = type_idx;

    // Reconstrucción ordenada (Type → Import → Function → Memory → Export → Code).
    // Los imports ocupan los primeros índices de función; la tarea va después.
    let uso = detectar_uso(tarea);
    // Tipos host bajo "sil" (solo los que el programa usa).
    // Cada entrada: (nombre, params, results).
    let mut tipos_host: Vec<(&str, Vec<ValType>, Vec<ValType>)> = Vec::new();
    if uso.cap_solicitar {
        tipos_host.push((
            "sil_cap_solicitar",
            vec![ValType::I32, ValType::I32, ValType::I64, ValType::I64],
            vec![ValType::I64],
        ));
    }
    if uso.cap_validar {
        tipos_host.push((
            "sil_cap_validar",
            vec![ValType::I64, ValType::I32, ValType::I32, ValType::I64],
            vec![ValType::I32],
        ));
    }
    if uso.canal {
        tipos_host.push(("sil_canal_crear", vec![ValType::I64], vec![ValType::I64]));
        tipos_host.push((
            "sil_canal_enviar",
            vec![ValType::I64, ValType::I64],
            vec![ValType::I32],
        ));
        tipos_host.push(("sil_canal_recibir", vec![ValType::I64], vec![ValType::I64]));
    }
    if uso.io_archivo {
        tipos_host.push((
            "sil_io_leer",
            vec![ValType::I32, ValType::I32, ValType::I64],
            vec![ValType::I64],
        ));
        tipos_host.push((
            "sil_io_escribir",
            vec![
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I64,
            ],
            vec![ValType::I64],
        ));
    }
    if uso.red {
        tipos_host.push((
            "sil_red_enviar",
            vec![
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I64,
            ],
            vec![ValType::I64],
        ));
        tipos_host.push((
            "sil_red_recibir",
            vec![ValType::I32, ValType::I32, ValType::I64],
            vec![ValType::I64],
        ));
    }

    let mut module = Module::new();

    // Types: [0] = tarea, [1] = proc_exit, [2] = fd_write, [3..] = host.
    let mut types = TypeSection::new();
    types.function(params.clone(), results.clone());
    types.function([ValType::I32], []);
    types.function(
        [ValType::I32, ValType::I32, ValType::I32, ValType::I32],
        [ValType::I32],
    );
    for (_, p, r) in &tipos_host {
        types.function(p.clone(), r.clone());
    }
    module.section(&types);

    let mut imports = ImportSection::new();
    imports.import(
        "wasi_snapshot_preview1",
        "proc_exit",
        wasm_encoder::EntityType::Function(1),
    );
    imports.import(
        "wasi_snapshot_preview1",
        "fd_write",
        wasm_encoder::EntityType::Function(2),
    );
    // Índice de cada import host por nombre.
    let mut idx_host: HashMap<&str, u32> = HashMap::new();
    for (i, (nombre, _, _)) in tipos_host.iter().enumerate() {
        let idx_tipo = (3 + i) as u32;
        let idx_func = (2 + i) as u32;
        imports.import("sil", nombre, wasm_encoder::EntityType::Function(idx_tipo));
        idx_host.insert(*nombre, idx_func);
    }
    // Memoria: la provee el host ("sil.memory", 1 página inicial).
    // Así el start puede usar el segmento de datos durante la instanciación.
    imports.import(
        "sil",
        "memory",
        wasm_encoder::EntityType::Memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
        }),
    );
    module.section(&imports);

    // FunctionSection: nuestra función usa type 0; su índice real es 2 + hosts.
    let idx_tarea = (2 + tipos_host.len()) as u32;
    let mut funcs = FunctionSection::new();
    funcs.function(0);
    module.section(&funcs);

    // La memoria la provee el host (import "sil.memory" declarado arriba).

    // DataSection: literales de texto desde el offset 1024.
    // (Mismo orden de recolección que off_texto: el mapa ya existe.)
    let textos = recolectar_textos(tarea);
    let mut off_texto: HashMap<String, u32> = HashMap::new();
    {
        let mut off: u32 = 1024;
        for s in &textos {
            off_texto.insert(s.clone(), off);
            off += s.len() as u32;
        }
    }

    // ExportSection: función (índice real). La memoria es del host.
    let mut exports = ExportSection::new();
    exports.export(&tarea.nombre, ExportKind::Func, idx_tarea);
    module.section(&exports);

    // --- 3. CodeSection: cuerpo ---
    // M6 sound: CADA ValueId con resultado → un local WASM dedicado.
    // Params ocupan locals 0..n (son parámetros WASM). Intermedios ocupan
    // locals n..m (declarados explícitamente). Cero uso de pila entre
    // instrucciones: cada op hace local.get operandos → computa → local.set resultado.
    // Retorno: local.get del valor + end (la pila tiene exactamente el retorno).

    // Recolectar todos los ValueIds con resultado en orden determinista.
    // Params ocupan locals 0..n; intermedios después (ver locales_de).
    let (orden_vals, locales) = locales_de(tarea)?;

    let f = cuerpo_funcion(
        tarea,
        &orden_vals,
        &locales,
        &off_texto,
        &idx_host,
        &HashMap::new(),
        !results.is_empty(),
    )?;
    let mut code = CodeSection::new();
    code.function(&f);
    module.section(&code);

    // DataSection al final (orden WASM: Data tras Code).
    if !textos.is_empty() {
        use wasm_encoder::{DataSection, DataSegment, DataSegmentMode};
        let mut data = DataSection::new();
        for s in &textos {
            let off = off_texto[s];
            data.segment(DataSegment {
                mode: DataSegmentMode::Active {
                    memory_index: 0,
                    offset: &wasm_encoder::ConstExpr::i32_const(off as i32),
                },
                data: s.as_bytes().to_vec(),
            });
        }
        module.section(&data);
    }

    Ok(module.finish())
}

/// Emite un módulo WASM con todas las tareas del archivo + función start.
///
/// La última tarea es la principal. Si no tiene parámetros, la función start
/// la llama y descarta su retorno; si tiene parámetros, se emite una
/// biblioteca (sin start). `generar X` es llamada directa a X (cooperativo sin
/// desalojo, igual que el C99).
pub fn emitir_programa(tareas: &[TareaIR]) -> Result<Vec<u8>, ErrorWasm> {
    use silc_contracts::{require, require_valid_ident};
    require(!tareas.is_empty(), "programa sin tareas");
    let principal = tareas.last().expect("programa sin tareas");
    let con_start = principal.params.is_empty();

    // Uso host global + textos globales.
    let mut uso = UsoHost::default();
    let mut vistos = std::collections::HashSet::new();
    let mut textos: Vec<String> = Vec::new();
    for t in tareas {
        let u = detectar_uso(t);
        uso.cap_solicitar |= u.cap_solicitar;
        uso.cap_validar |= u.cap_validar;
        uso.canal |= u.canal;
        uso.io_archivo |= u.io_archivo;
        uso.red |= u.red;
        for s in recolectar_textos(t) {
            if vistos.insert(s.clone()) {
                textos.push(s);
            }
        }
    }
    let mut off_texto: HashMap<String, u32> = HashMap::new();
    {
        let mut off: u32 = 1024;
        for s in &textos {
            off_texto.insert(s.clone(), off);
            off += s.len() as u32;
        }
    }

    // Tipos host (igual que en emitir()).
    let mut tipos_host: Vec<(&str, Vec<ValType>, Vec<ValType>)> = Vec::new();
    if uso.cap_solicitar {
        tipos_host.push((
            "sil_cap_solicitar",
            vec![ValType::I32, ValType::I32, ValType::I64, ValType::I64],
            vec![ValType::I64],
        ));
    }
    if uso.cap_validar {
        tipos_host.push((
            "sil_cap_validar",
            vec![ValType::I64, ValType::I32, ValType::I32, ValType::I64],
            vec![ValType::I32],
        ));
    }
    if uso.canal {
        tipos_host.push(("sil_canal_crear", vec![ValType::I64], vec![ValType::I64]));
        tipos_host.push((
            "sil_canal_enviar",
            vec![ValType::I64, ValType::I64],
            vec![ValType::I32],
        ));
        tipos_host.push(("sil_canal_recibir", vec![ValType::I64], vec![ValType::I64]));
    }
    if uso.io_archivo {
        tipos_host.push((
            "sil_io_leer",
            vec![ValType::I32, ValType::I32, ValType::I64],
            vec![ValType::I64],
        ));
        tipos_host.push((
            "sil_io_escribir",
            vec![
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I64,
            ],
            vec![ValType::I64],
        ));
    }
    if uso.red {
        tipos_host.push((
            "sil_red_enviar",
            vec![
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I64,
            ],
            vec![ValType::I64],
        ));
        tipos_host.push((
            "sil_red_recibir",
            vec![ValType::I32, ValType::I32, ValType::I64],
            vec![ValType::I64],
        ));
    }

    // Firmas por tarea.
    let mut firmas: Vec<(Vec<ValType>, Vec<ValType>)> = Vec::new();
    for t in tareas {
        let mut params = Vec::new();
        for (_, tipo) in &t.params {
            params.push(valtype(tipo)?);
        }
        let results = match &t.retorno {
            SilType::Void => vec![],
            x => vec![valtype(x)?],
        };
        firmas.push((params, results));
    }

    let mut module = Module::new();
    let mut types = TypeSection::new();
    for (p, r) in &firmas {
        types.function(p.clone(), r.clone());
    }
    // Type de start: () -> ().
    let idx_type_start = firmas.len() as u32;
    types.function([], []);
    types.function([ValType::I32], []);
    types.function(
        [ValType::I32, ValType::I32, ValType::I32, ValType::I32],
        [ValType::I32],
    );
    for (_, p, r) in &tipos_host {
        types.function(p.clone(), r.clone());
    }
    module.section(&types);

    let n_wasi = 2u32;
    // Types: [firmas][start][proc_exit][fd_write][hosts...].
    let base_host = 3 + firmas.len() as u32;
    let mut imports = ImportSection::new();
    imports.import(
        "wasi_snapshot_preview1",
        "proc_exit",
        wasm_encoder::EntityType::Function(idx_type_start + 1),
    );
    imports.import(
        "wasi_snapshot_preview1",
        "fd_write",
        wasm_encoder::EntityType::Function(idx_type_start + 2),
    );
    let mut idx_host: HashMap<&str, u32> = HashMap::new();
    for (i, (nombre, _, _)) in tipos_host.iter().enumerate() {
        let idx_func = n_wasi + i as u32;
        imports.import(
            "sil",
            nombre,
            wasm_encoder::EntityType::Function(base_host + i as u32),
        );
        idx_host.insert(*nombre, idx_func);
    }
    // Memoria importada del host (ver emitir()).
    imports.import(
        "sil",
        "memory",
        wasm_encoder::EntityType::Memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
        }),
    );
    module.section(&imports);

    // Funciones: índice = n_imports + posición.
    let n_imports = n_wasi + tipos_host.len() as u32;
    let mut funcs = FunctionSection::new();
    for (i, _) in tareas.iter().enumerate() {
        funcs.function(i as u32);
    }
    // start usa el type start.
    if con_start {
        funcs.function(idx_type_start);
    }
    module.section(&funcs);

    let mut exports = ExportSection::new();
    for (i, t) in tareas.iter().enumerate() {
        exports.export(&t.nombre, ExportKind::Func, n_imports + i as u32);
    }
    module.section(&exports);

    // llamables: nombre → (índice, tiene retorno).
    let mut llamables: HashMap<String, (u32, bool)> = HashMap::new();
    for (i, t) in tareas.iter().enumerate() {
        llamables.insert(
            t.nombre.clone(),
            (n_imports + i as u32, !firmas[i].1.is_empty()),
        );
    }

    let mut code = CodeSection::new();
    // Validar nombres de tareas para el mapa (identificadores SANOS ya
    // verificados en lowering; aquí solo cuerdo el programa).
    for t in tareas {
        require_valid_ident(&t.nombre, 64, "identificador de tarea inválido");
    }
    for t in tareas {
        let (orden_vals, locales) = locales_de(t)?;
        let con_ret = !firmas[tareas.iter().position(|x| x.nombre == t.nombre).unwrap()]
            .1
            .is_empty();
        let f = cuerpo_funcion(
            t,
            &orden_vals,
            &locales,
            &off_texto,
            &idx_host,
            &llamables,
            con_ret,
        )?;
        code.function(&f);
    }
    // start: llama a la principal y descarta su retorno si lo hay.
    if con_start {
        let mut f = wasm_encoder::Function::new([]);
        let idx_p = llamables[&principal.nombre].0;
        f.instruction(&wasm_encoder::Instruction::Call(idx_p));
        if llamables[&principal.nombre].1 {
            f.instruction(&wasm_encoder::Instruction::Drop);
        }
        f.instruction(&wasm_encoder::Instruction::End);
        code.function(&f);
        // StartSection ANTES de Code (orden WASM: Start < Code < Data).
        use wasm_encoder::StartSection;
        let start = StartSection {
            function_index: n_imports + tareas.len() as u32,
        };
        module.section(&start);
    }

    module.section(&code);

    if !textos.is_empty() {
        use wasm_encoder::{DataSection, DataSegment, DataSegmentMode};
        let mut data = DataSection::new();
        for s in &textos {
            let off = off_texto[s];
            data.segment(DataSegment {
                mode: DataSegmentMode::Active {
                    memory_index: 0,
                    offset: &wasm_encoder::ConstExpr::i32_const(off as i32),
                },
                data: s.as_bytes().to_vec(),
            });
        }
        module.section(&data);
    }

    Ok(module.finish())
}

/// Locales de una tarea: valores intermedios ordenados + mapa a índice.
type LocalesTarea = (Vec<(ValueId, ValType)>, HashMap<ValueId, u32>);

/// Calcula (orden_vals, locales) de una tarea (extraído de emitir()).
fn locales_de(tarea: &TareaIR) -> Result<LocalesTarea, ErrorWasm> {
    let mut orden_vals: Vec<(ValueId, ValType)> = Vec::new();
    {
        let mut vistos = std::collections::HashSet::new();
        let mut bids: Vec<_> = tarea.bloques.keys().collect();
        bids.sort_by_key(|k| format!("{k:?}"));
        for bid in bids {
            let b = &tarea.bloques[bid];
            for ins in &b.instrs {
                if let Some(v) = ins.resultado {
                    if vistos.insert(v) {
                        let t = tarea.tipos.get(&v).cloned().unwrap_or(SilType::Entero64);
                        let es_param = matches!(ins.op, Operacion::Param { .. });
                        if !es_param {
                            orden_vals.push((
                                v,
                                valtype(&t)
                                    .map_err(|_| ErrorWasm::TipoNoSoportado(format!("{t:?}")))?,
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut locales: HashMap<ValueId, u32> = HashMap::new();
    {
        let mut idx: u32 = 0;
        let mut bids: Vec<_> = tarea.bloques.keys().collect();
        bids.sort_by_key(|k| format!("{k:?}"));
        for bid in bids {
            let b = &tarea.bloques[bid];
            for ins in &b.instrs {
                if let Operacion::Param { .. } = &ins.op {
                    if let Some(v) = ins.resultado {
                        locales.insert(v, idx);
                        idx += 1;
                    }
                }
            }
        }
        for (v, _) in &orden_vals {
            locales.insert(*v, idx);
            idx += 1;
        }
    }
    Ok((orden_vals, locales))
}

/// Cuerpo de una función: declara sus locales, emite instrucciones en orden
/// y deja el retorno en la pila (si `con_retorno`). `llamables` mapea
/// nombre de tarea → índice de función para `generar` (vacío = error limpio).
#[allow(clippy::too_many_arguments)]
fn cuerpo_funcion(
    tarea: &TareaIR,
    orden_vals: &[(ValueId, ValType)],
    locales: &HashMap<ValueId, u32>,
    off_texto: &HashMap<String, u32>,
    idx_host: &HashMap<&str, u32>,
    llamables: &HashMap<String, (u32, bool)>,
    con_retorno: bool,
) -> Result<wasm_encoder::Function, ErrorWasm> {
    // Declarar locals para intermedios: agrupar consecutivos del mismo tipo.
    let mut locals_decl: Vec<(u32, ValType)> = Vec::new();
    for (_, t) in orden_vals {
        if let Some((n, last)) = locals_decl.last_mut() {
            if *last == *t {
                *n += 1;
                continue;
            }
        }
        locals_decl.push((1, *t));
    }
    let mut f = wasm_encoder::Function::new(locals_decl);

    // Emitir instrucciones en orden.
    let mut bids: Vec<_> = tarea.bloques.keys().collect();
    bids.sort_by_key(|k| format!("{k:?}"));
    // Rastrear el valor de retorno (del terminador).
    let mut valor_retorno: Option<ValueId> = None;
    for bid in bids {
        let b = &tarea.bloques[bid];
        for ins in &b.instrs {
            emitir_instr(&mut f, locales, tarea, ins, off_texto, idx_host, llamables)?;
        }
        if let Terminador::Retorno(v) = &b.terminador {
            valor_retorno = *v;
        }
    }

    // Pushear valor de retorno a la pila (si no es void).
    if con_retorno {
        if let Some(v) = valor_retorno {
            let idx = locales
                .get(&v)
                .ok_or_else(|| ErrorWasm::Wasm("retorno sin local".into()))?;
            f.instruction(&wasm_encoder::Instruction::LocalGet(*idx));
        } else {
            return Err(ErrorWasm::Wasm(
                "función con retorno declarado pero sin valor".into(),
            ));
        }
    }

    f.instruction(&wasm_encoder::Instruction::End);
    Ok(f)
}
/// Invariante: la pila está VACÍA antes y después de cada instrucción.
/// Texto = i64 empaquetado (ptr32 alto, len32 bajo). Capacidad = i64 handle.
/// Llamadas al host "sil" (ver tipos en emitir()): el host provee HMAC,
/// canales, IO y red con la misma semantica que sil-rt.
/// Emite una instrucción: operandos vía local.get → computa → local.set resultado.
/// Invariante: la pila está VACÍA antes y después de cada instrucción.
/// Texto = i64 empaquetado (ptr32 alto, len32 bajo). Capacidad = i64 handle.
/// Llamadas al host "sil" (ver tipos en emitir()): el host provee HMAC,
/// canales, IO y red con la misma semantica que sil-rt.
fn emitir_instr(
    f: &mut wasm_encoder::Function,
    locales: &HashMap<ValueId, u32>,
    tarea: &TareaIR,
    ins: &silc_causal_ir::nodes::InstrCausal,
    off_texto: &HashMap<String, u32>,
    idx_host: &HashMap<&str, u32>,
    llamables: &HashMap<String, (u32, bool)>,
) -> Result<(), ErrorWasm> {
    use wasm_encoder::Instruction as I;
    let dst = |v: Option<ValueId>| -> Result<u32, ErrorWasm> {
        v.and_then(|x| locales.get(&x).copied())
            .ok_or_else(|| ErrorWasm::Wasm("instr sin resultado/local".into()))
    };
    let src = |v: ValueId| -> Result<u32, ErrorWasm> {
        locales
            .get(&v)
            .copied()
            .ok_or_else(|| ErrorWasm::Wasm("operando sin local".into()))
    };
    let host = |nombre: &str| -> Result<u32, ErrorWasm> {
        idx_host
            .get(nombre)
            .copied()
            .ok_or_else(|| ErrorWasm::Wasm(format!("import host {nombre} no declarado")))
    };
    // Deja (ptr i32, len i32) del texto empaquetado en el local dado.
    let push_ptr_len = |f: &mut wasm_encoder::Function, local: u32| {
        f.instruction(&I::LocalGet(local));
        f.instruction(&I::I64Const(32));
        f.instruction(&I::I64ShrU);
        f.instruction(&I::I32WrapI64);
        f.instruction(&I::LocalGet(local));
        f.instruction(&I::I64Const(0xFFFF_FFFF));
        f.instruction(&I::I64And);
        f.instruction(&I::I32WrapI64);
    };
    // Permiso IR → i64 para el host.
    let permiso_host = |p: silc_causal_ir::nodes::Permiso| -> i64 {
        use silc_causal_ir::nodes::Permiso as P;
        match p {
            P::LecturaArchivo => 1,
            P::EscrituraArchivo => 2,
            P::Red => 4,
            P::Ejecucion => 8,
        }
    };
    match &ins.op {
        Operacion::Param { .. } => {
            // Params ya están en locals. Nada que emitir.
        }
        Operacion::Const { valor, .. } => {
            if let Some(v) = ins.resultado {
                let d = dst(Some(v))?;
                match valor {
                    Constante::Int(n) => f.instruction(&wasm_encoder::Instruction::I64Const(*n)),
                    Constante::Float(x) => f.instruction(&wasm_encoder::Instruction::F64Const(*x)),
                    Constante::Bool(b) => {
                        f.instruction(&wasm_encoder::Instruction::I32Const(i32::from(*b)))
                    }
                    Constante::Texto(s) => {
                        let off = off_texto
                            .get(s)
                            .copied()
                            .ok_or_else(|| ErrorWasm::Wasm(format!("texto sin segmento: {s:?}")))?;
                        f.instruction(&wasm_encoder::Instruction::I64Const(empaquetar_texto(
                            off,
                            s.len() as u32,
                        )))
                    }
                };
                f.instruction(&wasm_encoder::Instruction::LocalSet(d));
            }
        }
        Operacion::AsignarArena { valor, .. } => {
            // `let x = src` → local.get src; local.set dst.
            if let Some(v) = ins.resultado {
                let s = src(*valor)?;
                let d = dst(Some(v))?;
                f.instruction(&wasm_encoder::Instruction::LocalGet(s));
                f.instruction(&wasm_encoder::Instruction::LocalSet(d));
            }
        }
        Operacion::BinOpSegura { op, lhs, rhs, .. } => {
            if let Some(v) = ins.resultado {
                let l = src(*lhs)?;
                let r = src(*rhs)?;
                let d = dst(Some(v))?;
                f.instruction(&wasm_encoder::Instruction::LocalGet(l));
                f.instruction(&wasm_encoder::Instruction::LocalGet(r));
                match op {
                    OpArit::Add => f.instruction(&wasm_encoder::Instruction::I64Add),
                    OpArit::Sub => f.instruction(&wasm_encoder::Instruction::I64Sub),
                    OpArit::Mul => f.instruction(&wasm_encoder::Instruction::I64Mul),
                    OpArit::Div => f.instruction(&wasm_encoder::Instruction::I64DivS),
                };
                f.instruction(&wasm_encoder::Instruction::LocalSet(d));
            }
        }
        Operacion::Comparar { op, lhs, rhs } => {
            if let Some(v) = ins.resultado {
                let l = src(*lhs)?;
                let r = src(*rhs)?;
                let d = dst(Some(v))?;
                f.instruction(&wasm_encoder::Instruction::LocalGet(l));
                f.instruction(&wasm_encoder::Instruction::LocalGet(r));
                match op {
                    OpCmp::Gt => f.instruction(&wasm_encoder::Instruction::I64GtS),
                    OpCmp::Lt => f.instruction(&wasm_encoder::Instruction::I64LtS),
                    OpCmp::Eq => f.instruction(&wasm_encoder::Instruction::I64Eq),
                    OpCmp::Ne => f.instruction(&wasm_encoder::Instruction::I64Ne),
                    OpCmp::Ge => f.instruction(&wasm_encoder::Instruction::I64GeS),
                    OpCmp::Le => f.instruction(&wasm_encoder::Instruction::I64LeS),
                };
                f.instruction(&wasm_encoder::Instruction::LocalSet(d));
            } else {
                // Comparación de verificación sin resultado (marcador M3):
                // evaluar y hacer drop para no corromper la pila.
                let l = src(*lhs)?;
                let r = src(*rhs)?;
                f.instruction(&wasm_encoder::Instruction::LocalGet(l));
                f.instruction(&wasm_encoder::Instruction::LocalGet(r));
                match op {
                    OpCmp::Gt => f.instruction(&wasm_encoder::Instruction::I64GtS),
                    OpCmp::Lt => f.instruction(&wasm_encoder::Instruction::I64LtS),
                    OpCmp::Eq => f.instruction(&wasm_encoder::Instruction::I64Eq),
                    OpCmp::Ne => f.instruction(&wasm_encoder::Instruction::I64Ne),
                    OpCmp::Ge => f.instruction(&wasm_encoder::Instruction::I64GeS),
                    OpCmp::Le => f.instruction(&wasm_encoder::Instruction::I64LeS),
                };
                f.instruction(&wasm_encoder::Instruction::Drop);
            }
        }
        Operacion::Retornar { .. } => {
            // El terminador maneja el retorno vía local.get. Nada aquí.
        }
        Operacion::Promover { valor, .. } => {
            // Misma arena en este backend: copiar local.
            if let Some(v) = ins.resultado {
                let s = src(*valor)?;
                let d = dst(Some(v))?;
                f.instruction(&I::LocalGet(s));
                f.instruction(&I::LocalSet(d));
            }
        }
        Operacion::Ramificar { .. } | Operacion::Saltar { .. } => {
            // El lenguaje aun no tiene `si`: inalcanzable desde el lowering.
            return Err(ErrorWasm::Wasm(
                "rama/salto explicito requiere CFG multi-bloque (fuera de alcance)".into(),
            ));
        }
        Operacion::CrearCanal { capacidad } => {
            if let Some(v) = ins.resultado {
                let d = dst(Some(v))?;
                let h = host("sil_canal_crear")?;
                f.instruction(&I::I64Const(*capacidad as i64));
                f.instruction(&I::Call(h));
                f.instruction(&I::LocalSet(d));
            }
        }
        Operacion::SolicitarCapacidad { req } => {
            if let Some(v) = ins.resultado {
                let d = dst(Some(v))?;
                let h = host("sil_cap_solicitar")?;
                // El recurso viaja como literal en el segmento de datos.
                let rec = req.recurso.clone().unwrap_or_default();
                let off = off_texto
                    .get(&rec)
                    .copied()
                    .ok_or_else(|| ErrorWasm::Wasm(format!("recurso sin segmento: {rec:?}")))?;
                f.instruction(&I::I32Const(off as i32));
                f.instruction(&I::I32Const(rec.len() as i32));
                f.instruction(&I::I64Const(permiso_host(req.permiso)));
                f.instruction(&I::I64Const(req.ttl_ns as i64));
                f.instruction(&I::Call(h));
                f.instruction(&I::LocalSet(d));
            }
        }
        Operacion::ValidarCapacidad { token } => {
            if let Some(v) = ins.resultado {
                let t = src(*token)?;
                let d = dst(Some(v))?;
                let h = host("sil_cap_validar")?;
                f.instruction(&I::LocalGet(t));
                // ptr=0,len=0: solo integridad + expiracion (el binding lo
                // verifica la operacion de E/S con la ruta real).
                f.instruction(&I::I32Const(0));
                f.instruction(&I::I32Const(0));
                f.instruction(&I::I64Const(0));
                f.instruction(&I::Call(h));
                f.instruction(&I::LocalSet(d));
            }
        }
        Operacion::InvocarIO {
            operacion,
            args,
            token,
        } => {
            use silc_causal_ir::nodes::OpIO;
            let t = src(*token)?;
            match operacion {
                OpIO::LeerArchivo => {
                    let ruta = args
                        .first()
                        .copied()
                        .ok_or_else(|| ErrorWasm::Wasm("leer sin ruta".into()))?;
                    let r = src(ruta)?;
                    let h = host("sil_io_leer")?;
                    push_ptr_len(f, r);
                    f.instruction(&I::LocalGet(t));
                    f.instruction(&I::Call(h));
                    if let Some(v) = ins.resultado {
                        let d = dst(Some(v))?;
                        f.instruction(&I::LocalSet(d));
                    } else {
                        f.instruction(&I::Drop);
                    }
                }
                OpIO::EscribirArchivo => {
                    let dval = args
                        .first()
                        .copied()
                        .ok_or_else(|| ErrorWasm::Wasm("escribir sin datos".into()))?;
                    let ruta = args
                        .get(1)
                        .copied()
                        .ok_or_else(|| ErrorWasm::Wasm("escribir sin ruta".into()))?;
                    // En WASM los datos viajan como (ptr,len): exige Texto.
                    if tarea.tipos.get(&dval) != Some(&silc_causal_ir::nodes::SilType::Texto) {
                        return Err(ErrorWasm::Wasm("escribir en WASM exige datos Texto".into()));
                    }
                    let dv = src(dval)?;
                    let r = src(ruta)?;
                    let h = host("sil_io_escribir")?;
                    push_ptr_len(f, r);
                    push_ptr_len(f, dv);
                    f.instruction(&I::LocalGet(t));
                    f.instruction(&I::Call(h));
                    // Resultado: bytes o -1; descartar si nadie lo usa.
                    if let Some(vv) = ins.resultado {
                        let d = dst(Some(vv))?;
                        f.instruction(&I::LocalSet(d));
                    } else {
                        f.instruction(&I::Drop);
                    }
                }
                OpIO::EnviarRed => {
                    let dval = args
                        .first()
                        .copied()
                        .ok_or_else(|| ErrorWasm::Wasm("enviar sin datos".into()))?;
                    let dstv = args
                        .get(1)
                        .copied()
                        .ok_or_else(|| ErrorWasm::Wasm("enviar sin destino".into()))?;
                    if tarea.tipos.get(&dval) != Some(&silc_causal_ir::nodes::SilType::Texto) {
                        return Err(ErrorWasm::Wasm(
                            "enviar red en WASM exige datos Texto".into(),
                        ));
                    }
                    let dv = src(dval)?;
                    let s = src(dstv)?;
                    let h = host("sil_red_enviar")?;
                    push_ptr_len(f, s);
                    push_ptr_len(f, dv);
                    f.instruction(&I::LocalGet(t));
                    f.instruction(&I::Call(h));
                    if let Some(vv) = ins.resultado {
                        let d = dst(Some(vv))?;
                        f.instruction(&I::LocalSet(d));
                    } else {
                        f.instruction(&I::Drop);
                    }
                }
                OpIO::RecibirRed => {
                    let dstv = args
                        .first()
                        .copied()
                        .ok_or_else(|| ErrorWasm::Wasm("recibir sin destino".into()))?;
                    let s = src(dstv)?;
                    let h = host("sil_red_recibir")?;
                    push_ptr_len(f, s);
                    f.instruction(&I::LocalGet(t));
                    f.instruction(&I::Call(h));
                    if let Some(v) = ins.resultado {
                        let d = dst(Some(v))?;
                        f.instruction(&I::LocalSet(d));
                    } else {
                        f.instruction(&I::Drop);
                    }
                }
            }
        }
        Operacion::CrearFibra { tarea: nombre, .. } => {
            // En programa multi-tarea: llamada directa (semántica cooperativa
            // idéntica al C99 sin desalojo). Sin programa: error limpio.
            match llamables.get(nombre) {
                Some((idx, con_ret)) => {
                    f.instruction(&I::Call(*idx));
                    if *con_ret {
                        f.instruction(&I::Drop);
                    }
                }
                None => {
                    return Err(ErrorWasm::Wasm(format!(
                        "generar {nombre}: requiere programa multi-tarea"
                    )));
                }
            }
        }
        Operacion::EnviarCanal { canal, valor } => {
            let c = src(*canal)?;
            let v = src(*valor)?;
            let h = host("sil_canal_enviar")?;
            f.instruction(&I::LocalGet(c));
            f.instruction(&I::LocalGet(v));
            f.instruction(&I::Call(h));
            f.instruction(&I::Drop);
        }
        Operacion::RecibirCanal { canal } => {
            if let Some(v) = ins.resultado {
                let c = src(*canal)?;
                let d = dst(Some(v))?;
                let h = host("sil_canal_recibir")?;
                f.instruction(&I::LocalGet(c));
                f.instruction(&I::Call(h));
                f.instruction(&I::LocalSet(d));
            }
        }
    }
    // Invariantes: emitir trap condicional para Demostracion (verificación SMT en runtime).
    if let Some(inv) = &ins.invariante {
        use silc_causal_ir::nodes::ClaseInvariante;
        if inv.clase == ClaseInvariante::Demostracion {
            emitir_trap_si_falso(f, locales, &inv.formula)?;
        }
    }
    Ok(())
}

// =============================================================================
// Tests M6 (disciplina elite: binario validado con wasmparser)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use silc_causal_ir::{lower::bajar_tarea, nodes::TareaIR};
    use silc_frontend::{lexer::lexear, parser::parsear};

    fn ir_de(src: &str) -> TareaIR {
        let prog = parsear(&lexear(src)).expect("parse OK");
        let silc_frontend::ast::Decl::Tarea(t) = &prog.defs[0] else {
            panic!()
        };
        bajar_tarea(t).expect("lower OK")
    }

    fn valida_wasm(bytes: &[u8]) {
        wasmparser::Validator::new()
            .validate_all(bytes)
            .expect("WASM inválido");
    }

    #[test]
    fn emite_identidad_valida() {
        let ir = ir_de("definir tarea f(x: Entero64) -> Entero64:\n    retornar x\n");
        let b = emitir(&ir).unwrap();
        assert!(!b.is_empty());
        valida_wasm(&b);
    }

    #[test]
    fn exporta_nombre_y_memoria() {
        let ir = ir_de("definir tarea mifunc(x: Entero64) -> Entero64:\n    retornar x\n");
        let b = emitir(&ir).unwrap();
        valida_wasm(&b);
        // Verificar exports e imports parseando el módulo.
        let mut exports = Vec::new();
        let mut memorias_importadas = 0;
        for p in wasmparser::Parser::new(0).parse_all(&b) {
            match p {
                Ok(wasmparser::Payload::ExportSection(s)) => {
                    for e in s {
                        let e = e.unwrap();
                        exports.push(e.name.to_string());
                    }
                }
                Ok(wasmparser::Payload::ImportSection(s)) => {
                    for e in s {
                        let e = e.unwrap();
                        if e.module == "sil" && e.name == "memory" {
                            memorias_importadas += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        assert!(
            exports.contains(&"mifunc".to_string()),
            "exports: {exports:?}"
        );
        assert_eq!(memorias_importadas, 1, "falta import sil.memory");
    }

    #[test]
    fn aritmetica_params_valida() {
        // x + y con ambos params → local.get + add. Válido M6.
        let ir =
            ir_de("definir tarea s(x: Entero64, y: Entero64) -> Entero64:\n    retornar x + y\n");
        let b = emitir(&ir).unwrap();
        valida_wasm(&b);
    }

    #[test]
    fn comparacion_valida() {
        // verificar genera Comparar; el retorno usa la comparación.
        // M6: `retornar x > 0` no es sintaxis CNL válida (retornar expr aritmética).
        // Usamos tarea que retorna param y tiene verificación (la verificación
        // no emite código que rompa la pila en M6 por diseño).
        let ir = ir_de(
            "definir tarea f(x: Entero64) -> Entero64:\n    verificar que x > 0\n    retornar x\n",
        );
        let b = emitir(&ir).unwrap();
        valida_wasm(&b);
    }

    #[test]
    fn intermedio_reutilizado_valido() {
        // let y = x * 2; retornar x + y → `y` es intermedio reutilizado.
        // M6 con locals-para-todo: funciona correctamente.
        let ir = ir_de("definir tarea f(x: Entero64) -> Entero64:\n    let y: Entero64 = x * 2\n    retornar x + y\n");
        let b = emitir(&ir).unwrap();
        valida_wasm(&b);
    }

    #[test]
    fn tipos_no_soportados_error_limpio() {
        let ir = ir_de("definir tarea f(xs: Lista de Entero64) -> Entero64:\n    retornar 0\n");
        let r = emitir(&ir);
        assert!(r.is_err());
    }

    #[test]
    fn capacidades_emiten_imports_sil() {
        let ir = ir_de(
            "definir tarea f() -> Booleano:\n    solicitar capacidad lectura_archivo para \"/tmp/x\" como cap\n    validar cap\n    retornar verdadero\n",
        );
        let b = emitir(&ir).unwrap();
        valida_wasm(&b);
        // El módulo importa el host sil con las funciones usadas.
        let mut nombres = Vec::new();
        for p in wasmparser::Parser::new(0).parse_all(&b) {
            if let Ok(wasmparser::Payload::ImportSection(s)) = p {
                for e in s {
                    let e = e.unwrap();
                    nombres.push(format!("{}.{}", e.module, e.name));
                }
            }
        }
        assert!(
            nombres.contains(&"sil.sil_cap_solicitar".to_string()),
            "imports: {nombres:?}"
        );
        assert!(
            nombres.contains(&"sil.sil_cap_validar".to_string()),
            "imports: {nombres:?}"
        );
    }

    #[test]
    fn canal_valido() {
        let ir = ir_de(
            "definir tarea f() -> Entero64:\n    crear canal con capacidad 8 como c\n    enviar 40 en c\n    recibir de c como v\n    retornar v\n",
        );
        let b = emitir(&ir).unwrap();
        valida_wasm(&b);
    }

    #[test]
    fn texto_en_segmento_de_datos() {
        let ir = ir_de(
            "definir tarea f() -> Booleano:\n    solicitar capacidad lectura_archivo para \"/tmp/datos\" como cap\n    leer archivo \"/tmp/datos\" con cap como t\n    retornar verdadero\n",
        );
        let b = emitir(&ir).unwrap();
        valida_wasm(&b);
        // El literal de ruta vive en el segmento de datos.
        let mut segmentos = 0;
        for p in wasmparser::Parser::new(0).parse_all(&b) {
            if let Ok(wasmparser::Payload::DataSection(s)) = p {
                for e in s {
                    let _ = e.unwrap();
                    segmentos += 1;
                }
            }
        }
        assert!(segmentos > 0, "sin segmento de datos");
    }

    #[test]
    fn export_apunta_a_la_tarea() {
        // El export de la tarea debe apuntar a la función definida,
        // no a un import (bug histórico: índice 0).
        let ir = ir_de(
            "definir tarea mifunc() -> Booleano:\n    solicitar capacidad red para \"h:1\" como cap\n    retornar verdadero\n",
        );
        let b = emitir(&ir).unwrap();
        valida_wasm(&b);
        let mut n_imports = 0;
        let mut idx_export = 0;
        for p in wasmparser::Parser::new(0).parse_all(&b) {
            match p {
                Ok(wasmparser::Payload::ImportSection(s)) => {
                    for e in s {
                        let e = e.unwrap();
                        // Solo cuentan los imports de función (la memoria no).
                        if matches!(e.ty, wasmparser::TypeRef::Func(_)) {
                            n_imports += 1;
                        }
                    }
                }
                Ok(wasmparser::Payload::ExportSection(s)) => {
                    for e in s {
                        let e = e.unwrap();
                        if e.name == "mifunc" {
                            idx_export = e.index;
                        }
                    }
                }
                _ => {}
            }
        }
        assert_eq!(
            idx_export, n_imports,
            "export debe apuntar tras los imports"
        );
    }
}
