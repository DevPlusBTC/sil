//! silc CLI: compilador SIL Fase 0.
//!
//! Comandos: build, check, run, test, fmt, lsp, emit-smt, --version.

use clap::{Parser, Subcommand};
use miette::Result;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "silc", version, about = "Compilador SIL (Semantic Intention Language) - Fase 0")]
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
    /// Compila .sil → binario nativo (o --target)
    Build {
        /// Archivos fuente .sil
        #[arg(required = true)]
        archivos: Vec<String>,
        /// Salida
        #[arg(short, long, default_value = "a.out")]
        out: String,
        /// Target: native | wasm | c99 | spirv
        #[arg(long, default_value = "native")]
        target: String,
        /// Optimización: none | speed | size | max
        #[arg(long, default_value = "speed")]
        opt: String,
    },
    /// Solo verifica sintaxis + SMT (sin codegen)
    Check {
        #[arg(required = true)]
        archivos: Vec<String>,
    },
    /// Compila y ejecuta (modo desarrollo)
    Run {
        #[arg(required = true)]
        archivos: Vec<String>,
    },
    /// Inicia servidor LSP
    Lsp,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Tracing setup
    let nivel = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    tracing_subscriber::fmt()
        .with_env_filter(format!("silc={nivel}"))
        .init();

    match cli.cmd {
        Comando::Build { archivos, out, target, opt } => {
            info!(?archivos, ?out, ?target, ?opt, "build (M0: no implementado)");
            println!("silc build: M0 esqueleto. Implementación en M5 (C99) / M6 (LLVM).");
            println!("  archivos: {archivos:?} → {out} [{target}/{opt}]");
        }
        Comando::Check { archivos } => {
            info!(?archivos, "check (M0: no implementado)");
            // M1/M2: lexear + parsear cada archivo, reportar errores miette
            for arch in &archivos {
                let fuente = std::fs::read_to_string(arch)
                    .map_err(|e| miette::miette!("No se pudo leer {arch}: {e}"))?;
                let toks = silc_frontend::lexer::lexear(&fuente);
                println!("{arch}: {} tokens (parse en M2)", toks.len());
            }
        }
        Comando::Run { archivos } => {
            println!("silc run: M0 esqueleto. Archivos: {archivos:?}");
        }
        Comando::Lsp => {
            println!("silc lsp: M0 esqueleto. Servidor LSP en M8.");
        }
    }

    Ok(())
}
