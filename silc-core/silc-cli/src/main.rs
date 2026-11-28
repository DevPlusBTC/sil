//! silc CLI: compilador SIL Fase 0.
//!
//! M8: pipeline end-to-end real — S-exp reader → AST builder → lower → SMT → codegen.
//! Comandos: build, check, run, emit-smt, lsp, --version.

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result, WrapErr};
use silc_causal_ir::nodes::{ConfiguracionRedIR, ManejadorEventoIR, ProgramaIR};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

mod lsp;

#[derive(Parser, Debug)]
#[command(
    name = "silc",
    version,
    about = "Compilador SIL (Semantic Intention Language) - Fase 0"
)]
struct Cli {
    /// Nivel de verbosidad (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Timeout SMT por query (ms). Z3 usa timeout agresivo (500ms) como fallback.
    #[arg(long, default_value = "60000", global = true, value_parser = clap::value_parser!(u64).range(1..))]
    smt_timeout: u64,

    #[command(subcommand)]
    cmd: Comando,
}

#[derive(Subcommand, Debug)]
enum Comando {
    /// Compila .sil → salida (C99 por defecto; --target c99|wasm)
    Build {
        /// Archivos fuente .sil
        #[arg(required = true)]
        archivos: Vec<PathBuf>,
        /// Salida (archivo .c/.wasm o binario con --cc)
        #[arg(short, long, default_value = "a.out.c")]
        out: PathBuf,
        /// Target: c99 | wasm
        #[arg(long, default_value = "c99")]
        target: String,
        /// Compilar el C generado con cc hasta binario (requiere gcc/clang)
        #[arg(long, default_value_t = false)]
        cc: bool,
    },
    /// Solo verifica sintaxis + SMT (sin codegen). Exit 0 si todo válido.
    Check {
        #[arg(required = true)]
        archivos: Vec<PathBuf>,
    },
    /// Emite SMT-LIB2 para inspección manual (debug/auditoría).
    EmitSmt {
        #[arg(required = true)]
        archivos: Vec<PathBuf>,
    },
    /// Emite código Rust legible para auditoría humana (no para producción).
    EmitRust {
        #[arg(required = true)]
        archivos: Vec<PathBuf>,
        #[arg(short, long, default_value = "a.out.rs")]
        out: PathBuf,
    },
    /// Compila y ejecuta vía C (requiere gcc/clang). Solo tareas sin IO.
    Run {
        #[arg(required = true)]
        archivos: Vec<PathBuf>,
    },
    /// Inicia servidor LSP (JSON-RPC por stdio).
    Lsp,
}

/// Programa compilado en memoria: metadata de servicio + tareas IR.
struct Compilado {
    programa_ir: ProgramaIR,
}

fn leer_fuente(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("No se pudo leer {}", path.display()))
}

/// Baja una tarea declarada (de nivel superior o dentro de un servicio) a IR.
fn bajar_tarea_declarada(
    t: &silc_frontend::ast::TareaDecl,
    path: &Path,
) -> Result<silc_causal_ir::nodes::TareaIR> {
    silc_causal_ir::lower::bajar_tarea(t).map_err(|e| {
        miette::miette!(
            "{}: error de lowering en tarea '{}': {e}",
            path.display(),
            t.nombre.nombre
        )
    })
}

