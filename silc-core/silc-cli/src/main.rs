//! silc CLI: compilador SIL Fase 0.
//!
//! M8: pipeline end-to-end real — lex → parse → lower → SMT → codegen.
//! Comandos: build, check, run, emit-smt, lsp, --version.

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

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

    /// Timeout SMT por query (ms)
    #[arg(long, default_value = "5000", global = true)]
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
    /// Compila y ejecuta vía C (requiere gcc/clang). Solo tareas sin IO.
    Run {
        #[arg(required = true)]
        archivos: Vec<PathBuf>,
    },
    /// Inicia servidor LSP (JSON-RPC por stdio).
    Lsp,
}

/// Programa compilado en memoria (todas las fases).
struct Compilado {
    nombre_archivo: String,
    tareas_ir: Vec<silc_causal_ir::nodes::TareaIR>,
}

fn leer_fuente(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("No se pudo leer {}", path.display()))
}

/// Fase 1-4: lex → parse → lower → SMT verify. Retorna IR verificado.
fn compilar_archivo(path: &Path, smt_timeout: u64) -> Result<Compilado> {
    let fuente = leer_fuente(path)?;
    let nombre = path.display().to_string();

    // Fase 1: Lex
    let toks = silc_frontend::lexer::lexear(&fuente);
    debug!(archivo = %nombre, tokens = toks.len(), "lex OK");

    // Fase 2: Parse (errores miette con spans via ErrorFrontend)
    let prog = silc_frontend::parser::parsear(&toks).map_err(|e| {
        // ErrorFrontend ya es Diagnostic; lo envolvemos con contexto de archivo.
        miette::miette!("{}: {e:?}", path.display())
    })?;
    debug!(archivo = %nombre, defs = prog.defs.len(), "parse OK");

    // Fase 3: Lower cada tarea
    let mut tareas_ir = Vec::new();
    for decl in &prog.defs {
        if let silc_frontend::ast::Decl::Tarea(t) = decl {
            let ir = silc_causal_ir::lower::bajar_tarea(t).map_err(|e| {
                miette::miette!(
                    "{}: error de lowering en tarea '{}': {e}",
                    path.display(),
                    t.nombre.nombre
                )
            })?;
            tareas_ir.push(ir);
        }
        // M8: estructuras/variantes se registran (sin codegen aún, pero no error).
    }
    if tareas_ir.is_empty() {
        return Err(miette::miette!(
            "{}: sin tareas para compilar",
            path.display()
        ));
    }

    // Fase 4: SMT verify cada tarea (intervalos - Z3 feature needs fixes)
    for ir in &tareas_ir {
        silc_causal_ir::smt::verificar_tarea(ir, smt_timeout).map_err(|e| {
            miette::miette!(
                "{}: verificación SMT falló en '{}': {e}",
                path.display(),
                ir.nombre
            )
        })?;
    }
    info!(archivo = %nombre, tareas = tareas_ir.len(), "SMT OK");

    Ok(Compilado {
        nombre_archivo: nombre,
        tareas_ir,
    })
}

fn cmd_check(archivos: &[PathBuf], smt_timeout: u64) -> Result<()> {
    let mut ok = true;
    for arch in archivos {
        match compilar_archivo(arch, smt_timeout) {
            Ok(c) => println!(
                "{}: OK ({} tareas verificadas)",
                c.nombre_archivo,
                c.tareas_ir.len()
            ),
            Err(e) => {
                eprintln!("{e:?}");
                ok = false;
            }
        }
    }
    if ok {
        Ok(())
    } else {
        Err(miette::miette!("verificación falló"))
    }
}

fn cmd_emit_smt(archivos: &[PathBuf], smt_timeout: u64) -> Result<()> {
    for arch in archivos {
        // Compilar sin verificar (para inspeccionar incluso metas falsas).
        let fuente = leer_fuente(arch)?;
        let toks = silc_frontend::lexer::lexear(&fuente);
        let prog = silc_frontend::parser::parsear(&toks)
            .map_err(|e| miette::miette!("{}: {e:?}", arch.display()))?;
        for decl in &prog.defs {
            if let silc_frontend::ast::Decl::Tarea(t) = decl {
                let ir = silc_causal_ir::lower::bajar_tarea(t)
                    .map_err(|e| miette::miette!("lower: {e}"))?;
                println!(";; === {} :: {} ===", arch.display(), ir.nombre);
                print!("{}", silc_causal_ir::smt::script_completo(&ir));
            }
        }
    }
    let _ = smt_timeout;
    Ok(())
}

