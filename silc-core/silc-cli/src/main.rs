//! silc CLI: compilador SIL Fase 0.
//!
//! M8: pipeline end-to-end real — lex → parse → lower → SMT → codegen.
//! Comandos: build, check, run, emit-smt, lsp, --version.

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

mod lsp;

// Backend SMT: Z3 (feature `smt`) o Intervalos (fallback)
#[cfg(feature = "smt")]
use silc_causal_ir::smt::{verificar_tarea_con, CacheLemas, VerificadorZ3};

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
    #[arg(long, default_value = "5000", global = true, value_parser = clap::value_parser!(u64).range(1..))]
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

    // Fase 3: Lower cada tarea (las de servicio también se compilan).
    let mut tareas_ir = Vec::new();
    for decl in &prog.defs {
        match decl {
            silc_frontend::ast::Decl::Tarea(t) => {
                tareas_ir.push(bajar_tarea_declarada(t, path)?);
            }
            silc_frontend::ast::Decl::Servicio(s) => {
                for t in &s.tareas {
                    tareas_ir.push(bajar_tarea_declarada(t, path)?);
                }
            }
            // Estructuras, variantes, modelos, memoria persistente, capacidades,
            // módulos y migraciones son declaraciones de tipos/metadatos: se
            // validan en el parser y no generan código todavía.
            _ => {}
        }
    }
    if tareas_ir.is_empty() {
        return Err(miette::miette!(
            "{}: sin tareas para compilar",
            path.display()
        ));
    }

    // Fase 4: SMT verify cada tarea
    // Usa Z3 (feature `smt`) si está disponible, si no intervalos
    for ir in &tareas_ir {
        #[cfg(feature = "smt")]
        {
            let mut backend = VerificadorZ3::new();
            let mut cache = CacheLemas::cargar();
            verificar_tarea_con(ir, &mut backend, &mut cache, smt_timeout).map_err(|e| {
                miette::miette!(
                    "{}: verificación SMT (Z3) falló en '{}': {e}",
                    path.display(),
                    ir.nombre
                )
            })?;
            cache.persistir();
        }
        #[cfg(not(feature = "smt"))]
        {
            silc_causal_ir::smt::verificar_tarea(ir, smt_timeout).map_err(|e| {
                miette::miette!(
                    "{}: verificación SMT (intervalos) falló en '{}': {e}",
                    path.display(),
                    ir.nombre
                )
            })?;
        }
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
    if c.tareas_ir.is_empty() {
        return Err(miette::miette!("archivo sin tareas"));
    }

    match target {
        "c99" => {
            // Una sola tarea con parámetros: función suelta (biblioteca).
            // En otro caso: programa completo con main ejecutable.
            let codigo = if c.tareas_ir.len() == 1 && !c.tareas_ir[0].params.is_empty() {
                silc_backend::c99::emitir(&c.tareas_ir[0])
                    .map_err(|e| miette::miette!("backend C99: {e}"))?
            } else {
                silc_backend::c99::emitir_programa(&c.tareas_ir)
                    .map_err(|e| miette::miette!("backend C99: {e}"))?
            };
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
            // Un módulo por programa (todas las tareas + start + host).
            // Una sola tarea con parámetros: módulo de una función.
            let bytes = if c.tareas_ir.len() == 1 && !c.tareas_ir[0].params.is_empty() {
                silc_backend::wasm::emitir(&c.tareas_ir[0])
                    .map_err(|e| miette::miette!("backend WASM: {e}"))?
            } else {
                silc_backend::wasm::emitir_programa(&c.tareas_ir)
                    .map_err(|e| miette::miette!("backend WASM: {e}"))?
            };
            std::fs::write(out, &bytes)
                .into_diagnostic()
                .wrap_err_with(|| format!("No se pudo escribir {}", out.display()))?;
            println!(
                "{} ({} tareas) → {} ({} bytes WASM)",
                c.nombre_archivo,
                c.tareas_ir.len(),
                out.display(),
                bytes.len()
            );
            let host_js = out.with_extension("host.js");
            std::fs::write(&host_js, silc_backend::wasm::HOST_JS)
                .into_diagnostic()
                .wrap_err_with(|| format!("No se pudo escribir {}", host_js.display()))?;
            println!("host de referencia: {}", host_js.display());
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
    // Intentar cc, gcc, clang en orden. En Windows se enlaza ws2_32
    // (la genera el runtime solo si el programa usa red).
    let comps = ["cc", "gcc", "clang"];
    let mut ultimo_err = String::new();
    for cc in comps {
        let mut cmd = std::process::Command::new(cc);
        cmd.args(["-std=c99", "-O2", "-Wall"])
            .arg(c_path)
            .arg("-o")
            .arg(&bin);
        if cfg!(windows) {
            cmd.arg("-lws2_32");
        }
        let r = cmd.output();
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
    let codigo = std::fs::read_to_string(&tmp_c)
        .into_diagnostic()
        .wrap_err("no se pudo leer el C generado")?;
    if !codigo.contains("int main(void)") {
        let _ = std::fs::remove_file(&tmp_c);
        return Err(miette::miette!(
            "el archivo es una biblioteca (la última tarea tiene parámetros); `silc run` requiere una tarea principal sin parámetros"
        ));
    }
    // Compilar a binario temporal.
    let tmp_bin = tmp_c.with_extension(if cfg!(windows) { "exe" } else { "out" });
    let comps = ["cc", "gcc", "clang"];
    let mut compilado = false;
    for cc in comps {
        let mut cmd = std::process::Command::new(cc);
        cmd.args(["-std=c99", "-O2"])
            .arg(&tmp_c)
            .arg("-o")
            .arg(&tmp_bin);
        if cfg!(windows) {
            cmd.arg("-lws2_32");
        }
        let r = cmd.output();
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
        .with_writer(std::io::stderr)
        .init();

    match cli.cmd {
        Comando::Build {
            archivos,
            out,
            target,
            cc,
        } => {
            let cc = cc || std::env::var("SIL_CC").map(|v| v == "1").unwrap_or(false);
            cmd_build(&archivos, &out, &target, cli.smt_timeout, cc)
        }
        Comando::Check { archivos } => cmd_check(&archivos, cli.smt_timeout),
        Comando::EmitSmt { archivos } => cmd_emit_smt(&archivos, cli.smt_timeout),
        Comando::Run { archivos } => cmd_run(&archivos, cli.smt_timeout),
        Comando::Lsp => lsp::servir(),
    }
}