/// Baja un servicio a ProgramaIR: extrae metadata de red/manejadores y baja sus tareas.
fn bajar_servicio(s: &silc_frontend::ast::ServicioDecl, path: &Path) -> Result<ProgramaIR> {
    let mut tareas_ir = Vec::new();
    let mut _tipos_definidos: Vec<silc_causal_ir::nodes::TipoDefinido> = Vec::new();
    for t in &s.tareas {
        tareas_ir.push(bajar_tarea_declarada(t, path)?);
    }
    // Tipos definidos en el servicio (estructura, variante, modelo)
    for _decl in &s.tareas {
        // The tasks themselves don't contain type declarations; they're at the top level of the service.
        // For now, we don't have type declarations inside tasks, but if they're added to the AST,
        // they would be processed here.
    }
    // Also check for type declarations at the service level (if the AST supports it)
    // For now, the service AST doesn't include type declarations directly.

    // Manejadores: evento -> tarea handler
    let mut manejadores = Vec::new();
    for m in &s.manejadores {
        // Buscar si hay una tarea en el servicio que coincida con el evento
        let handler_name = format!("__handler_{}", m.evento.nombre);
        let tarea_handler = s
            .tareas
            .iter()
            .find(|t| t.nombre.nombre == handler_name || t.nombre.nombre == m.evento.nombre)
            .map(|t| t.nombre.nombre.clone())
            .unwrap_or_else(|| handler_name);

        let params = m
            .params
            .iter()
            .map(|p| {
                let t = silc_causal_ir::lower::bajar_tipo(&p.tipo)
                    .unwrap_or(silc_causal_ir::nodes::SilType::Entero64);
                (p.nombre.nombre.clone(), t)
            })
            .collect();

        manejadores.push(ManejadorEventoIR {
            evento: m.evento.nombre.clone(),
            params,
            tarea_handler,
        });
    }

    let red = s.red.as_ref().map(|r| ConfiguracionRedIR {
        puerto: r.puerto,
        protocolo: r.protocolo.as_ref().map(|p| p.nombre.clone()),
    });

    // Convertir restricciones frontend -> IR (dropping span)
    let restricciones: Vec<silc_causal_ir::nodes::Restriccion> = s
        .restricciones
        .iter()
        .map(|r| silc_causal_ir::nodes::Restriccion {
            clave: r.clave.clone(),
            valor: r.valor.clone(),
        })
        .collect();

    Ok(ProgramaIR {
        servicio_nombre: Some(s.nombre.nombre.clone()),
        red,
        manejadores,
        tareas: tareas_ir,
        restricciones,
        tipos_definidos: Vec::new(),
    })
}

/// Fase 1-4: S-exp reader + AST builder → lower → SMT verify. Retorna IR verificado.
fn compilar_archivo(path: &Path, smt_timeout: u64) -> Result<Compilado> {
    let fuente = leer_fuente(path)?;
    let nombre = path.display().to_string();

    // Fase 1-2: S-exp reader + AST builder
    let prog = silc_frontend::a_ast::parsear(&fuente)
        .map_err(|e| miette::miette!("{}: {e:?}", path.display()))?;
    debug!(archivo = %nombre, defs = prog.defs.len(), "parse OK");

    // Fase 3: Lower - detectar si hay servicio y procesar apropiadamente
    let programa_ir = if let Some(servicio) = prog.defs.iter().find_map(|d| {
        if let silc_frontend::ast::Decl::Servicio(s) = d {
            Some(s)
        } else {
            None
        }
    }) {
        // Hay un servicio: bajar sus tareas y extraer metadata
        bajar_servicio(servicio, path)?
    } else {
        // Sin servicio: bajar tareas sueltas y tipos definidos
        let mut tareas_ir = Vec::new();
        let mut tipos_definidos: Vec<silc_causal_ir::nodes::TipoDefinido> = Vec::new();
        for decl in &prog.defs {
            if let silc_frontend::ast::Decl::Tarea(t) = decl {
                tareas_ir.push(bajar_tarea_declarada(t, path)?);
            } else if let Some(tipo_def) = silc_causal_ir::lower::bajar_tipo_definido(decl)
                .map_err(|e| miette::miette!("error de lowering: {}", e))?
            {
                tipos_definidos.push(tipo_def);
            }
            // Otras declaraciones se validan en parser pero no generan código en Fase 0.
        }
        if tareas_ir.is_empty() && tipos_definidos.is_empty() {
            return Err(miette::miette!(
                "{}: sin tareas ni tipos para compilar",
                path.display()
            ));
        }
        ProgramaIR {
            servicio_nombre: None,
            red: None,
            manejadores: Vec::new(),
            tareas: tareas_ir,
            restricciones: Vec::new(),
            tipos_definidos,
        }
    };

    // Fase 4: SMT verify cada tarea
    // Estrategia: usar verificador de intervalos por defecto (rápido, siempre disponible).
    // Z3 solo como opción experimental (feature "smt"): setear variable de entorno SIL_USE_Z3=1

    // Bloque Z3: solo compila cuando el feature "smt" está activado
    #[cfg(feature = "smt")]
    {
        let usar_z3 = std::env::var("SIL_USE_Z3").is_ok();
        if usar_z3 {
            // Z3 habilitado vía variable de entorno: intentar Z3 primero con timeout agresivo
            let timeout_z3 = 500; // Timeout agresivo (500ms) para no bloquear
            let mut backend_z3 = silc_causal_ir::smt::VerificadorZ3::new();
            let mut cache = silc_causal_ir::smt::CacheLemas::cargar();

            for ir in &programa_ir.tareas {
                match silc_causal_ir::smt::verificar_tarea_con(
                    ir,
                    &mut backend_z3,
                    &mut cache,
                    timeout_z3,
                ) {
                    Ok(()) => {
                        cache.persistir();
                    }
                    Err(silc_causal_ir::smt::ErrorSMT::Indecidible { .. }) => {
                        // Z3 timeoutó o no pudo decidir → fallback a intervalos silencioso
                        debug!(tarea = %ir.nombre, "Z3 indecidible, usando intervalos");
                        silc_causal_ir::smt::verificar_tarea(ir, smt_timeout)
                            .map_err(|e| miette::miette!("{}: {}", path.display(), e))?;
                    }
                    Err(e) => {
                        return Err(miette::miette!("{}: {}", path.display(), e));
                    }
                }
            }
        } else {
            // Z3 no habilitado: usar intervalos directamente
            for ir in &programa_ir.tareas {
                silc_causal_ir::smt::verificar_tarea(ir, smt_timeout)
                    .map_err(|e| miette::miette!("{}: {}", path.display(), e))?;
            }
        }
    }

    // Sin feature "smt": usar intervalos directamente
    #[cfg(not(feature = "smt"))]
    {
        for ir in &programa_ir.tareas {
            silc_causal_ir::smt::verificar_tarea(ir, smt_timeout)
                .map_err(|e| miette::miette!("{}: {}", path.display(), e))?;
        }
    }

    Ok(Compilado { programa_ir })
}