fn cmd_build(
    archivos: &[PathBuf],
    out: &Path,
    target: &str,
    smt_timeout: u64,
    cc: bool,
) -> Result<()> {
    if archivos.len() != 1 {
        return Err(miette::miette!(
            "M8: un solo archivo por build (multi-archivo en Fase 1)"
        ));
    }
    let c = compilar_archivo(&archivos[0], smt_timeout)?;
    if c.tareas_ir.len() != 1 {
        return Err(miette::miette!(
            "M8: una sola tarea por archivo (multi-tarea en Fase 1)"
        ));
    }
    let ir = &c.tareas_ir[0];

    match target {
        "c99" => {
            let codigo =
                silc_backend::c99::emitir(ir).map_err(|e| miette::miette!("backend C99: {e}"))?;
            std::fs::write(out, &codigo)
                .into_diagnostic()
                .wrap_err_with(|| format!("No se pudo escribir {}", out.display()))?;
            println!(
                "{} → {} ({} bytes C99)",
                c.nombre_archivo,
                out.display(),
                codigo.len()
            );
            if cc {
                compilar_c_con_cc(out)?;
            }
        }
        "wasm" => {
            let bytes =
                silc_backend::wasm::emitir(ir).map_err(|e| miette::miette!("backend WASM: {e}"))?;
            std::fs::write(out, &bytes)
                .into_diagnostic()
                .wrap_err_with(|| format!("No se pudo escribir {}", out.display()))?;
            println!(
                "{} → {} ({} bytes WASM)",
                c.nombre_archivo,
                out.display(),
                bytes.len()
            );
        }
        otro => {
            return Err(miette::miette!(
                "target desconocido: {otro} (válidos: c99, wasm)"
            ))
        }
    }
    Ok(())
}

/// Compila .c → binario con cc/gcc/clang disponible.
fn compilar_c_con_cc(c_path: &Path) -> Result<()> {
    let bin = c_path.with_extension(if cfg!(windows) { "exe" } else { "" });
    // Intentar cc, gcc, clang en orden.
    let comps = ["cc", "gcc", "clang"];
    let mut ultimo_err = String::new();
    for cc in comps {
        let r = std::process::Command::new(cc)
            .args(["-std=c99", "-O2", "-Wall"])
            .arg(c_path)
            .arg("-o")
            .arg(&bin)
            .output();
        match r {
            Ok(o) if o.status.success() => {
                println!("binario: {}", bin.display());
                return Ok(());
            }
            Ok(o) => {
                ultimo_err = String::from_utf8_lossy(&o.stderr).into_owned();
            }
            Err(e) => {
                ultimo_err = e.to_string();
            }
        }
    }
    Err(miette::miette!(
        "ningún compilador C disponible (cc/gcc/clang): {ultimo_err}"
    ))
}

fn cmd_run(archivos: &[PathBuf], smt_timeout: u64) -> Result<()> {
    // Build a .c temporal + compilar + ejecutar + mostrar salida.
    let tmp_c = std::env::temp_dir().join(format!("sil_run_{}.c", std::process::id()));
    cmd_build(archivos, &tmp_c, "c99", smt_timeout, false)?;
    // Compilar a binario temporal.
    let tmp_bin = tmp_c.with_extension(if cfg!(windows) { "exe" } else { "out" });
    let comps = ["cc", "gcc", "clang"];
    let mut compilado = false;
    for cc in comps {
        let r = std::process::Command::new(cc)
            .args(["-std=c99", "-O2"])
            .arg(&tmp_c)
            .arg("-o")
            .arg(&tmp_bin)
            .output();
        if matches!(r, Ok(o) if o.status.success()) {
            compilado = true;
            break;
        }
    }
    if !compilado {
        return Err(miette::miette!("sin compilador C para `silc run`"));
    }
    let out = std::process::Command::new(&tmp_bin)
        .output()
        .into_diagnostic()
        .wrap_err("falló la ejecución")?;
    print!("{}", String::from_utf8_lossy(&out.stdout));
    eprint!("{}", String::from_utf8_lossy(&out.stderr));
    let _ = std::fs::remove_file(&tmp_c);
    let _ = std::fs::remove_file(&tmp_bin);
    if out.status.success() {
        Ok(())
    } else {
        Err(miette::miette!("el programa retornó {}", out.status))
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let nivel = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    tracing_subscriber::fmt()
        .with_env_filter(format!("silc={nivel}"))
        .init();

    match cli.cmd {
        Comando::Build {
            archivos,
            out,
            target,
            ..
        } => {
            // M8: flag --cc eliminado del struct (simplificado); usamos env SIL_CC=1.
            let cc = std::env::var("SIL_CC").map(|v| v == "1").unwrap_or(false);
            cmd_build(&archivos, &out, &target, cli.smt_timeout, cc)
        }
        Comando::Check { archivos } => cmd_check(&archivos, cli.smt_timeout),
        Comando::EmitSmt { archivos } => cmd_emit_smt(&archivos, cli.smt_timeout),
        Comando::Run { archivos } => cmd_run(&archivos, cli.smt_timeout),
        Comando::Lsp => {
            eprintln!(
                "silc lsp: servidor LSP completo en Fase 1 (protocolo en silc_lsp.py prototipo)."
            );
            eprintln!("M8: use `silc check --verbose` para diagnósticos incrementales.");
            Ok(())
        }
    }
}
