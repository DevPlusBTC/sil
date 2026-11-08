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

        let mut codegen = CodegenLLVM::new(&context, &module, &builder, function, tarea)?;
        codegen.run()?;

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
    use inkwell::values::{BasicValueEnum, FunctionValue, PhiValue};
    use inkwell::AddressSpace;
    use inkwell::BasicBlock;
    use silc_causal_ir::nodes::{
        BlockId, Bloque, Constante, InstrCausal, OpArit, OpCmp, OpIO, Terminador, ValueId,
    };
    use std::collections::{HashMap, HashSet, VecDeque};

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
            SilType::Nominal(_) => context.i8_type().ptr_type(AddressSpace::default()).into(),
            SilType::Void => {
                return Err(ErrorLLVM::TipoNoSoportado(
                    "Void no es un tipo de valor".into(),
                ))
            }
            otro => return Err(ErrorLLVM::TipoNoSoportado(format!("{otro:?}"))),
        })
    }

    pub(super) fn declarar_funciones_runtime<'ctx>(
        module: &Module<'ctx>,
        context: &'ctx Context,
    ) -> Result<(), ErrorLLVM> {
        let ptr_type = context.i8_type().ptr_type(AddressSpace::default());
        let size_type = context.i64_type();
        let bool_type = context.bool_type();
        let void_type = context.void_type();

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
            void_type.fn_type(&[ptr_type.into()], false),
            None,
        );
        module.add_function(
            "sil_canal_crear",
            ptr_type.fn_type(&[ptr_type.into(), size_type.into()], false),
            None,
        );
        module.add_function(
            "sil_canal_enviar",
            bool_type.fn_type(&[ptr_type.into(), ptr_type.into()], false),
            None,
        );
        let ptr_ptr_type = ptr_type.ptr_type(AddressSpace::default());
        module.add_function(
            "sil_canal_recibir",
            bool_type.fn_type(&[ptr_type.into(), ptr_ptr_type.into()], false),
            None,
        );
        module.add_function(
            "sil_io_leer_archivo",
            ptr_type.fn_type(&[ptr_type.into(), ptr_type.into(), ptr_type.into()], false),
            None,
        );
        module.add_function(
            "sil_io_escribir_archivo",
            size_type.fn_type(
                &[
                    ptr_type.into(),
                    ptr_type.into(),
                    ptr_type.into(),
                    ptr_type.into(),
                    size_type.into(),
                ],
                false,
            ),
            None,
        );
        module.add_function(
            "sil_io_enviar_red",
            size_type.fn_type(
                &[
                    ptr_type.into(),
                    ptr_type.into(),
                    ptr_type.into(),
                    ptr_type.into(),
                    size_type.into(),
                ],
                false,
            ),
            None,
        );
        module.add_function(
            "sil_io_recibir_red",
            ptr_type.fn_type(&[ptr_type.into(), ptr_type.into(), ptr_type.into()], false),
            None,
        );
        module.add_function(
            "sil_cap_solicitar",
            ptr_type.fn_type(
                &[ptr_type.into(), size_type.into(), size_type.into()],
                false,
            ),
            None,
        );
        module.add_function(
            "sil_cap_validar",
            bool_type.fn_type(&[ptr_type.into(), ptr_type.into(), size_type.into()], false),
            None,
        );
        Ok(())
    }

    struct CodegenLLVM<'ctx> {
        context: &'ctx Context,
        module: &'ctx Module<'ctx>,
        builder: &'ctx Builder<'ctx>,
        function: FunctionValue<'ctx>,
        tarea: &'ctx TareaIR,
        vars_llvm: HashMap<ValueId, BasicValueEnum<'ctx>>,
        blocks_llvm: HashMap<BlockId, BasicBlock<'ctx>>,
        phi_vars: HashMap<ValueId, PhiValue<'ctx>>,
        predecessors: HashMap<BlockId, Vec<BlockId>>,
        process_order: Vec<BlockId>,
    }

    impl<'ctx> CodegenLLVM<'ctx> {
        fn new(
            context: &'ctx Context,
            module: &'ctx Module<'ctx>,
            builder: &'ctx Builder<'ctx>,
            function: FunctionValue<'ctx>,
            tarea: &'ctx TareaIR,
        ) -> Result<Self, ErrorLLVM> {
            let mut vars_llvm = HashMap::new();
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

            let mut predecessors: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
            for (_, bloque) in &tarea.bloques {
                predecessors.entry(bloque.id).or_default();
                match &bloque.terminador {
                    Terminador::Salto(target) => {
                        predecessors.entry(*target).or_default().push(bloque.id);
                    }
                    Terminador::Rama { entonces, sino, .. } => {
                        predecessors.entry(*entonces).or_default().push(bloque.id);
                        predecessors.entry(*sino).or_default().push(bloque.id);
                    }
                    _ => {}
                }
            }

            let mut phi_candidates: HashSet<ValueId> = HashSet::new();
            for (bid, preds) in &predecessors {
                if preds.len() > 1 {
                    let mut assigned_in_pred: HashMap<ValueId, usize> = HashMap::new();
                    for pred in preds {
                        if let Some(b) = tarea.bloques.get(pred) {
                            for instr in &b.instrs {
                                if let Some(dest) = instr.resultado {
                                    *assigned_in_pred.entry(dest).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                    for (var, count) in assigned_in_pred {
                        if count > 1 || preds.len() > 1 {
                            phi_candidates.insert(var);
                        }
                    }
                }
            }

            let process_order = Self::compute_process_order(tarea, &predecessors);

            Ok(Self {
                context,
                module,
                builder,
                function,
                tarea,
                vars_llvm,
                blocks_llvm: HashMap::new(),
                phi_vars: HashMap::new(),
                predecessors,
                process_order,
            })
        }

        fn compute_process_order(
            tarea: &TareaIR,
            predecessors: &HashMap<BlockId, Vec<BlockId>>,
        ) -> Vec<BlockId> {
            let mut visited = HashSet::new();
            let mut order = Vec::new();
            let mut stack = Vec::new();

            if let Some(entry_bid) = tarea.bloques.keys().next() {
                stack.push(*entry_bid);
            }

            while let Some(bid) = stack.pop() {
                if visited.contains(&bid) {
                    continue;
                }
                visited.insert(bid);
                order.push(bid);

                if let Some(b) = tarea.bloques.get(&bid) {
                    match &b.terminador {
                        Terminador::Salto(target) => stack.push(*target),
                        Terminador::Rama { entonces, sino, .. } => {
                            stack.push(*sino);
                            stack.push(*entonces);
                        }
                        _ => {}
                    }
                }
            }

            for bid in tarea.bloques.keys() {
                if !visited.contains(bid) {
                    order.push(*bid);
                }
            }
            order
        }

        fn run(&mut self) -> Result<(), ErrorLLVM> {
            for &bid in &self.process_order {
                let name = if self.blocks_llvm.is_empty() {
                    "entry"
                } else {
                    &format!("block_{}", bid.0)
                };
                let block = self.context.append_basic_block(self.function, name);
                self.blocks_llvm.insert(bid, block);
            }

            self.insert_phi_nodes()?;

            for &bid in &self.process_order {
                if let Some(block) = self.blocks_llvm.get(&bid) {
                    self.builder.position_at_end(*block);
                    if let Some(bloque) = self.tarea.bloques.get(&bid) {
                        self.generar_bloque(bloque)?;
                    }
                }
            }

            self.complete_phi_nodes()?;
            Ok(())
        }

        fn insert_phi_nodes(&mut self) -> Result<(), ErrorLLVM> {
            for (&bid, preds) in &self.predecessors {
                if preds.len() > 1 {
                    if let Some(block) = self.blocks_llvm.get(&bid) {
                        self.builder.position_at_end(*block);
                        let mut vars_needing_phi = HashSet::new();
                        for pred in preds {
                            if let Some(pb) = self.tarea.bloques.get(pred) {
                                for instr in &pb.instrs {
                                    if let Some(dest) = instr.resultado {
                                        if self.variable_used_after(dest, bid) {
                                            vars_needing_phi.insert(dest);
                                        }
                                    }
                                }
                            }
                        }
                        for (&var, &val) in &self.vars_llvm {
                            if self.variable_used_after(var, bid) {
                                let mut distinct = HashSet::new();
                                for pred in preds {
                                    distinct.insert(pred);
                                }
                                if distinct.len() > 1 {
                                    vars_needing_phi.insert(var);
                                }
                            }
                        }

                        for var in vars_needing_phi {
                            if let Some(&val) = self.vars_llvm.get(&var) {
                                let ty = val.get_type();
                                let phi = self.builder.build_phi(ty, &format!("phi_{}", var.0))?;
                                self.phi_vars.insert(var, phi);
                                self.vars_llvm.insert(var, phi.as_basic_value());
                            }
                        }
                    }
                }
            }
            Ok(())
        }

        fn variable_used_after(&self, var: ValueId, merge_bid: BlockId) -> bool {
            let mut visited = HashSet::new();
            let mut stack = vec![merge_bid];
            while let Some(bid) = stack.pop() {
                if visited.contains(&bid) {
                    continue;
                }
                visited.insert(bid);
                if let Some(b) = self.tarea.bloques.get(&bid) {
                    for instr in &b.instrs {
                        match &instr.op {
                            Operacion::BinOpSegura { lhs, rhs, .. }
                            | Operacion::Comparar { lhs, rhs, .. }
                            | Operacion::EnviarCanal {
                                canal: lhs,
                                valor: rhs,
                            } => {
                                if *lhs == var || *rhs == var {
                                    return true;
                                }
                            }
                            Operacion::RecibirCanal { canal: lhs } => {
                                if *lhs == var {
                                    return true;
                                }
                            }
                            Operacion::InvocarIO { args, .. } => {
                                if args.contains(&var) {
                                    return true;
                                }
                            }
                            Operacion::Retornar { valor } => {
                                if *valor == var {
                                    return true;
                                }
                            }
                            Operacion::SolicitarCapacidad { req } => {
                                if req.recurso.as_ref() == Some(&var) {
                                    return true;
                                }
                            }
                            _ => {}
                        }
                    }
                    match &b.terminador {
                        Terminador::Rama { cond, .. } => {
                            if *cond == var {
                                return true;
                            }
                        }
                        Terminador::Retorno(Some(v)) => {
                            if *v == var {
                                return true;
                            }
                        }
                        _ => {}
                    }
                    match &b.terminador {
                        Terminador::Salto(t) => stack.push(*t),
                        Terminador::Rama { entonces, sino, .. } => {
                            stack.push(*entonces);
                            stack.push(*sino);
                        }
                        _ => {}
                    }
                }
            }
            false
        }

        fn complete_phi_nodes(&mut self) -> Result<(), ErrorLLVM> {
            for (&var, phi) in &self.phi_vars {
                let mut incoming = Vec::new();
                for (&bid, block) in &self.blocks_llvm {
                    self.builder.position_at_end(*block);
                    if let Some(bloque) = self.tarea.bloques.get(&bid) {
                        if let Some(preds) = self.predecessors.get(&bid) {
                            if preds.len() > 1 {
                                for pred in preds {
                                    if let Some(val) = self.vars_llvm.get(&var) {
                                        if let Some(pred_block) = self.blocks_llvm.get(pred) {
                                            incoming.push((*val, *pred_block));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if !incoming.is_empty() {
                    phi.add_incoming(&incoming);
                }
            }
            Ok(())
        }

        fn generar_bloque(&mut self, bloque: &Bloque) -> Result<(), ErrorLLVM> {
            for instr in &bloque.instrs {
                self.generar_instruccion(instr)?;
            }
            self.generar_terminador(&bloque.terminador)?;
            Ok(())
        }

        fn generar_instruccion(&mut self, instr: &InstrCausal) -> Result<(), ErrorLLVM> {
            use silc_causal_ir::nodes::Operacion::*;
            match &instr.op {
                Const { valor, .. } => {
                    if let Some(dest) = instr.resultado {
                        let val = self.generar_constante(valor)?;
                        self.vars_llvm.insert(dest, val);
                    }
                }
                Param { .. } => {}
                BinOpSegura { op, lhs, rhs } => {
                    if let Some(dest) = instr.resultado {
                        let l = self.get_var(*lhs)?;
                        let r = self.get_var(*rhs)?;
                        let val = self.generar_binop(*op, l, r)?;
                        self.vars_llvm.insert(dest, val);
                    }
                }
                Comparar { op, lhs, rhs } => {
                    if let Some(dest) = instr.resultado {
                        let l = self.get_var(*lhs)?;
                        let r = self.get_var(*rhs)?;
                        let val = self.generar_cmp(*op, l, r)?;
                        self.vars_llvm.insert(dest, val);
                    }
                }
                Retornar { .. } => {}
                InvocarIO { operacion, args } => {
                    let fn_name = match operacion {
                        OpIO::LeerArchivo => "sil_io_leer_archivo",
                        OpIO::EscribirArchivo => "sil_io_escribir_archivo",
                        OpIO::EnviarRed => "sil_io_enviar_red",
                        OpIO::RecibirRed => "sil_io_recibir_red",
                    };
                    let fn_val = self.module.get_function(fn_name).ok_or_else(|| {
                        ErrorLLVM::Llvm(format!("Función runtime no encontrada: {}", fn_name))
                    })?;
                    let args: Vec<_> = args
                        .iter()
                        .map(|a| self.get_var(*a).map(Into::into))
                        .collect::<Result<Vec<_>, _>>()?;
                    if let Some(dest) = instr.resultado {
                        if let Some(val) = self
                            .builder
                            .build_call(fn_val, &args, "call_io")?
                            .try_as_basic_value()
                            .left()
                        {
                            self.vars_llvm.insert(dest, val);
                        }
                    }
                }
                EnviarCanal { canal, valor } => {
                    let fn_val = self
                        .module
                        .get_function("sil_canal_enviar")
                        .ok_or_else(|| ErrorLLVM::Llvm("sil_canal_enviar no encontrado".into()))?;
                    let call = self.builder.build_call(
                        fn_val,
                        &[self.get_var(*canal)?.into(), self.get_var(*valor)?.into()],
                        "send",
                    )?;
                    if let Some(dest) = instr.resultado {
                        if let Some(v) = call.try_as_basic_value().left() {
                            self.vars_llvm.insert(dest, v);
                        }
                    }
                }
                RecibirCanal { canal } => {
                    let fn_val = self
                        .module
                        .get_function("sil_canal_recibir")
                        .ok_or_else(|| ErrorLLVM::Llvm("sil_canal_recibir no encontrado".into()))?;
                    let ptr_type = self.context.i8_type().ptr_type(AddressSpace::default());
                    let msg_ptr = self.builder.build_alloca(ptr_type, "msg_ptr")?;
                    self.builder.build_call(
                        fn_val,
                        &[self.get_var(*canal)?.into(), msg_ptr.into()],
                        "recv",
                    )?;
                    if let Some(dest) = instr.resultado {
                        let val = self.builder.build_load(ptr_type, msg_ptr, "msg")?;
                        self.vars_llvm.insert(dest, val);
                    }
                }
                SolicitarCapacidad { req } => {
                    let fn_val = self
                        .module
                        .get_function("sil_cap_solicitar")
                        .ok_or_else(|| ErrorLLVM::Llvm("sil_cap_solicitar no encontrado".into()))?;
                    let recurso = req
                        .recurso
                        .as_ref()
                        .map(|r| self.get_var(*r))
                        .transpose()?
                        .unwrap_or_else(|| {
                            self.context
                                .i8_type()
                                .ptr_type(AddressSpace::default())
                                .const_null()
                                .into()
                        });
                    let call = self.builder.build_call(
                        fn_val,
                        &[
                            recurso.into(),
                            self.context
                                .i64_type()
                                .const_int(req.permisos as u64, false)
                                .into(),
                            self.context
                                .i64_type()
                                .const_int(req.ttl_ns.unwrap_or(0) as u64, false)
                                .into(),
                        ],
                        "cap_req",
                    )?;
                    if let Some(dest) = instr.resultado {
                        if let Some(v) = call.try_as_basic_value().left() {
                            self.vars_llvm.insert(dest, v);
                        }
                    }
                }
                ValidarCapacidad { cap, .. } => {
                    let fn_val = self
                        .module
                        .get_function("sil_cap_validar")
                        .ok_or_else(|| ErrorLLVM::Llvm("sil_cap_validar no encontrado".into()))?;
                    let _ = self.builder.build_call(
                        fn_val,
                        &[
                            self.get_var(*cap)?.into(),
                            self.context
                                .i8_type()
                                .ptr_type(AddressSpace::default())
                                .const_null()
                                .into(),
                            self.context.i32_type().const_int(0, false).into(),
                        ],
                        "cap_val",
                    )?;
                }
                _ => {}
            }
            Ok(())
        }

        fn get_var(&self, var: ValueId) -> Result<BasicValueEnum<'ctx>, ErrorLLVM> {
            self.vars_llvm
                .get(&var)
                .copied()
                .ok_or_else(|| ErrorLLVM::VariableNoDefinida(format!("{:?}", var)))
        }

        fn generar_terminador(&mut self, term: &Terminador) -> Result<(), ErrorLLVM> {
            use silc_causal_ir::nodes::Terminador::*;
            match term {
                Retorno(v) => {
                    if let Some(v) = v {
                        self.builder.build_return(Some(&self.get_var(*v)?))?;
                    } else {
                        self.builder.build_return(None)?;
                    }
                }
                Salto(target) => {
                    if let Some(target_block) = self.blocks_llvm.get(target) {
                        self.builder.build_unconditional_branch(*target_block)?;
                    }
                }
                Rama {
                    cond,
                    entonces,
                    sino,
                } => {
                    let cond_val = self.get_var(*cond)?.into_int_value();
                    let then_block = self
                        .blocks_llvm
                        .get(entonces)
                        .ok_or_else(|| ErrorLLVM::Llvm("Bloque entonces no encontrado".into()))?;
                    let else_block = self
                        .blocks_llvm
                        .get(sino)
                        .ok_or_else(|| ErrorLLVM::Llvm("Bloque sino no encontrado".into()))?;
                    self.builder
                        .build_conditional_branch(cond_val, *then_block, *else_block)?;
                }
                Inalcanzable => {
                    self.builder.build_unreachable()?;
                }
            }
            Ok(())
        }

        fn generar_constante(&self, c: &Constante) -> Result<BasicValueEnum<'ctx>, ErrorLLVM> {
            Ok(match c {
                Constante::Int(n) => self.context.i64_type().const_int(*n as u64, false).into(),
                Constante::Float(f) => self.context.f64_type().const_float(*f).into(),
                Constante::Bool(b) => self.context.bool_type().const_int(*b as u64, false).into(),
                Constante::Texto(_) => self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .const_null()
                    .into(),
            })
        }

        fn generar_binop(
            &self,
            op: OpArit,
            l: BasicValueEnum<'ctx>,
            r: BasicValueEnum<'ctx>,
        ) -> Result<BasicValueEnum<'ctx>, ErrorLLVM> {
            let (l, r) = (l.into_int_value(), r.into_int_value());
            Ok(match op {
                OpArit::Add => self.builder.build_int_add(l, r, "add")?.into(),
                OpArit::Sub => self.builder.build_int_sub(l, r, "sub")?.into(),
                OpArit::Mul => self.builder.build_int_mul(l, r, "mul")?.into(),
                OpArit::Div => self.builder.build_int_signed_div(l, r, "div")?.into(),
            })
        }

        fn generar_cmp(
            &self,
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
            Ok(self.builder.build_int_compare(pred, l, r, "cmp")?.into())
        }
    }
}