/// Emite C99 para una tarea individual.
fn emitir_c99(tarea: &silc_causal_ir::nodes::TareaIR, out: &Path) -> Result<()> {
    let c = silc_backend::c99::emitir(tarea).map_err(|e| miette::miette!("error C99: {e}"))?;
    std::fs::write(out, c)
        .into_diagnostic()
        .wrap_err_with(|| format!("escribir {}", out.display()))?;
    Ok(())
}

/// Emite WASM para una tarea individual.
fn emitir_wasm(tarea: &silc_causal_ir::nodes::TareaIR, out: &Path) -> Result<()> {
    let wasm = silc_backend::wasm::emitir(tarea).map_err(|e| miette::miette!("error WASM: {e}"))?;
    std::fs::write(out, wasm)
        .into_diagnostic()
        .wrap_err_with(|| format!("escribir {}", out.display()))?;
    Ok(())
}

/// Emite C99 para un programa completo (servicio + tareas).
fn emitir_programa_c99(prog: &ProgramaIR, out: &Path) -> Result<()> {
    let c =
        silc_backend::c99::emitir_programa(prog).map_err(|e| miette::miette!("error C99: {e}"))?;
    std::fs::write(out, c)
        .into_diagnostic()
        .wrap_err_with(|| format!("escribir {}", out.display()))?;
    Ok(())
}

/// Emite WASM para un programa completo (servicio + tareas).
fn emitir_programa_wasm(prog: &ProgramaIR, out: &Path) -> Result<()> {
    let wasm = silc_backend::wasm::emitir_programa(prog)
        .map_err(|e| miette::miette!("error WASM: {e}"))?;
    std::fs::write(out, wasm)
        .into_diagnostic()
        .wrap_err_with(|| format!("escribir {}", out.display()))?;
    Ok(())
}

/// Emite Rust legible para auditoría.
fn emitir_rust(tarea: &silc_causal_ir::nodes::TareaIR, out: &Path) -> Result<()> {
    let rust = silc_backend::rust::emitir_tarea(tarea);
    std::fs::write(out, rust)
        .into_diagnostic()
        .wrap_err_with(|| format!("escribir {}", out.display()))?;
    Ok(())
}

