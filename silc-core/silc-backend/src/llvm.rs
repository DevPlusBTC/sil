//! Emisión LLVM IR vía inkwell (LLVM 17).
//! Requiere feature `llvm` + LLVM 17 instalado.

#[cfg(feature = "llvm")]
use inkwell::types::BasicType;
use silc_causal_ir::nodes::TareaIR;
#[cfg(feature = "llvm")]
use silc_causal_ir::nodes::{Operacion, SilType};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorLLVM {
    #[error("Error LLVM: {0}")]
    Llvm(String),
    #[error("Backend LLVM no compilado (activa feature `llvm` + instala LLVM 17)")]
    NoDisponible,
    #[error("Tipo no soportado: {0}")]
    TipoNoSoportado(String),
    #[error("Variable no definida: {0}")]
    VariableNoDefinida(String),
}

#[cfg(feature = "llvm")]
impl From<inkwell::builder::BuilderError> for ErrorLLVM {
    fn from(e: inkwell::builder::BuilderError) -> Self {
        ErrorLLVM::Llvm(e.to_string())
    }
}

/// Emite módulo LLVM IR desde TareaIR.
pub fn emitir(tarea: &TareaIR) -> Result<String, ErrorLLVM> {
    #[cfg(feature = "llvm")]
    {
        use self::llvm_impl::*;
        let context = inkwell::context::Context::create();
        let module = context.create_module(&tarea.nombre);
        let builder = context.create_builder();

        declarar_funciones_runtime(&module, &context)?;

        let param_types = tarea
            .params
            .iter()
            .map(|(_, tipo)| tipo_llvm(&context, tipo).map(Into::into))
            .collect::<Result<Vec<_>, _>>()?;
        let fn_type = match &tarea.retorno {
            SilType::Void => context.void_type().fn_type(&param_types, false),
            tipo => tipo_llvm(&context, tipo)?.fn_type(&param_types, false),
        };
        let function = module.add_function(&tarea.nombre, fn_type, None);

        let mut vars_llvm = std::collections::HashMap::new();
        let mut indice_param = 0u32;
        for bloque in tarea.bloques.values() {
            for instr in &bloque.instrs {
                if matches!(instr.op, Operacion::Param { .. }) {
                    if let (Some(destino), Some(arg)) =
                        (instr.resultado, function.get_nth_param(indice_param))
                    {
                        vars_llvm.insert(destino, arg);
                    }
                    indice_param += 1;
                }
            }
        }

        for bloque in tarea.bloques.values() {
            generar_bloque(
                &context,
                &module,
                &builder,
                bloque,
                &mut vars_llvm,
                function,
            )?;
        }

        if let Err(e) = module.verify() {
            return Err(ErrorLLVM::Llvm(format!("Verificación LLVM falló: {}", e)));
        }
        Ok(module.to_string())
    }
    #[cfg(not(feature = "llvm"))]
    {
        let _ = tarea;
        Err(ErrorLLVM::NoDisponible)
    }
}

#[cfg(feature = "llvm")]
mod llvm_impl {
    use super::*;
    use inkwell::builder::Builder;
    use inkwell::context::Context;
    use inkwell::module::Module;
    use inkwell::types::BasicTypeEnum;
    use inkwell::values::{BasicValueEnum, FunctionValue};
    use inkwell::AddressSpace;
    use silc_causal_ir::nodes::{
        Bloque, Constante, InstrCausal, OpArit, OpCmp, OpIO, Terminador, ValueId,
    };

