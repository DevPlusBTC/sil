//! Targets de compilación.

/// Target de emisión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    /// Binario nativo (vía LLVM).
    #[default]
    Native,
    /// WebAssembly/WASI.
    Wasm,
    /// C99 portable.
    C99,
    /// SPIR-V (GPU).
    Spirv,
}

impl std::str::FromStr for Target {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "native" => Ok(Target::Native),
            "wasm" | "wasm32-wasi" => Ok(Target::Wasm),
            "c99" => Ok(Target::C99),
            "spirv" => Ok(Target::Spirv),
            _ => Err(format!("target desconocido: {s} (válidos: native, wasm, c99, spirv)")),
        }
    }
}