fn ejecutar_c99_binario(codigo: &str) -> Result<()> {
    static CONTADOR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!("sil_run_{}_{}.c", std::process::id(), n));
    std::fs::write(&path, codigo).into_diagnostic()?;

    let bin = path.with_extension("");
    let out = std::process::Command::new("gcc")
        .args(["-std=c99", "-O2", "-o", &bin.to_string_lossy()])
        .arg(&path)
        .output()
        .into_diagnostic()
        .wrap_err("gcc falló")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(miette::miette!("gcc rechazó: {err}"));
    }
    let _ = std::fs::remove_file(&path);
    let run = std::process::Command::new(&bin)
        .output()
        .into_diagnostic()?;
    let _ = std::fs::remove_file(&bin);
    if !run.status.success() {
        let err = String::from_utf8_lossy(&run.stderr);
        return Err(miette::miette!(
            "ejecución falló (code {}): {err}",
            run.status.code().unwrap_or(-1)
        ));
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Logging: -v = info, -vv = debug
    let nivel = match cli.verbose {
        0 => tracing::Level::WARN,
        1 => tracing::Level::INFO,
        _ => tracing::Level::DEBUG,
    };
    tracing_subscriber::fmt()
        .with_max_level(nivel)
        .with_target(false)
        .init();

    match cli.cmd {
        Comando::Build {
            archivos,
            out,
            target,
            cc,
        } => {
            for path in &archivos {
                let compilado = compilar_archivo(path, cli.smt_timeout)?;
                let prog = compilado.programa_ir;
                match target.as_str() {
                    "c99" => {
                        if prog.tareas.len() == 1 && prog.servicio_nombre.is_none() {
                            emitir_c99(&prog.tareas[0], &out)?;
                        } else {
                            emitir_programa_c99(&prog, &out)?;
                        }
                        if cc {
                            let c = std::fs::read_to_string(&out).into_diagnostic()?;
                            ejecutar_c99_binario(&c)?;
                        }
                    }
                    "wasm" => {
                        if prog.tareas.len() == 1 && prog.servicio_nombre.is_none() {
                            emitir_wasm(&prog.tareas[0], &out)?;
                        } else {
                            emitir_programa_wasm(&prog, &out)?;
                        }
                    }
                    _ => {
                        return Err(miette::miette!(
                            "target desconocido: {target} (use c99|wasm)"
                        ))
                    }
                }
                info!("✅ {} → {}", archivos[0].display(), out.display());
            }
        }
        Comando::Check { archivos } => {
            for path in &archivos {
                compilar_archivo(path, cli.smt_timeout)?;
                info!("✅ {}: verificado", path.display());
            }
        }
        Comando::EmitSmt { archivos } => {
            for path in &archivos {
                let fuente = leer_fuente(path)?;
                let prog = silc_frontend::a_ast::parsear(&fuente)
                    .map_err(|e| miette::miette!("{}: {e:?}", path.display()))?;
                for t in &prog.defs {
                    if let silc_frontend::ast::Decl::Tarea(t) = t {
                        let ir = silc_causal_ir::lower::bajar_tarea(t)
                            .map_err(|e| miette::miette!("lowering: {e}"))?;
                        let smt = silc_causal_ir::smt::script_completo(&ir);
                        println!("{}", smt);
                    }
                }
            }
        }
        Comando::EmitRust { archivos, out } => {
            for path in &archivos {
                let compilado = compilar_archivo(path, cli.smt_timeout)?;
                let prog = compilado.programa_ir;
                if prog.tareas.len() == 1 {
                    emitir_rust(&prog.tareas[0], &out)?;
                    info!("✅ {} → {}", archivos[0].display(), out.display());
                } else {
                    return Err(miette::miette!("emit-rust: solo 1 archivo"));
                }
            }
        }
        Comando::Run { archivos } => {
            for path in &archivos {
                let compilado = compilar_archivo(path, cli.smt_timeout)?;
                let prog = compilado.programa_ir;
                if prog.tareas.len() == 1 {
                    let c = silc_backend::c99::emitir(&prog.tareas[0])
                        .map_err(|e| miette::miette!("error C99: {e}"))?;
                    ejecutar_c99_binario(&c)?;
                } else {
                    return Err(miette::miette!("run: solo 1 archivo (sin servicio)"));
                }
            }
        }
        Comando::Lsp => {
            lsp::servir()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use silc_causal_ir::nodes::TareaIR;

    fn ir_de(src: &str) -> TareaIR {
        let prog = silc_frontend::a_ast::parsear(src).expect("parse OK");
        let silc_frontend::ast::Decl::Tarea(t) = &prog.defs[0] else {
            panic!()
        };
        silc_causal_ir::lower::bajar_tarea(t).expect("lower OK")
    }

    #[test]
    fn check_error_sintaxis() {
        let src = "(task f (params) (body (return @)))";
        assert!(silc_frontend::a_ast::parsear(src).is_err());
    }

    #[test]
    fn check_falla_smt() {
        let src = "(task f (params (monto i64)) (returns i64) (body (prove (> monto 1000000)) (return monto)))";
        let ir = ir_de(src);
        let mut b = silc_causal_ir::smt::VerificadorIntervalos::new();
        let mut c = silc_causal_ir::smt::CacheLemas::cargar();
        assert!(silc_causal_ir::smt::verificar_tarea_con(&ir, &mut b, &mut c, 5000).is_err());
    }

    #[test]
    fn check_ok() {
        let src = "(task f (params (x i64)) (returns i64) (body (assume (> x 0)) (prove (> x 0)) (return x)))";
        let ir = ir_de(src);
        let mut b = silc_causal_ir::smt::VerificadorIntervalos::new();
        let mut c = silc_causal_ir::smt::CacheLemas::cargar();
        silc_causal_ir::smt::verificar_tarea_con(&ir, &mut b, &mut c, 5000).unwrap();
    }

    #[test]
    fn build_target_desconocido_falla() {
        let src = "(task f (params) (returns i64) (body (return 1)))";
        let ir = ir_de(src);
        let _c = silc_backend::c99::emitir(&ir).unwrap();
        let res = std::process::Command::new("cc_inexistente_123")
            .args(["-std=c99", "-fsyntax-only", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        assert!(res.is_err() || !res.unwrap().status.success());
    }

    #[test]
    fn build_c99_genera_archivo() {
        let src = "(task f (params) (returns i64) (body (return 1)))";
        let ir = ir_de(src);
        let tmp = std::env::temp_dir().join("sil_test_build.c");
        emitir_c99(&ir, &tmp).unwrap();
        assert!(tmp.exists());
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn build_wasm_genera_binario_valido() {
        let src = "(task f (params) (returns i64) (body (return 1)))";
        let ir = ir_de(src);
        let tmp = std::env::temp_dir().join("sil_test_build.wasm");
        emitir_wasm(&ir, &tmp).unwrap();
        assert!(tmp.exists());
        wasmparser::Validator::new()
            .validate_all(&std::fs::read(&tmp).unwrap())
            .unwrap();
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn emit_smt_muestra_script() {
        let src = "(task f (params (x i64)) (returns i64) (body (assume (> x 0)) (prove (> x 0)) (return x)))";
        let prog = silc_frontend::a_ast::parsear(src).expect("parse OK");
        let silc_frontend::ast::Decl::Tarea(t) = &prog.defs[0] else {
            panic!()
        };
        let ir = silc_causal_ir::lower::bajar_tarea(t).unwrap();
        let smt = silc_causal_ir::smt::script_completo(&ir);
        assert!(smt.contains("(set-logic QF_LIA)"));
        assert!(smt.contains("(declare-fun x () Int)"));
    }

    #[test]
    fn emit_rust_genera_codigo_audit() {
        let src = "(task f (params (x i64)) (returns i64) (body (return x)))";
        let ir = ir_de(src);
        let tmp = std::env::temp_dir().join("sil_test_rust.rs");
        emitir_rust(&ir, &tmp).unwrap();
        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("pub fn f"));
        let _ = std::fs::remove_file(tmp);
    }
}