    pub(super) fn tipo_llvm<'ctx>(
        context: &'ctx Context,
        tipo: &SilType,
    ) -> Result<BasicTypeEnum<'ctx>, ErrorLLVM> {
        Ok(match tipo {
            SilType::Entero64 | SilType::USD | SilType::EUR | SilType::CapacidadHardware => {
                context.i64_type().into()
            }
            SilType::Flotante64 => context.f64_type().into(),
            SilType::Booleano => context.bool_type().into(),
            SilType::Texto => context.i8_type().ptr_type(AddressSpace::default()).into(),
            SilType::Void => {
                return Err(ErrorLLVM::TipoNoSoportado(
                    "Void no es un tipo de valor".into(),
                ))
            }
            otro => return Err(ErrorLLVM::TipoNoSoportado(format!("{otro:?}"))),
        })
    }

    // Lifetime explícito para Module/Context (Module<'ctx> es invariante)
    pub(super) fn declarar_funciones_runtime<'ctx>(
        module: &Module<'ctx>,
        context: &'ctx Context,
    ) -> Result<(), ErrorLLVM> {
        let ptr_type = context.i8_type().ptr_type(AddressSpace::default());
        let size_type = context.i64_type();

        module.add_function(
            "sil_arena_asignar",
            ptr_type.fn_type(
                &[ptr_type.into(), size_type.into(), size_type.into()],
                false,
            ),
            None,
        );
        module.add_function(
            "sil_arena_reset",
            context.void_type().fn_type(&[ptr_type.into()], false),
            None,
        );
        module.add_function(
            "sil_canal_crear",
            ptr_type.fn_type(&[ptr_type.into(), size_type.into()], false),
            None,
        );

        let bool_type = context.bool_type();
        module.add_function(
            "sil_canal_enviar",
            bool_type.fn_type(&[ptr_type.into(), ptr_type.into()], false),
            None,
        );

        let ptr_ptr_type = context
            .i8_type()
            .ptr_type(AddressSpace::default())
            .ptr_type(AddressSpace::default());
        module.add_function(
            "sil_canal_recibir",
            bool_type.fn_type(&[ptr_type.into(), ptr_ptr_type.into()], false),
            None,
        );
        Ok(())
    }

    pub(super) fn generar_bloque<'ctx>(
        context: &'ctx Context,
        module: &Module<'ctx>,
        builder: &Builder<'ctx>,
        bloque: &Bloque,
        vars_llvm: &mut std::collections::HashMap<ValueId, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
    ) -> Result<(), ErrorLLVM> {
        // BlockId no implementa Display → usar {:?}
        let block = context.append_basic_block(function, &format!("bloque_{:?}", bloque.id));
        builder.position_at_end(block);

        for instr in &bloque.instrs {
            generar_instruccion(context, module, builder, instr, vars_llvm)?;
        }
        generar_terminador(context, builder, &bloque.terminador, vars_llvm)?;
        Ok(())
    }

    pub(super) fn generar_instruccion<'ctx>(
        context: &'ctx Context,
        module: &Module<'ctx>,
        builder: &Builder<'ctx>,
        instr: &InstrCausal,
        vars_llvm: &mut std::collections::HashMap<ValueId, BasicValueEnum<'ctx>>,
    ) -> Result<(), ErrorLLVM> {
        use silc_causal_ir::nodes::Operacion::*;
        match &instr.op {
            Const { valor, .. } => {
                if let Some(dest) = instr.resultado {
                    vars_llvm.insert(dest, generar_constante(context, builder, valor)?);
                }
            }
            BinOpSegura { op, lhs, rhs } => {
                if let Some(dest) = instr.resultado {
                    let l = vars_llvm
                        .get(lhs)
                        .copied()
                        .ok_or_else(|| ErrorLLVM::VariableNoDefinida(format!("{:?}", lhs)))?;
                    let r = vars_llvm
                        .get(rhs)
                        .copied()
                        .ok_or_else(|| ErrorLLVM::VariableNoDefinida(format!("{:?}", rhs)))?;
                    vars_llvm.insert(dest, generar_binop(context, builder, *op, l, r)?);
                    // deref *
                }
            }
            Comparar { op, lhs, rhs } => {
                if let Some(dest) = instr.resultado {
                    let l = vars_llvm
                        .get(lhs)
                        .copied()
                        .ok_or_else(|| ErrorLLVM::VariableNoDefinida(format!("{:?}", lhs)))?;
                    let r = vars_llvm
                        .get(rhs)
                        .copied()
                        .ok_or_else(|| ErrorLLVM::VariableNoDefinida(format!("{:?}", rhs)))?;
                    vars_llvm.insert(dest, generar_cmp(context, builder, *op, l, r)?);
                    // deref *
                }
            }
            Retornar { .. } => {
                // El terminador del bloque emite el return; aquí no se duplica.
            }
            InvocarIO {
                operacion, args, ..
            } => {
                let fn_name = match operacion {
                    OpIO::LeerArchivo => "sil_io_leer_archivo",
                    OpIO::EscribirArchivo => "sil_io_escribir_archivo",
                    OpIO::EnviarRed => "sil_io_enviar_red",
                    OpIO::RecibirRed => "sil_io_recibir_red",
                };
                let fn_val = module.get_function(fn_name).ok_or_else(|| {
                    ErrorLLVM::Llvm(format!("Función runtime no encontrada: {}", fn_name))
                })?;
                let args: Vec<_> = args
                    .iter()
                    .map(|a| {
                        vars_llvm
                            .get(a)
                            .copied()
                            .ok_or_else(|| ErrorLLVM::VariableNoDefinida(format!("{:?}", a)))
                            .unwrap()
                            .into()
                    })
                    .collect();
                if let Some(dest) = instr.resultado {
                    if let Some(val) = builder
                        .build_call(fn_val, &args, "call_io")?
                        .try_as_basic_value()
                        .left()
                    {
                        vars_llvm.insert(dest, val);
                    }
                }
            }
            EnviarCanal { canal, valor } => {
                let fn_val = module
                    .get_function("sil_canal_enviar")
                    .ok_or_else(|| ErrorLLVM::Llvm("sil_canal_enviar no encontrado".into()))?;
                let call = builder.build_call(
                    fn_val,
                    &[vars_llvm[canal].into(), vars_llvm[valor].into()],
                    "send",
                )?;
                if let Some(dest) = instr.resultado {
                    if let Some(v) = call.try_as_basic_value().left() {
                        vars_llvm.insert(dest, v);
                    }
                }
            }
            RecibirCanal { canal } => {
                let fn_val = module
                    .get_function("sil_canal_recibir")
                    .ok_or_else(|| ErrorLLVM::Llvm("sil_canal_recibir no encontrado".into()))?;
                let ptr_type = context.i8_type().ptr_type(AddressSpace::default());
                let msg_ptr = builder.build_alloca(ptr_type, "msg_ptr")?;
                builder.build_call(fn_val, &[vars_llvm[canal].into(), msg_ptr.into()], "recv")?;
                if let Some(dest) = instr.resultado {
                    vars_llvm.insert(dest, builder.build_load(ptr_type, msg_ptr, "msg")?);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn generar_terminador<'ctx>(
        _context: &'ctx Context,
        builder: &Builder<'ctx>,
        terminador: &Terminador,
        vars_llvm: &std::collections::HashMap<ValueId, BasicValueEnum<'ctx>>,
    ) -> Result<(), ErrorLLVM> {
        use silc_causal_ir::nodes::Terminador::*;
        match terminador {
            Retorno(v) => {
                if let Some(v) = v {
                    builder.build_return(Some(&vars_llvm[v]))?;
                } else {
                    builder.build_return(None)?;
                }
            }
            Salto(_) => {}
            Rama {
                cond,
                entonces: _,
                sino: _,
            } => {
                let _ = vars_llvm[cond].into_int_value();
            }
            Inalcanzable => {
                builder.build_unreachable()?;
            }
        }
        Ok(())
    }

    pub(super) fn generar_constante<'ctx>(
        context: &'ctx Context,
        _builder: &Builder<'ctx>,
        c: &Constante,
    ) -> Result<BasicValueEnum<'ctx>, ErrorLLVM> {
        Ok(match c {
            Constante::Int(n) => context.i64_type().const_int(*n as u64, false).into(),
            Constante::Float(f) => context.f64_type().const_float(*f).into(),
            Constante::Bool(b) => context.bool_type().const_int(*b as u64, false).into(),
            Constante::Texto(_) => context.i8_type().const_int(0, false).into(),
        })
    }

    pub(super) fn generar_binop<'ctx>(
        _context: &'ctx Context,
        builder: &Builder<'ctx>,
        op: OpArit,
        l: BasicValueEnum<'ctx>,
        r: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, ErrorLLVM> {
        let (l, r) = (l.into_int_value(), r.into_int_value());
        Ok(match op {
            OpArit::Add => builder.build_int_add(l, r, "add")?.into(),
            OpArit::Sub => builder.build_int_sub(l, r, "sub")?.into(),
            OpArit::Mul => builder.build_int_mul(l, r, "mul")?.into(),
            OpArit::Div => builder.build_int_signed_div(l, r, "div")?.into(),
        })
    }

    pub(super) fn generar_cmp<'ctx>(
        _context: &'ctx Context,
        builder: &Builder<'ctx>,
        op: OpCmp,
        l: BasicValueEnum<'ctx>,
        r: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, ErrorLLVM> {
        let (l, r) = (l.into_int_value(), r.into_int_value());
        let pred = match op {
            OpCmp::Eq => inkwell::IntPredicate::EQ,
            OpCmp::Ne => inkwell::IntPredicate::NE,
            OpCmp::Lt => inkwell::IntPredicate::SLT,
            OpCmp::Le => inkwell::IntPredicate::SLE,
            OpCmp::Gt => inkwell::IntPredicate::SGT,
            OpCmp::Ge => inkwell::IntPredicate::SGE,
        };
        Ok(builder.build_int_compare(pred, l, r, "cmp")?.into())
    }
}
